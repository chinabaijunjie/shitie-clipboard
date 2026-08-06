//! 🔬 SPIKE 代码 — M1 剪贴板监听可行性验证(临时,验证完演化为正式实现或删除)
//!
//! 验证目标(wiki/dev/m1-research.md 第四节):
//! 1. 🔴 clipboard-rs 的 ClipboardWatcherContext 在 Tauri 后台线程跑,
//!    会不会触发 NSPasteboard 线程安全崩溃(tauri-apps/plugins-workspace#3205)?
//! 2. 🟡 真实复制场景下 available_formats() + get_text() 行为是否正常?
//!
//! 架构:
//! - setup 里 std::thread::spawn 一个专用后台线程
//! - ClipboardWatcherContext::new_with_interval(500ms) + add_handler
//! - handler 持有 AppHandle,on_clipboard_change 时读文本 + emit 前端
//! - AppState 持有 WatcherShutdown,退出时停止
//!
//! ⚠️ 此模块是临时验证代码,不是正式实现。验证结论回填 m1-research.md 后,
//!    此模块演化为正式的 clipboard 监听模块(或删除重写)。

use std::time::Duration;

use clipboard_rs::{
    common::ClipboardContent, Clipboard, ClipboardContext, ClipboardHandler,
    ClipboardWatcher, ClipboardWatcherContext, WatcherShutdown,
};
use tauri::{AppHandle, Emitter};

/// 前端监听的事件名(临时,spike 用)。
pub const SPIKE_EVENT: &str = "spike://clipboard-changed";

/// Spike handler:检测到剪贴板变化时读文本并 emit 前端。
///
/// 注意:ClipboardHandler 要求 on_clipboard_change(&mut self),不能直接用闭包。
struct SpikeHandler {
    app: AppHandle,
    /// 缓存 ClipboardContext 用于读内容(on_clipboard_change 里读)。
    /// ⚠️ 这是 spike 要验证的关键点:从后台线程拿到的 context 读 NSPasteboard 会不会崩溃。
    clipboard: ClipboardContext,
}

impl ClipboardHandler for SpikeHandler {
    fn on_clipboard_change(&mut self) {
        // 🔴 验证点 1:从后台线程读 NSPasteboard,看是否 EXC_BAD_ACCESS 崩溃。
        let text = self.clipboard.get_text().unwrap_or_else(|e| format!("<读文本失败: {e}>"));
        let formats = self
            .clipboard
            .available_formats()
            .unwrap_or_default();

        log::warn!(
            "[spike] 剪贴板变化!formats={:?}, text 长度={}, 预览={:?}",
            formats,
            text.len(),
            text.chars().take(50).collect::<String>()
        );

        // emit 给前端(验证前后端事件链路)。
        // payload 带 formats(看真实复制有哪些 UTI)+ 文本预览。
        let _ = self.app.emit(
            SPIKE_EVENT,
            serde_json::json!({
                "formats": formats,
                "preview": text.chars().take(100).collect::<String>(),
                "len": text.len(),
            }),
        );
    }
}

/// 启动剪贴板监听后台线程,返回停止句柄。
///
/// 在 Tauri setup 里调用。返回的 WatcherShutdown 存入 AppState,应用退出时 drop 停止。
pub fn start_watcher(app: &AppHandle) -> Result<WatcherShutdown, String> {
    let clipboard =
        ClipboardContext::new().map_err(|e| format!("ClipboardContext::new 失败: {e}"))?;

    let handler = SpikeHandler {
        app: app.clone(),
        clipboard,
    };

    let mut watcher =
        ClipboardWatcherContext::new_with_interval(Duration::from_millis(500))
            .map_err(|e| format!("ClipboardWatcherContext::new 失败: {e}"))?;

    let shutdown = watcher.get_shutdown_channel();
    watcher.add_handler(handler);

    std::thread::Builder::new()
        .name("clipboard-watcher-spike".into())
        .spawn(move || {
            log::warn!("[spike] 剪贴板监听线程启动,开始 start_watch(阻塞)");
            // start_watch 阻塞当前线程,直到 shutdown channel 触发。
            watcher.start_watch();
            log::warn!("[spike] 剪贴板监听线程退出");
        })
        .map_err(|e| format!("spawn watcher 线程失败: {e}"))?;

    Ok(shutdown)
}

/// 暴露给前端 invoke 的临时 command:主动读一次剪贴板(spike 验证同步读路径)。
#[tauri::command]
pub fn spike_read_clipboard(state: tauri::State<'_, crate::AppState>) -> Result<serde_json::Value, String> {
    // 从 AppState 拿一个独立的 ClipboardContext(不碰 watcher 的那个)。
    let cb = ClipboardContext::new().map_err(|e| e.to_string())?;
    let text = cb.get_text().unwrap_or_default();
    let formats = cb.available_formats().unwrap_or_default();
    let _ = state; // 占位,确认 State 注入可用
    Ok(serde_json::json!({
        "formats": formats,
        "preview": text.chars().take(100).collect::<String>(),
    }))
}

/// 允许上层在不需要时编译排除整个 spike 模块的标志性引用。
#[allow(dead_code)]
pub fn _touch(_c: &ClipboardContent) {}
