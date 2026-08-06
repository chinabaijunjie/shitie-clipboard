//! Clipboard Manager — Tauri 主进程
//!
//! 架构:纯 Rust 单进程(Tauri 既是壳也是后端),无 Python sidecar / 无 HTTP。
//!
//! 职责:
//! - 注册 Tauri commands 暴露给前端 invoke(M0 ping,M1 起加剪贴板业务)
//! - 管理 AppState(DB 连接 + 剪贴板监听句柄等共享资源)
//! - 应用级生命周期(目前 watcher shutdown 靠 AppState drop 自动清理)
//!
//! 模块化(§1.21):
//! - clipboard:监听剪贴板变化 → handler 读内容 → 写 DB → emit 前端
//! - storage:纯数据访问(items/formats CRUD),不感知剪贴板
//! - commands:invoke 入口(阶段 C 加 list_items/copy_back 等)
//! - models:纯数据类型
//! - error:错误契约

mod clipboard;
mod commands;
mod error;
mod models;
mod storage;

use std::sync::Mutex;

use commands::ping::ping;
use tauri::Manager;

/// 应用全局状态。
///
/// 持有:
/// - 主 DB 连接(给 command 读列表/回填用,Mutex 包,command handler 短时持锁)
/// - 剪贴板监听句柄(drop 即停止监听)
///
/// ⚠️ watcher 线程有自己独立的 DB 连接(WAL 模式下并发安全),与这里的 db 互不干扰。
pub struct AppState {
    /// 主 DB 连接(command 用)。Mutex 因 Tauri command 可能并发调用。
    db: Mutex<rusqlite::Connection>,
    /// 剪贴板监听句柄。drop 即停止监听(app 退出时随 AppState 自动清理)。
    _watcher: clipboard::ClipboardWatcherHandle,
}

impl AppState {
    /// 获取 db 连接的锁(command handler 用)。
    ///
    /// ⚠️ 短时持有:command 跑在 tokio 线程,持锁期间会阻塞其他 command。
    /// M1 命令都是毫秒级查询,可接受。
    pub fn db(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db
            .lock()
            .expect("DB Mutex 中毒(持有者 panic)")
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 日志插件:常驻工具排障必备(写 ~/Library/Logs/{bundleId}/)。
            // Info 级:M1 起剪贴板捕获需要 info 日志观测(每次复制打一条);仍过滤 verbose。
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // DB 路径:app_data_dir/clipboard.db
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("拿不到 app_data_dir");
            std::fs::create_dir_all(&data_dir)
                .expect("创建 app_data_dir 失败");
            let db_path = data_dir.join("clipboard.db");

            // 主 DB 连接(command 用)
            let main_conn = storage::init_db(&db_path)
                .expect("主 DB 连接初始化失败");

            // 启动剪贴板监听(后台线程,独立 DB 连接)
            // ⚠️ 不在这里 emit 任何事件:setup 早于 WebView JS,emit 即发即弃无缓冲
            //    (constraints.md「Tauri v2 setup 时序坑」)。
            //    事件在 watcher 回调里发(那时 WebView 已就绪,且由用户复制触发,时序天然正确)。
            let watcher = clipboard::ClipboardWatcherHandle::start(app.handle().clone(), &db_path)
                .expect("剪贴板监听启动失败");

            app.manage(AppState {
                db: Mutex::new(main_conn),
                _watcher: watcher,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            // 阶段 C 追加:list_items / copy_back / delete_item
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // AppState drop 时 _watcher 自动停止监听,无需在此手动清理。
            // 未来若加托盘/全局快捷键,在此处理 RunEvent::Exit。
        });
}
