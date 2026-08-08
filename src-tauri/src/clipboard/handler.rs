//! 剪贴板变化处理器 — on_clipboard_change → 读内容 → 写 DB → emit 前端
//!
//! 职责(单一):剪贴板变化时,把内容捕获成 ClipboardItem 存下来,并通知前端。
//! 不关心监听线程生命周期(那是 clipboard/mod.rs 的事),不关心 UI 怎么展示。
//!
//! ⚠️ 线程安全(spike 验证结论,m1-research.md 第四节):
//! - on_clipboard_change 在 watcher 后台线程调用
//! - 读 NSPasteboard 不崩溃(spike 5 次实测)
//! - DB 写入用本线程独立的 Connection(rusqlite WAL 模式下多连接并发安全)
//!
//! ⚠️ 容错(spike 实测发现):
//! - get_text() 在无文本格式时报错(文件 URL 场景),必须先 available_formats() 判断
//! - 读/写任何一步失败都只 log,不 panic(watcher 崩了监听就断了)
//!
//! M2:多格式捕获。on_clipboard_change 流程:
//! 1. consume_self_write(防反馈循环,M1 机制保留)
//! 2. available_formats → classify_formats(白/黑名单决策)
//! 3. capture_multi 读多格式 → classify_content_kind
//! 4. generate_title 按 kind 生成标题
//! 5. insert_or_update_multi 写 DB + emit 前端

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clipboard_rs::{Clipboard, ClipboardHandler};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::capture::{self, CaptureDecision};
use crate::clipboard::title;
use crate::storage::items::{self, FormatInput};

/// 前端监听的事件名(新增或去重更新一条时推送)。
pub const CLIPBOARD_ADDED_EVENT: &str = "clipboard://added";

/// 消费自身写入标志,决定是否跳过本次剪贴板变化(纯函数,可单测)。
///
/// 业务契约(copy_back 反馈循环防护):
/// - 标志被置位(copy_back 写剪贴板前 mark)→ 返回 true(跳过本次,这是自身写入)
/// - 标志未置位(用户真实复制)→ 返回 false(正常捕获)
///
/// 用 swap(false) 消费:保证只跳过一次,下一次变化(真实复制)正常处理。
/// 这把「反馈循环防护」的核心决策从 handler 的私有方法提升为可测的纯函数。
pub fn should_skip_by_flag(flag: &std::sync::atomic::AtomicBool) -> bool {
    flag.swap(false, Ordering::SeqCst)
}

/// 判断是否应跳过空捕获(纯函数,可单测)。
///
/// 业务契约:用户清空剪贴板时,clipboard-rs 会触发变化但内容为空。
/// - 只有纯文本格式 + trim 后为空 → 跳过(空内容不入库)
/// - 有多种格式(如 rtf + 空 text)→ 不跳过(其他格式可能是有效的)
/// - 有图片/文件格式 → 不跳过(即使无文本也是有效内容)
pub fn should_skip_empty_capture(formats: &[crate::clipboard::capture::CapturedFormat]) -> bool {
    use crate::clipboard::uti::UTI_PLAIN_TEXT;
    // 只有纯文本格式时才考虑跳过
    if formats.len() != 1 {
        return false;
    }
    let only = &formats[0];
    if only.uti != UTI_PLAIN_TEXT {
        return false;
    }
    String::from_utf8_lossy(&only.data).trim().is_empty()
}

/// 剪贴板变化处理器。
///
/// 持有独立的 DB 连接(本线程创建,WAL 模式下与其他连接并发安全)
/// + ClipboardContext(读剪贴板)+ AppHandle(emit 前端)
/// + self_write_flag(检测并跳过 copy_back 自身写入,防反馈循环)。
pub struct ClipboardChangeHandler {
    app: AppHandle,
    /// 本线程独立的 DB 连接(在 start_watcher 里于 watcher 线程内创建)。
    db: Connection,
    /// 剪贴板读取上下文(clipboard-rs)。
    clipboard: clipboard_rs::ClipboardContext,
    /// 图片存储根目录(图片捕获存盘用)。
    images_dir: std::path::PathBuf,
    /// 自身写入标志(与 copy_back 共享):true 表示下一次变化是自身写入,应跳过。
    /// on_clipboard_change 开头消费(置 false),若原为 true 则跳过本次捕获。
    self_write_flag: Arc<AtomicBool>,
}

