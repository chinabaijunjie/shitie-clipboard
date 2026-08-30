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
#[cfg(target_os = "macos")]
mod macos_panel;
mod settings;
mod storage;
mod window_control;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use commands::clipboard::{clear_history, copy_back, delete_item, get_storage_stats, list_items, search_items, toggle_pin};
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

/// 单实例锁 key 的标识名。
const SINGLE_INSTANCE_LOCK_NAME: &str = "shitie-clipboard-lock";

/// 单实例锁的 key(平台差异,见 `run()` 内注释)。
///
/// - macOS:**必须是绝对路径**——single-instance crate 在 macOS 上把参数当文件路径
///   直接 flock。相对路径会落到进程 CWD:launchd(开机自启 LaunchAgent)启动的进程
///   CWD=/,普通用户无权在根目录创建文件 → 锁初始化失败 → 多开保护整个失效
///   (v2.0.1 及之前 macOS 从未真正拦截过多开,开机自启 + 会话恢复双实例并存的根因)。
///   temp_dir 是 per-user 稳定路径,launchd / 会话恢复 / 手动启动一致可见。
/// - Windows:参数是命名互斥体名,**不能含反斜杠**(temp 路径含 `\` 会让 CreateMutex
///   失败),用纯标识符(互斥体本身按名全局互斥,无需路径)。
/// - Linux:参数是 abstract unix socket 名,任意字符串,沿用标识符。
fn single_instance_key() -> String {
    if cfg!(target_os = "macos") {
        std::env::temp_dir()
            .join(SINGLE_INSTANCE_LOCK_NAME)
            .to_string_lossy()
            .into_owned()
    } else {
        SINGLE_INSTANCE_LOCK_NAME.to_string()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 单实例保护:防止多个进程同时运行(开机自启 LaunchAgent + macOS 会话恢复 +
    // 手动启动叠加场景)。第二个实例启动时直接退出自己。
    // (注:不激活已有实例的窗口——窗口激活需跨进程通信,v2.0.2 暂不做)
    let lock_init_error: Option<String>;
    let _single_instance = match single_instance::SingleInstance::new(&single_instance_key()) {
        Ok(si) => {
            if !si.is_single() {
                eprintln!("拾贴已在运行,退出重复实例");
                return;
            }
            lock_init_error = None;
            Some(si)
        }
        Err(e) => {
            // 锁失败不拒绝启动(可用性优先),但必须留痕——此时本次启动无多开保护。
            // eprintln 在 GUI 进程不可见,真正落盘的日志在 setup 里日志插件就绪后补记。
            lock_init_error = Some(format!("{e}"));
            None
        }
    };

    tauri::Builder::default()
        .setup(move |app| {
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

            // 单实例锁初始化失败补记(GUI 进程 stderr 不可见,此处才真正落盘可排查)
            if let Some(e) = lock_init_error {
                log::warn!("[startup] 单实例锁初始化失败,本次启动无多开保护: {e}");
            }

            // M3:Store 插件(设置持久化,settings.json in app_data_dir)
            app.handle().plugin(tauri_plugin_store::Builder::default().build())?;

            // macOS:tauri-nspanel plugin 注册(⚠️ 时序红线:必须在 setup_panel/to_panel 之前)。
            // 注册全局面板表 WebviewPanelManager,晚于 to_panel 会 panic。调研见 fullscreen-research.md。
            #[cfg(target_os = "macos")]
            macos_panel::register_plugin(app.handle());

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
            // ⚠️ 仅 release 构建注册:autostart 记录的是「当前可执行文件路径」,
            //    dev 构建即 target/debug 调试产物——若在 tauri dev 下开启自启,
            //    登录项会永久指向调试二进制,之后每次开机拉起旧调试版与正式版并存
            //    (v2.0.2 前用户机器上的幽灵多实例事故)。前端设置面板在 dev 下
            //    同步禁用开关,此处为后端兜底。
            #[cfg(not(debug_assertions))]
            {
                app.handle().plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    Some(vec!["--hidden"]),
                ))?;
            }

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

            // macOS:主窗口转 NSPanel 子类(全屏悬浮支持)
            // 把 tao 默认的 NSWindow 通过 isa-swizzle 转成 NSPanel,获得 nonactivatingPanel 能力。
            // 必须在窗口创建后、show 之前。调研见 fullscreen-research.md。
            #[cfg(target_os = "macos")]
            macos_panel::setup_panel(app.handle());

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
            toggle_pin,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁 key 的平台形态(v2.0.2 修复回归):
    /// - macOS 必须精确落在 per-user temp_dir 下——相对路径在 launchd 的 CWD=/
    ///   下无权创建,锁整个失效;非 temp_dir 的绝对路径(如根目录)同样不可靠,
    ///   故直接断言完整路径
    /// - 其他平台保持纯标识符(Windows 互斥体名不能含反斜杠)
    #[test]
    fn single_instance_key_platform_shape() {
        let key = single_instance_key();
        #[cfg(target_os = "macos")]
        {
            let expected = std::env::temp_dir()
                .join(SINGLE_INSTANCE_LOCK_NAME)
                .to_string_lossy()
                .into_owned();
            assert_eq!(key, expected, "macOS 锁 key 必须是 temp_dir 下的绝对路径");
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(key, SINGLE_INSTANCE_LOCK_NAME);
            assert!(
                !key.contains('\\') && !key.contains('/'),
                "互斥体名不能含路径分隔符: {key}"
            );
        }
    }

    /// 同 key 连续创建两个 SingleInstance,第二个必须拿不到锁。
    /// v2.0.1 及之前 macOS 上锁从未真正互斥(flock 文件落在 CWD 创建失败),
    /// 本测试守护「锁在真实文件系统上生效」这一行为。
    /// ⚠️ 用带 pid 后缀的测试专用 key,不碰生产锁——若跑测试时正式版应用正在
    ///    运行(同用户 temp_dir),生产 flock 已被持有,第一行断言会假失败。
    #[test]
    fn single_instance_lock_actually_dedupes() {
        let test_key = format!("{}.test-{}", single_instance_key(), std::process::id());
        let a = single_instance::SingleInstance::new(&test_key).expect("创建第一个锁实例");
        assert!(a.is_single(), "第一个实例应持有锁");
        let b = single_instance::SingleInstance::new(&test_key).expect("创建第二个锁实例");
        assert!(!b.is_single(), "同 key 第二个实例应被视为重复进程");
    }
}
