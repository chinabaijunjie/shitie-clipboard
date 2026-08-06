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

use clipboard_rs::{common::ContentFormat, Clipboard, ClipboardHandler};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::storage::items;

/// 纯文本 UTI(白名单:只有含此格式的复制才捕获,M1 只处理文本)。
const UTI_PLAIN_TEXT: &str = "public.utf8-plain-text";

/// 前端监听的事件名(新增或去重更新一条时推送)。
pub const CLIPBOARD_ADDED_EVENT: &str = "clipboard://added";

/// 剪贴板变化处理器。
///
/// 持有独立的 DB 连接(本线程创建,WAL 模式下与其他连接并发安全)
/// + ClipboardContext(读剪贴板)+ AppHandle(emit 前端)。
pub struct ClipboardChangeHandler {
    app: AppHandle,
    /// 本线程独立的 DB 连接(在 start_watcher 里于 watcher 线程内创建)。
    db: Connection,
    /// 剪贴板读取上下文(clipboard-rs)。
    clipboard: clipboard_rs::ClipboardContext,
}

impl ClipboardChangeHandler {
    /// 在 watcher 线程内创建(handler 必须在线程内构造,Connection 不能跨线程发送)。
    ///
    /// `db_path`:数据库文件路径(WAL 模式,与主连接并发安全)。
    pub fn new(app: AppHandle, db: Connection) -> Result<Self, String> {
        let clipboard = clipboard_rs::ClipboardContext::new()
            .map_err(|e| format!("ClipboardContext::new 失败: {e}"))?;
        Ok(Self {
            app,
            db,
            clipboard,
        })
    }
}

impl ClipboardHandler for ClipboardChangeHandler {
    fn on_clipboard_change(&mut self) {
        // 1. 先判断是否含纯文本格式(spike 实测:文件场景无文本格式,get_text 会报错)
        let has_text = match self.clipboard.has(ContentFormat::Text) {
            true => true,
            false => {
                // has() 不可靠时回退到 available_formats() 检查
                match self.clipboard.available_formats() {
                    Ok(formats) => formats.iter().any(|f| f == UTI_PLAIN_TEXT),
                    Err(e) => {
                        log::warn!("[clipboard] available_formats 失败,跳过: {e}");
                        return;
                    }
                }
            }
        };

        if !has_text {
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
        if text.is_empty() {
            return;
        }

        // 4. 写 DB(去重插入或更新)
        match items::insert_or_update(&self.db, &text) {
            Ok(item) => {
                log::info!(
                    "[clipboard] 捕获: id={} title={:?}",
                    item.id,
                    item.title.chars().take(30).collect::<String>()
                );
                // 5. emit 前端
                if let Err(e) = self.app.emit(CLIPBOARD_ADDED_EVENT, &item) {
                    log::warn!("[clipboard] emit 前端失败: {e}");
                }
            }
            Err(e) => {
                // DB 写入失败:log 完整错误码+消息,但不中断监听
                log::error!(
                    "[clipboard] 写 DB 失败 [{}]: {}",
                    e.code,
                    e.message
                );
                // 若是 DB 损坏类错误,后续可能持续失败,但仍不 panic(让 watcher 继续跑)
            }
        }
    }
}

/// 辅助:把 ContentFormat 转成 UTI 字符串(调试用,目前只用 Text)。
#[allow(dead_code)]
fn _content_format_uti(fmt: ContentFormat) -> &'static str {
    match fmt {
        ContentFormat::Text => UTI_PLAIN_TEXT,
        _ => "other",
    }
}
