//! macOS NSPanel 集成 —— 全屏 Space 悬浮支持。
//!
//! 主窗口在 setup 阶段从 NSWindow（tao 默认）转成 NSPanel 子类，
//! 获得 nonactivatingPanel + canBecomeKeyWindow 能力，能悬浮在全屏应用之上。
//!
//! 选型调研见 wiki/dev/fullscreen-research.md。
//! 参考实现:EcoPaste(7283 star 同类剪贴板)src-tauri/src/window/macos.rs。
//!
//! 模块化(§1.21):本模块只管 NSPanel 转换/配置/show/hide,
//! 不管可见性决策(window_control.rs)/ 不管事件绑定(lib.rs)。

#![cfg(target_os = "macos")]
// panel_event! 宏要求每个方法显式写 `-> ()` 返回类型(宏解析必需),
// clippy::unused_unit 对此误报,模块级 allow 消除。
#![allow(clippy::unused_unit)]

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use tauri_nspanel::objc2_app_kit::{
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

use crate::window_control::MAIN_WINDOW;

/// show 前延迟(等窗口几何应用完,避免闪烁。EcoPaste 经验值)。
const PANEL_SHOW_DELAY: Duration = Duration::from_millis(16);

// 定义 NSPanel 子类 + 失焦 delegate。
//
// - `can_become_key_window: true` — NSPanel 默认返回 NO,必须 override 成 true 才能接收键盘
//   (搜索框/列表导航需要)。这是 NSPanel 子类化的核心价值。
// - `can_become_main_window: false` — 不成为 main window,app 激活状态不转移,
//   前台 app(全屏的那个)保持激活。
// - `is_floating_panel: true` — 浮在普通窗口之上。
//
// ⚠️ `tauri_panel!` 宏自动注入 NSNotification/NSWindow 等类型,块内直接用裸名即可。
tauri_panel! {
    panel!(MainPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })

    panel_event!(MainPanelEventHandler {
        // 失焦回调:panel 失去 key 状态时触发(用户点别的窗口/切 app)。
        // 替代 Tauri 的 Focused(false) 事件(NSPanel 下不可靠)。
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

/// setup 最早阶段:注册 plugin(⚠️ 必须在 to_panel 之前)。
///
/// `init()` 注册 WebviewPanelManager(全局面板表)。若晚于 to_panel,
/// `to_panel` 内部 `state::<WebviewPanelManager>()` 会 panic。
pub fn register_plugin<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.plugin(tauri_nspanel::init()) {
        log::error!("[panel] 注册 tauri-nspanel plugin 失败: {e}");
    }
}

/// setup 末尾:WebviewWindow → NSPanel + 配置 + 绑失焦回调。
pub fn setup_panel<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window(MAIN_WINDOW) else {
        log::warn!("[panel] 找不到主窗口,跳过 NSPanel 转换");
        return;
    };

    // ⭐ 核心步骤:isa-swizzle,把 TaoWindow(NSWindow) 转成 MainPanel(NSPanel)
    // MainPanel<R> 带泛型(默认 Wry),显式传 R 匹配 FromWindow<R> for MainPanel<R>
    let panel = match win.to_panel::<MainPanel<R>>() {
        Ok(p) => p,
        Err(e) => {
            log::error!("[panel] to_panel 失败: {e:?}");
            return;
        }
    };

    panel.set_level(PanelLevel::Dock.value()); // 20,高于普通窗口但不盖菜单栏
    panel.set_style_mask({
        // nonactivating_panel 是关键:不抢 app 焦点(仅对 NSPanel 子类生效)
        let mask: NSWindowStyleMask = StyleMask::empty()
            .resizable()
            .nonactivating_panel()
            .into();
        mask
    });
    panel.set_collection_behavior({
        // 初始 hide 态:move_to_active_space(下次 show 跟到当前 space)
        let behavior: NSWindowCollectionBehavior = CollectionBehavior::new()
            .stationary()
            .move_to_active_space()
            .full_screen_auxiliary() // ⭐ 允许在全屏 Space 显示
            .into();
        behavior
    });

    // 绑失焦回调(delegate 模式,替代 Tauri Focused(false))
    let handler = MainPanelEventHandler::new();
    let resign_handle = app.clone();
    handler.window_did_resign_key(move |_notification| {
        // 复用 window_control 的统一 hide 入口(guard 一致管理)
        let hide_on_blur = crate::window_control::read_hide_on_blur_pub(&resign_handle);
        if crate::window_control::should_hide_on_blur(hide_on_blur) {
            crate::window_control::hide_main_window(&resign_handle);
        }
    });
    panel.set_event_handler(Some(handler.as_ref()));

    log::info!("[panel] NSPanel 转换 + 配置完成(level=Dock, nonactivating, fullScreenAuxiliary)");
}

/// show panel(主线程 + 16ms 延迟 + 切 collectionBehavior 跟随当前 Space)。
///
/// ⚠️ 用 `show_and_make_key()`(内部 orderFrontRegardless,不激活 app),
/// 绝不能用原 `set_focus()`(底层 makeKeyAndOrderFront + activateIgnoringOtherApps:YES,
/// 会激活 app → 切到 app 的 Space → 全屏应用被挤走)。
pub fn show_panel<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    // 快捷键回调可能在非主线程;panel 操作必须在主线程
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PANEL_SHOW_DELAY).await;
        let panel_handle = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if let Ok(panel) = panel_handle.get_webview_panel(MAIN_WINDOW) {
                panel.show_and_make_key();
                // show 时切到 can_join_all_spaces:显示瞬间出现在用户当前 Space
                let behavior: NSWindowCollectionBehavior = CollectionBehavior::new()
                    .stationary()
                    .can_join_all_spaces()
                    .full_screen_auxiliary()
                    .into();
                panel.set_collection_behavior(behavior);
            } else {
                log::warn!("[panel] show: 找不到 panel(未 setup?)");
            }
        });
    });
}

/// hide panel(主线程 + 切回 move_to_active_space)。
pub fn hide_panel<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let inner = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Ok(panel) = inner.get_webview_panel(MAIN_WINDOW) {
            panel.hide();
            // hide 后切回 move_to_active_space:下次 show 按当前 Space 落位
            let behavior: NSWindowCollectionBehavior = CollectionBehavior::new()
                .stationary()
                .move_to_active_space()
                .full_screen_auxiliary()
                .into();
            panel.set_collection_behavior(behavior);
        }
    });
}
