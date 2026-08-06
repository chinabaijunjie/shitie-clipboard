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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clipboard_rs::{Clipboard, ClipboardHandler};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::storage::items;

/// 纯文本 UTI(白名单:只有含此格式的复制才捕获,M1 只处理文本)。
pub const UTI_PLAIN_TEXT: &str = "public.utf8-plain-text";

/// 前端监听的事件名(新增或去重更新一条时推送)。
pub const CLIPBOARD_ADDED_EVENT: &str = "clipboard://added";

/// 判断是否应该捕获当前剪贴板内容(纯函数,可单测)。
///
/// 决策规则(M1):
/// - 含 `public.utf8-plain-text` 格式 → 捕获(返回 true)
/// - 不含文本格式(文件/图片等)→ 跳过(M1 不处理)
///
/// `available_formats` 为剪贴板当前所有 UTI 列表。
pub fn should_capture(available_formats: &[String]) -> bool {
    available_formats.iter().any(|f| f == UTI_PLAIN_TEXT)
}

/// 判断文本是否值得入库(纯函数,可单测)。
///
/// 空文本(用户清空剪贴板等)不入库。
pub fn is_worth_storing(text: &str) -> bool {
    !text.is_empty()
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
    /// 自身写入标志(与 copy_back 共享):true 表示下一次变化是自身写入,应跳过。
    /// on_clipboard_change 开头消费(置 false),若原为 true 则跳过本次捕获。
    self_write_flag: Arc<AtomicBool>,
}

impl ClipboardChangeHandler {
    /// 在 watcher 线程内创建(handler 必须在线程内构造,Connection 不能跨线程发送)。
    pub fn new(
        app: AppHandle,
        db: Connection,
        self_write_flag: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let clipboard = clipboard_rs::ClipboardContext::new()
            .map_err(|e| format!("ClipboardContext::new 失败: {e}"))?;
        Ok(Self {
            app,
            db,
            clipboard,
            self_write_flag,
        })
    }

    /// 消费自身写入标志。返回 true 表示「这是 copy_back 自身写入,应跳过」。
    /// 用 swap 而非 load+store,保证只消费一次(避免并发竞态)。
    fn consume_self_write(&self) -> bool {
        self.self_write_flag.swap(false, Ordering::SeqCst)
    }
}

impl ClipboardHandler for ClipboardChangeHandler {
    fn on_clipboard_change(&mut self) {
        // 0. 消费自身写入标志:copy_back 刚写过剪贴板,这次变化是自身写入,跳过。
        //    (防反馈循环:copy_back set_text → changeCount 变 → watcher 捕获 → copy_count 虚高)
        if self.consume_self_write() {
            log::debug!("[clipboard] 跳过自身写入(copy_back 触发)");
            return;
        }

        // 1. 判断是否含纯文本格式(spike 实测:文件场景无文本格式,get_text 会报错)
        let formats = match self.clipboard.available_formats() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[clipboard] available_formats 失败,跳过: {e}");
                return;
            }
        };
        if !should_capture(&formats) {
            // 非文本内容(文件/图片等),M1 不处理,静默跳过(不是错误)
            return;
        }

        // 2. 读纯文本
        let text = match self.clipboard.get_text() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[clipboard] get_text 失败,跳过: {e}");
                return;
            }
        };

        // 3. 空文本跳过(用户清空剪贴板等)
        if !is_worth_storing(&text) {
            return;
        }

        // 4. 写 DB(去重插入或更新)+ emit 前端
        match items::insert_or_update(&self.db, &text) {
            Ok(item) => {
                log::info!(
                    "[clipboard] 捕获: id={} title={:?}",
                    item.id,
                    item.title.chars().take(30).collect::<String>()
                );
                if let Err(e) = self.app.emit(CLIPBOARD_ADDED_EVENT, &item) {
                    log::warn!("[clipboard] emit 前端失败: {e}");
                }
            }
            Err(e) => {
                // DB 写入失败:log 完整错误码+消息,但不中断监听(让 watcher 继续跑)
                log::error!("[clipboard] 写 DB 失败 [{}]: {}", e.code, e.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_capture_true_when_plain_text_present() {
        // 真实纯文本复制:含 public.utf8-plain-text(spike 实测格式)
        let formats = vec![
            "public.utf8-plain-text".to_string(),
            "NSStringPboardType".to_string(),
        ];
        assert!(should_capture(&formats));
    }

    #[test]
    fn should_capture_false_when_no_text_format() {
        // 文件 URL 场景(spike 实测:无文本格式)
        let formats = vec![
            "com.apple.alias-record".to_string(),
            "CorePasteboardFlavorType 0x616C6973".to_string(),
        ];
        assert!(!should_capture(&formats));
    }

    #[test]
    fn should_capture_false_when_empty_formats() {
        assert!(!should_capture(&[]));
    }

    #[test]
    fn is_worth_storing_rejects_empty() {
        // 空字符串拒(用户清空剪贴板场景)
        assert!(!is_worth_storing(""));
    }

    #[test]
    fn is_worth_storing_keeps_whitespace() {
        // 纯空格/制表符:is_empty()=false,会被存储。
        // M1 有意保留(用户可能真复制了空格/缩进)。若要拒纯空白,M2 加 trim 判断。
        assert!(is_worth_storing("   "));
        assert!(is_worth_storing("\t\n"));
    }

    #[test]
    fn is_worth_storing_accepts_nonempty() {
        assert!(is_worth_storing("a"));
        assert!(is_worth_storing("中文内容"));
        assert!(is_worth_storing("  有内容的文本  "));
    }

    #[test]
    fn plain_text_uti_constant() {
        // 确保 UTI 字符串与系统一致(spike 实测值)
        assert_eq!(UTI_PLAIN_TEXT, "public.utf8-plain-text");
    }

    #[test]
    fn self_write_flag_consume_once_semantics() {
        // 防反馈循环的核心机制:标志位只能被消费一次
        // (copy_back 置 true → watcher 第一次 on_clipboard_change 消费返回 true 跳过
        //  → 标志清零,后续真实复制正常捕获)
        let flag = Arc::new(AtomicBool::new(false));

        // 初始:false(不跳过)
        assert!(!flag.swap(false, Ordering::SeqCst));

        // copy_back 置位
        flag.store(true, Ordering::SeqCst);

        // watcher 第一次消费:返回 true(跳过自身写入),并清零
        assert!(flag.swap(false, Ordering::SeqCst));

        // 第二次消费(真实复制):返回 false(不跳过,正常捕获)
        assert!(!flag.swap(false, Ordering::SeqCst));
    }
}
