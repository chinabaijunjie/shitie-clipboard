//! 剪贴板监听层 — 管理监听线程的生命周期(启动/停止)
//!
//! 职责(单一):起一个后台线程跑 clipboard-rs watcher,检测到变化时交给 handler 处理。
//! 不关心读什么内容(那是 handler.rs),不关心怎么存(那是 storage)。
//!
//! 架构(spike 验证结论,m1-research.md 第四节):
//! - 后台 std::thread 跑 ClipboardWatcherContext::start_watch(阻塞)
//! - 500ms 轮询 changeCount(macOS 无原生通知,Maccy/clipboard-master 共识值)
//! - 检测到变化 → handler.on_clipboard_change(后台线程内,读 NSPasteboard 不崩溃)
//!
//! ⚠️ 线程内构造 watcher 的原因:
//! handler 含 rusqlite::Connection(不是 Send),无法跨线程移动。
//! 所以 watcher + handler 都在监听线程内创建,只把 shutdown channel 发回主线程。
//!
//! ⚠️ 平台边界:clipboard-rs 屏蔽平台差异(M1 macOS 轮询 / M3 Windows 原生事件),
//! 本模块代码平台无关,M3 加 Windows 时此处零改动。

pub mod capture;
pub mod handler;
pub mod title;
pub mod uti;

use std::{
    path::Path,
    sync::atomic::AtomicBool,
    sync::Arc,
    time::Duration,
};

use clipboard_rs::{ClipboardWatcher as _, ClipboardWatcherContext, WatcherShutdown};
use tauri::AppHandle;

use handler::{ClipboardChangeHandler, CLIPBOARD_ADDED_EVENT};

/// 监听轮询间隔(500ms,Maccy/clipboard-master 共识值)。
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// 剪贴板监听器句柄(持有停止通道,drop 即停止)。
///
/// 命名加 Handle 后缀,避免与 clipboard_rs::ClipboardWatcher trait 同名冲突。
pub struct ClipboardWatcherHandle {
    _shutdown: WatcherShutdown,
}

impl ClipboardWatcherHandle {
    /// 启动剪贴板监听后台线程。
    ///
    /// `db_path`:数据库文件路径(handler 在 watcher 线程内开独立连接,WAL 模式并发安全)。
    /// `self_write_flag`:自身写入标志(与 copy_back 共享;copy_back 置位,handler 消费跳过)。
    ///
    /// 返回的 handle drop 时自动停止监听(通过 WatcherShutdown 的 Drop 语义)。
    pub fn start(
        app: AppHandle,
        db_path: &Path,
        self_write_flag: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let (tx_shutdown, rx_shutdown) = std::sync::mpsc::channel::<WatcherShutdown>();
        let db_path = db_path.to_path_buf();
        let app_for_thread = app;

        std::thread::Builder::new()
            .name("clipboard-watcher".into())
            .spawn(move || run_watcher(app_for_thread, &db_path, self_write_flag, tx_shutdown))
            .map_err(|e| format!("spawn clipboard-watcher 线程失败: {e}"))?;

        // 等待线程内把 shutdown channel 发回来。
        // 线程启动后立即创建 watcher 并发回 shutdown,此处几乎不阻塞。
        // 若收到 None 表示线程 panic 或提前 return(启动失败),rx_shutdown 会返回 Err。
        let shutdown = rx_shutdown
            .recv()
            .map_err(|_| "监听线程未返回 shutdown channel(启动失败,见日志)".to_string())?;

        Ok(ClipboardWatcherHandle {
            _shutdown: shutdown,
        })
    }
}

/// watcher 线程主体:开 DB → 建 handler → 创建 watcher → 发回 shutdown → start_watch(阻塞)。
///
/// 任一步失败都 log 后 return(线程退出,不 panic)。shutdown 通过 channel 发回主线程。
fn run_watcher(
    app: AppHandle,
    db_path: &Path,
    self_write_flag: Arc<AtomicBool>,
    tx_shutdown: std::sync::mpsc::Sender<WatcherShutdown>,
) {
    log::info!("[clipboard] 监听线程启动");

    // 线程内开独立 DB 连接(WAL 模式与主连接并发安全)
    let db = match crate::storage::init_db(db_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[clipboard] 线程内开 DB 连接失败: {}", e.message);
            return;
        }
    };

    // 线程内构造 handler(传入 self_write_flag 用于跳过自身写入)
    let handler = match ClipboardChangeHandler::new(app, db, self_write_flag) {
        Ok(h) => h,
        Err(e) => {
            log::error!("[clipboard] 构造 handler 失败: {e}");
            return;
        }
    };

    // 创建 watcher 并发回 shutdown channel
    let mut watcher = match ClipboardWatcherContext::new_with_interval(POLL_INTERVAL) {
        Ok(w) => w,
        Err(e) => {
            log::error!("[clipboard] 创建 watcher 失败: {e}");
            return;
        }
    };
    let shutdown = watcher.get_shutdown_channel();
    if tx_shutdown.send(shutdown).is_err() {
        log::error!("[clipboard] 发送 shutdown channel 失败(主线程已放弃启动)");
        return;
    }

    watcher.add_handler(handler);
    log::info!(
        "[clipboard] 开始监听(500ms 轮询 changeCount),event={CLIPBOARD_ADDED_EVENT}"
    );
    // start_watch 阻塞,直到 shutdown channel 触发(drop handle → WatcherShutdown drop → 停止)
    watcher.start_watch();
    log::info!("[clipboard] 监听线程退出");
}
