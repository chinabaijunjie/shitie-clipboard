//! 拾贴 — Tauri 主进程
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
mod settings;
mod storage;
mod window_control;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use commands::clipboard::{clear_history, copy_back, delete_item, get_storage_stats, list_items, search_items};
use commands::ping::ping;
use commands::settings::{get_settings, update_settings};
use tauri::{Manager, Window};

/// 应用全局状态。
///
/// 用户主动退出标志(托盘菜单「退出拾贴」置 true)。
///
/// `app.exit(0)` 会触发 `RunEvent::ExitRequested`,而 M3 的常驻逻辑会 `prevent_exit()` 拦截。
/// 用此标志区分:用户主动退出(放行)vs 窗口关闭触发的退出请求(拦截常驻)。
static USER_REQUESTED_EXIT: AtomicBool = AtomicBool::new(false);

/// 持有:
/// - 主 DB 连接(给 command 读列表/回填用,Mutex 包,command handler 短时持锁)
/// - 剪贴板监听句柄(drop 即停止监听)
/// - 自身写入标志(copy_back 写剪贴板前置位,watcher 检测到则跳过,避免反馈循环)
///
/// ⚠️ watcher 线程有自己独立的 DB 连接(WAL 模式下并发安全),与这里的 db 互不干扰。
pub struct AppState {
    /// 主 DB 连接(command 用)。Mutex 因 Tauri command 可能并发调用。
    db: Mutex<rusqlite::Connection>,
    /// 剪贴板监听句柄。drop 即停止监听(app 退出时随 AppState 自动清理)。
    _watcher: clipboard::ClipboardWatcherHandle,
    /// 自身写入待跳过标志(防 copy_back 反馈循环)。
    /// copy_back 写剪贴板前置 true;watcher on_clipboard_change 开头消费(置 false),
    /// 若检测到 true 则跳过本次捕获(这是自身写入,不是用户复制)。
    self_write_flag: Arc<AtomicBool>,
    /// 图片存储根目录(<app_data_dir>/images/)。copy_back/delete 图片时用。
    images_dir: std::path::PathBuf,
    /// 用户设置(M3:快捷键/自启/历史上限/保留天数/容量上限/失焦隐藏)。
    /// Mutex 因 update_settings 写 / get_settings / window_event 读 并发。
    settings: Mutex<settings::Settings>,
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

    /// 图片存储根目录(delete 图片引用计数删除 / copy_back 图片回填用)。
    pub fn images_dir(&self) -> &std::path::Path {
        &self.images_dir
    }

    /// 用户设置的 Mutex 引用(get_settings / update_settings / window_event 用)。
    pub fn settings(&self) -> &Mutex<settings::Settings> {
        &self.settings
    }

    /// 标记「下一次剪贴板变化是自身写入,watcher 应跳过」。
    /// copy_back 写剪贴板前调用。
    pub fn mark_self_write(&self) {
        self.self_write_flag.store(true, Ordering::SeqCst);
    }
}