impl ClipboardChangeHandler {
    /// 在 watcher 线程内创建(handler 必须在线程内构造,Connection 不能跨线程发送)。
    pub fn new(
        app: AppHandle,
        db: Connection,
        images_dir: std::path::PathBuf,
        self_write_flag: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let clipboard = clipboard_rs::ClipboardContext::new()
            .map_err(|e| format!("ClipboardContext::new 失败: {e}"))?;
        Ok(Self {
            app,
            db,
            clipboard,
            images_dir,
            self_write_flag,
        })
    }

    /// 消费自身写入标志。返回 true 表示「这是 copy_back 自身写入,应跳过」。
    /// 用 swap 而非 load+store,保证只消费一次(避免并发竞态)。
    fn consume_self_write(&self) -> bool {
        should_skip_by_flag(&self.self_write_flag)
    }
}

impl ClipboardHandler for ClipboardChangeHandler {
    fn on_clipboard_change(&mut self) {
        // 0. 消费自身写入标志:copy_back 刚写过剪贴板,这次变化是自身写入,跳过。
        //    (防反馈循环:copy_back set → changeCount 变 → watcher 捕获 → copy_count 虚高)
        if self.consume_self_write() {
            log::debug!("[clipboard] 跳过自身写入(copy_back 触发)");
            return;
        }

        // 1. 读剪贴板所有 UTI
        let available_formats = match self.clipboard.available_formats() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[clipboard] available_formats 失败,跳过: {e}");
                return;
            }
        };

        // 2. 分类:隐私跳过 / 白名单交集(纯函数决策)
        let decision = capture::classify_formats(&available_formats);
        let CaptureDecision::Capture(to_capture_utis) = decision else {
            // 隐私跳过 或 无白名单格式,静默跳过(不是错误)
            return;
        };

        // 3. 读多格式内容(文本/富文本/文件/图片;图片走 image_store 存盘)
        let captured = match capture::capture_multi(&self.clipboard, &to_capture_utis, &self.images_dir) {
            Ok(Some(c)) => c,
            Ok(None) => {
                // 无有效内容,跳过
                return;
            }
            Err(e) => {
                log::warn!("[clipboard] capture_multi 失败,跳过 [{}]: {}", e.code, e.message);
                return;
            }
        };

        // 4. 空内容跳过(如清空剪贴板):只有纯文本且 trim 后为空时跳过
        //    (图片/文件即使内容「空」也是有效内容)
        if should_skip_empty_capture(&captured.formats) {
            return;
        }

        // 5. 生成标题(图片用尺寸)+ 转成 FormatInput 写 DB
        let title_str = title::generate_title(&captured.formats, captured.kind, captured.image_size);
        let format_inputs: Vec<FormatInput<'_>> = captured
            .formats
            .iter()
            .map(|f| FormatInput {
                uti: &f.uti,
                data: &f.data,
                file_path: f.file_path.as_deref(),
            })
            .collect();

        match items::insert_or_update_multi(
            &self.db,
            &format_inputs,
            captured.kind,
            &title_str,
            &self.images_dir,
        ) {
            Ok(item) => {
                // insert_or_update_multi 已通过 fill_image_fields 把 thumb_path 填为绝对路径
                log::info!(
                    "[clipboard] 捕获: id={} kind={:?} title={:?}",
                    item.id,
                    item.content_kind,
                    item.title.chars().take(30).collect::<String>()
                );
                if let Err(e) = self.app.emit(CLIPBOARD_ADDED_EVENT, &item) {
                    log::warn!("[clipboard] emit 前端失败: {e}");
                }

                // M3:自动清理(保留天数 + 容量上限 + 条数上限)
                // 每次成功写入后触发,从 AppState 读设置决定是否清理
                if let Some(state) = self.app.try_state::<crate::AppState>() {
                    let (retention_days, max_storage_mb, max_items) = {
                        let guard = state.settings();
                        let s = guard.lock().expect("Settings Mutex 中毒");
                        (s.retention_days, s.max_storage_mb, s.max_items)
                    };
                    if crate::commands::settings::should_run_cleanup(retention_days, max_storage_mb)
                        || max_items > 0
                    {
                        if let Err(e) = crate::storage::cleanup::run_cleanup(
                            &self.db,
                            retention_days,
                            max_storage_mb,
                            max_items,
                            &self.images_dir,
                        ) {
                            log::warn!("[clipboard] 自动清理失败: {} {}", e.code, e.message);
                        }
                    }
                } else {
                    log::warn!("[clipboard] AppState 未就绪,跳过自动清理");
                }
            }
            Err(e) => {
                log::error!("[clipboard] 写 DB 失败 [{}]: {}", e.code, e.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::capture::CapturedFormat;

    fn fmt(uti: &str, data: &str) -> CapturedFormat {
        CapturedFormat {
            uti: uti.to_string(),
            data: data.as_bytes().to_vec(),
            file_path: None,
        }
    }

    #[test]
    fn should_skip_empty_capture_skips_empty_text() {
        // 用户清空剪贴板:只有空文本 → 跳过
        assert!(should_skip_empty_capture(&[fmt("public.utf8-plain-text", "")]));
        assert!(should_skip_empty_capture(&[fmt("public.utf8-plain-text", "   ")]));
        assert!(should_skip_empty_capture(&[fmt("public.utf8-plain-text", "\t\n")]));
    }

    #[test]
    fn should_skip_empty_capture_keeps_nonempty_text() {
        assert!(!should_skip_empty_capture(&[fmt("public.utf8-plain-text", "有内容")]));
        assert!(!should_skip_empty_capture(&[fmt("public.utf8-plain-text", "  x  ")]));
    }

    #[test]
    fn should_skip_empty_capture_keeps_multi_format() {
        // 多格式(即使 text 为空)不跳过:其他格式可能有效
        let formats = vec![
            fmt("public.utf8-plain-text", ""),
            fmt("public.rtf", "{\\rtf1 内容}"),
        ];
        assert!(!should_skip_empty_capture(&formats));
    }

    #[test]
    fn should_skip_empty_capture_keeps_non_text() {
        // 图片/文件格式即使内容「空」也不跳过(有效内容)
        assert!(!should_skip_empty_capture(&[fmt("public.png", "")]));
        assert!(!should_skip_empty_capture(&[fmt("public.file-url", "")]));
    }

    #[test]
    fn should_skip_by_flag_skips_when_marked() {
        // 业务契约:copy_back 置位后,handler 应跳过本次变化(防反馈循环)
        let flag = AtomicBool::new(false);
        flag.store(true, Ordering::SeqCst); // 模拟 copy_back mark_self_write
        assert!(should_skip_by_flag(&flag), "置位后应跳过(自身写入)");
    }

    #[test]
    fn should_skip_by_flag_captures_when_not_marked() {
        // 业务契约:未置位时(用户真实复制),不跳过,正常捕获
        let flag = AtomicBool::new(false);
        assert!(!should_skip_by_flag(&flag), "未置位应正常捕获");
    }

    #[test]
    fn should_skip_by_flag_consumes_once() {
        // 业务契约:标志只跳过一次(消费语义),下一次变化正常捕获
        let flag = AtomicBool::new(true);
        assert!(should_skip_by_flag(&flag), "第一次:跳过自身写入");
        assert!(!should_skip_by_flag(&flag), "第二次:已消费,正常捕获真实复制");
    }
}
