//! Clipboard Manager — Tauri 主进程
//!
//! 架构:纯 Rust 单进程(Tauri 既是壳也是后端),无 Python sidecar / 无 HTTP。
//!
//! 职责:
//! - 注册 Tauri commands 暴露给前端 invoke(M0 仅 ping,M1+ 加剪贴板业务)
//! - 管理 AppState(M1+ 加剪贴板轮询线程 / DB 连接等)
//! - 应用级生命周期(M1+ 在 RunEvent::Exit 清理监听 / 托盘)

mod commands;
mod error;
mod models;
mod spike_clipboard; // 🔬 M1 spike,验证完演化为正式模块或删除
mod storage;

// 把 command 函数 use 到当前作用域,generate_handler! 直接用裸名注册。
// (宏对模块路径解析有限制,use 别名最可靠)
use commands::ping::ping;
use spike_clipboard::spike_read_clipboard;

use clipboard_rs::WatcherShutdown;
use tauri::Manager;

/// 应用全局状态。
///
/// M0:空占位。M1 spike:持有剪贴板监听线程的停止句柄。
/// 正式 M1 实现会持有 watcher shutdown + DB 连接等共享资源。
pub struct AppState {
    /// 剪贴板监听线程停止句柄。drop 即停止监听(spike)。
    #[allow(dead_code)]
    watcher_shutdown: Option<WatcherShutdown>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            watcher_shutdown: None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 日志插件:常驻工具排障必备(写 ~/Library/Logs/{bundleId}/)。
            // Warn 级:Info 太吵(每次 invoke 都打),Warn + Error 足够排障且不刷屏。
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Warn)
                    .build(),
            )?;

            // 注册全局状态。M1 起在此初始化剪贴板轮询线程 / DB 连接。
            // ⚠️ 不在这里 emit 任何事件:setup 早于 WebView JS,emit 即发即弃无缓冲
            //    (constraints.md「Tauri v2 setup 时序坑」)。
            //    前端用 invoke ping 探活,调用本身就是就绪信号。
            let mut state = AppState::new();

            // 🔬 SPIKE:启动剪贴板监听后台线程。
            // 验证 m1-research.md 的核心未知项:NSPasteboard 线程安全崩溃。
            // 启动监听本身不 emit(避开 setup 时序坑),事件在 watcher 回调里发(那时 WebView 已就绪)。
            match spike_clipboard::start_watcher(&app.handle()) {
                Ok(shutdown) => {
                    state.watcher_shutdown = Some(shutdown);
                    log::warn!("[spike] 剪贴板监听已启动");
                }
                Err(e) => {
                    log::error!("[spike] 剪贴板监听启动失败: {e}");
                }
            }

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            spike_read_clipboard, // 🔬 M1 spike
            // M1+ 在此追加:clipboard / storage command
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // M1+ 在此处理 RunEvent::Exit:停止剪贴板监听 / 关闭 DB / 清理托盘
        });
}