/// 隐藏主窗口(前端 invoke 入口,走后端统一 guard 管理)。
///
/// 前端回填/Esc 等场景必须走此 command(而非直接 getCurrentWindow().hide()),
/// 以确保 SUPPRESS_BLUR_HIDE guard 被正确置位,防 Focused(false) 连锁(tauri 已知行为)。
#[tauri::command]
fn hide_window(window: Window) {
    window_control::hide_main_window(window.app_handle());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 日志插件:常驻工具排障必备。
            // macOS:~/Library/Logs/{bundleId}/
            // Windows:{FOLDERID_LocalAppData}/{bundleId}/logs/  (即 AppData\Local\...\logs\)
            // Info 级:M1 起剪贴板捕获需要 info 日志观测(每次复制打一条);仍过滤 verbose。
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // 启动日志:确认日志系统正常工作(排障第一线索)
            log::info!("[startup] 拾贴启动中, identifier={}", app.handle().config().identifier);

            // M3:Store 插件(设置持久化,settings.json in app_data_dir)
            app.handle().plugin(tauri_plugin_store::Builder::default().build())?;

            // M3:读取用户设置(首次启动写默认值)
            let user_settings = settings::load_settings(app.handle())
                .unwrap_or_else(|e| {
                    log::warn!("[settings] 加载设置失败,使用默认值: {}", e.message);
                    settings::Settings::default()
                });

            // M3:全局快捷键插件 + toggle 窗口 handler
            // ⚠️ Tauri 2:register() 不再接 handler,handler 在 plugin Builder 的 with_handler 传入
            #[cfg(desktop)]
            {
                let shortcut_str = user_settings.shortcut.clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, _shortcut, event| {
                            // 只响应 Pressed(松手检测做不到,Tauri 全局快捷键仅支持 keydown)
                            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                                window_control::toggle_window(app);
                            }
                        })
                        .build(),
                )?;

                // 注册快捷键(用设置中的值)
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                match app.global_shortcut().register(shortcut_str.as_str()) {
                    Ok(()) => log::info!("[shortcut] 全局快捷键注册成功: {}", shortcut_str),
                    Err(e) => log::error!("[shortcut] 注册快捷键 {} 失败: {e}", shortcut_str),
                }
            }

            // M3:系统托盘(常驻 + 左键 toggle / 右键菜单)
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let show_item = MenuItem::with_id(app, "show", "显示剪贴板", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "退出拾贴", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

                TrayIconBuilder::with_id("main-tray")
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("拾贴")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => window_control::show_main_window(app),
                        "quit" => {
                            // 置标志位:让 ExitRequested 放行,不被 prevent_exit 拦截
                            USER_REQUESTED_EXIT.store(true, Ordering::SeqCst);
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            window_control::toggle_window(tray.app_handle());
                        }
                    })
                    .build(app)?;
            }

            // M3:开机自启插件(LaunchAgent,无权限弹窗)
            // 不在 setup 里强制 enable —— 自启状态由用户在设置面板驱动
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--hidden"]),
            ))?;

            // DB 路径:app_data_dir/clipboard.db
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("拿不到 app_data_dir");
            std::fs::create_dir_all(&data_dir)
                .expect("创建 app_data_dir 失败");
            let db_path = data_dir.join("clipboard.db");

            // 图片存储目录:app_data_dir/images/(M2 阶段 C)
            let images_dir = data_dir.join("images");
            std::fs::create_dir_all(&images_dir)
                .expect("创建 images_dir 失败");

            // 主 DB 连接(command 用)
            let main_conn = storage::init_db(&db_path)
                .expect("主 DB 连接初始化失败");

            // 启动剪贴板监听(后台线程,独立 DB 连接)
            // ⚠️ 不在这里 emit 任何事件:setup 早于 WebView JS,emit 即发即弃无缓冲
            //    (constraints.md「Tauri v2 setup 时序坑」)。
            //    事件在 watcher 回调里发(那时 WebView 已就绪,且由用户复制触发,时序天然正确)。
            let self_write_flag = Arc::new(AtomicBool::new(false));
            let watcher = clipboard::ClipboardWatcherHandle::start(
                app.handle().clone(),
                &db_path,
                &images_dir,
                self_write_flag.clone(),
            )
            .expect("剪贴板监听启动失败");

            app.manage(AppState {
                db: Mutex::new(main_conn),
                _watcher: watcher,
                self_write_flag,
                images_dir,
                settings: Mutex::new(user_settings),
            });

            // M3:根据 launch_visible 设置决定是否显示窗口
            // (tauri.conf.json visible:false,由代码控制显示时机)
            let launch_visible = app
                .try_state::<AppState>()
                .map(|s| {
                    s.settings()
                        .lock()
                        .map(|gs| gs.launch_visible)
                        .unwrap_or(true)
                })
                .unwrap_or(true);

            // --hidden 启动参数(开机自启时)强制不显示,覆盖 launch_visible
            let hidden_flag = std::env::args().any(|a| a == "--hidden");
            if launch_visible && !hidden_flag {
                window_control::show_main_window(app.handle());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            list_items,
            copy_back,
            delete_item,
            search_items,
            get_settings,
            update_settings,
            get_storage_stats,
            clear_history,
            hide_window,
        ])
        // M3:窗口事件 — 失焦隐藏 + close 拦截转 hide
        .on_window_event(|window, event| {
            window_control::handle_window_event(event, window.app_handle());
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // M3:拦截退出请求 → 保持进程常驻(末窗口关闭不应退出 app)
            // 但用户从托盘菜单「退出拾贴」时放行(USER_REQUESTED_EXIT 标志位)
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if USER_REQUESTED_EXIT.load(Ordering::SeqCst) {
                    // 用户主动退出,放行
                    USER_REQUESTED_EXIT.store(false, Ordering::SeqCst);
                } else {
                    // 窗口关闭触发的退出请求,拦截 → 保持常驻
                    api.prevent_exit();
                }
            }
        });
}
