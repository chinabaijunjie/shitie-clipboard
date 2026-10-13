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

// ============================================================
// 输入源自动切换(拼音直搜配套)
// ============================================================
//
// 面板显示 → 切到 ABC(英文)布局:搜索框直接接收原始字母,配合后端拼音直搜
// (pinyin_full/pinyin_initials)完成中文内容检索——全屏 Space 下系统输入法
// 候选窗不渲染的问题由此彻底绕开。面板隐藏 → 恢复用户原输入源。

/// 面板打开期间保存的用户原输入源 ID
static SAVED_INPUT_SOURCE_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

const ABC_INPUT_SOURCE_ID: &str = "com.apple.keylayout.ABC";
const K_CF_STRING_UTF8: u32 = 0x08000100;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceID: *const std::ffi::c_void;
    fn TISCreateInputSourceList(filterProperties: *const std::ffi::c_void, includeAllInstalled: u8) -> *const std::ffi::c_void;
    fn TISGetInputSourceProperty(source: *const std::ffi::c_void, propertyKey: *const std::ffi::c_void) -> *const std::ffi::c_void;
    fn TISSelectInputSource(source: *const std::ffi::c_void) -> i32;
    fn TISCopyCurrentKeyboardInputSource() -> *const std::ffi::c_void;
    fn CFStringCreateWithCString(alloc: *const std::ffi::c_void, cStr: *const std::ffi::c_char, encoding: u32) -> *const std::ffi::c_void;
    fn CFStringGetLength(string: *const std::ffi::c_void) -> isize;
    fn CFStringGetCString(string: *const std::ffi::c_void, buffer: *mut std::ffi::c_char, bufferSize: isize, encoding: u32) -> u8;
    fn CFArrayGetCount(array: *const std::ffi::c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const std::ffi::c_void, index: isize) -> *const std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Rust String → CFString(调用方负责释放)
unsafe fn cf_string(s: &str) -> *const std::ffi::c_void {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_UTF8)
}

/// CFString → Rust String
unsafe fn cf_to_string(s: *const std::ffi::c_void) -> String {
    if s.is_null() {
        return String::new();
    }
    let len = CFStringGetLength(s);
    let max = len * 4 + 1;
    let mut buf = vec![0u8; max as usize];
    let ok = CFStringGetCString(s, buf.as_mut_ptr() as *mut std::ffi::c_char, max, K_CF_STRING_UTF8);
    if ok == 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// 按输入源 ID 切换(CR 修复:CF 对象即用即释,Find+Select 合一避免跨函数持有借来指针)。
/// 返回是否成功找到并切换。
fn select_enabled_input_source(id: &str) -> bool {
    unsafe {
        // Create 规则:list 为 +1 引用,用完必须 CFRelease
        let list = TISCreateInputSourceList(std::ptr::null(), 0);
        if list.is_null() {
            log::warn!("[input-source] TISCreateInputSourceList 返回空,无法按 ID 查找: {id}");
            return false;
        }
        let id_cf = cf_string(id);
        let count = CFArrayGetCount(list);
        let mut found = false;
        for i in 0..count {
            let src = CFArrayGetValueAtIndex(list, i);
            let src_id = cf_to_string(TISGetInputSourceProperty(src, kTISPropertyInputSourceID as *const _));
            if src_id == id {
                found = TISSelectInputSource(src) == 0;
                if !found {
                    log::warn!("[input-source] 切到 {id} 失败: TISSelectInputSource 非 0");
                }
                break;
            }
        }
        CFRelease(id_cf);
        CFRelease(list);
        if !found {
            log::warn!("[input-source] 未在启用列表中找到输入源: {id}");
        }
        found
    }
}

/// 保存当前输入源并切到 ABC(面板显示时调用;主线程)
fn switch_to_abc_and_save() {
    unsafe {
        // 已保存过(面板重复 show)不覆盖用户原输入源
        if SAVED_INPUT_SOURCE_ID.lock().unwrap().is_some() {
            return;
        }
        // Copy 规则:current 为 +1 引用,读取属性后 CFRelease
        let current = TISCopyCurrentKeyboardInputSource();
        let current_id = cf_to_string(TISGetInputSourceProperty(current, kTISPropertyInputSourceID as *const _));
        CFRelease(current);
        if current_id.is_empty() {
            // CR 修复:读取失败(空串)时仍保存占位,避免 restore 阶段误把 ABC 当作用户原输入源
            log::warn!("[input-source] 读取当前输入源 ID 失败,保存占位符以防误恢复");
            *SAVED_INPUT_SOURCE_ID.lock().unwrap() = Some(String::new());
        } else {
            *SAVED_INPUT_SOURCE_ID.lock().unwrap() = Some(current_id.clone());
        }
        // 切到 ABC(内部完成 CF 释放与失败日志)
        select_enabled_input_source(ABC_INPUT_SOURCE_ID);
    }
}

/// 恢复用户原输入源(面板隐藏时调用;主线程)
fn restore_input_source() {
    let saved = SAVED_INPUT_SOURCE_ID.lock().unwrap().take();
    if let Some(id) = saved {
        // 空串 = show 时读取失败占位(无法可靠恢复,保持 ABC 并留日志)
        if !id.is_empty() && id != ABC_INPUT_SOURCE_ID {
            select_enabled_input_source(&id);
        }
    }
}

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
                // ⭐ 激活 app 进程(仅进程级激活,不对普通窗口 orderFront)。
                // 原因:输入法候选词浮窗是输入法进程的窗口,不带 fullScreenAuxiliary,
                // 只跟随「激活 app 的 key window 所在 Space」渲染。app 不激活时,
                // 全屏 Space 下打字候选窗完全不显示(2026-10-13 实测:
                // 组合下划线在,CGWindowList 里无候选窗)。激活后候选窗跟随面板
                // 所在 Space 正常显示;面板本身是 auxiliary,不会挤走全屏应用
                // (Raycast 同款行为:全屏唤出时菜单栏切到面板 app)。
                activate_app();
                // 拼音直搜配套:切到 ABC 布局,搜索框直接接收原始字母
                // (全屏 Space 下系统输入法候选窗不渲染,拼音查询在 ABC 下完成)
                switch_to_abc_and_save();
            } else {
                log::warn!("[panel] show: 找不到 panel(未 setup?)");
            }
        });
    });
}

/// 激活 app 进程(macOS;进程级,不动窗口)。
fn activate_app() {
    use tauri_nspanel::objc2::MainThreadMarker;
    use tauri_nspanel::objc2_app_kit::NSApplication;
    unsafe {
        let mtm = MainThreadMarker::new().expect("activate_app 必须在主线程");
        let app = NSApplication::sharedApplication(mtm);
        // 旧 API(deprecated 但语义最明确):忽略其他应用强制激活。
        // 新 activate() 是协作式的,从非前台进程调用可能被系统拒绝。
        let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
    }
}

/// hide panel(主线程 + 切回 move_to_active_space + 恢复用户输入源)。
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
        // 拼音直搜配套:恢复用户原输入源
        restore_input_source();
    });
}
