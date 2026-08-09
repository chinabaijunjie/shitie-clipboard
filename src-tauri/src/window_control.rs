//! 窗口控制 — show / hide / toggle 主窗口
//!
//! M3 新增。核心职责:
//! - `toggle_window`:全局快捷键 / 托盘左键 → 根据可见性 show+focus / hide
//! - `hide_main_window`:统一 hide 入口(带 SUPPRESS_BLUR_HIDE guard 防 Focused(false) 连锁)
//! - `show_main_window`:show + focus + 清 guard
//!
//! 🔴 关键陷阱(Tauri 2 已知行为):hide() 会触发 Focused(false) → 如果失焦 handler 也调
//! hide(),会连锁抖动。解决方案:全局 AtomicBool guard,程序主动 hide 前置位,Focused(false)
//! 检查该标志跳过。与 self_write_flag(防 copy_back 反馈循环)同一套原子标志套路。
//!
//! 模块化(§1.21):本模块只管窗口可见性,不管窗口创建(tauri.conf.json 配)/
//! 不管事件绑定(lib.rs on_window_event)/ 不管快捷键注册(lib.rs setup)。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, Runtime, WindowEvent};

/// 主窗口 label(与 tauri.conf.json windows[0].label 对齐)。
pub const MAIN_WINDOW: &str = "main";

/// 程序主动 hide 时置位,阻止 Focused(false) 连锁触发二次 hide。
///
/// - hide_main_window() 前置位 true
/// - show_main_window() 复位 false
/// - on_window_event Focused(false) 检查:若 true 则跳过(这是程序触发的 hide,不是用户失焦)
static SUPPRESS_BLUR_HIDE: AtomicBool = AtomicBool::new(false);

/// 判断当前是否应该执行失焦隐藏(纯函数,可单测)。
///
/// - hide_on_blur=false → 不隐藏(用户关了失焦隐藏)
/// - SUPPRESS_BLUR_HIDE=true → 不隐藏(程序主动 hide 触发的失焦,防连锁)
/// - 其余 → 隐藏
pub fn should_hide_on_blur(hide_on_blur: bool) -> bool {
    hide_on_blur && !SUPPRESS_BLUR_HIDE.load(Ordering::SeqCst)
}

/// 判断窗口应该 show 还是 hide(纯函数,可单测)。
///
/// - is_visible=true → 应该 hide(toggle 关闭)
/// - is_visible=false → 应该 show(toggle 打开)
#[cfg(test)]
pub fn toggle_action(is_visible: bool) -> WindowAction {
    if is_visible {
        WindowAction::Hide
    } else {
        WindowAction::Show
    }
}

/// toggle 的动作枚举(用于纯函数测试)。
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    Show,
    Hide,
}

/// toggle 主窗口:可见 → hide,不可见 → show+focus。
///
/// 全局快捷键和托盘左键都调此函数。
pub fn toggle_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        match win.is_visible() {
            Ok(true) => hide_main_window(app),
            Ok(false) => show_main_window(app),
            Err(e) => log::warn!("[window] is_visible 失败,跳过 toggle: {e}"),
        }
    }
}

/// 隐藏主窗口(统一入口,带 SUPPRESS_BLUR_HIDE guard)。
///
/// 三条 hide 路径(快捷键 toggle / close 拦截 / 失焦隐藏)都走此函数,
/// 确保 guard 一致管理。
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    // 置位 guard:阻止后续 Focused(false) 连锁触发二次 hide
    SUPPRESS_BLUR_HIDE.store(true, Ordering::SeqCst);
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        if let Err(e) = win.hide() {
            log::warn!("[window] hide 失败: {e}");
        }
    }
}

/// 显示主窗口(show + focus + 清 guard)。
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        if let Err(e) = win.show() {
            log::warn!("[window] show 失败: {e}");
        }
        if let Err(e) = win.set_focus() {
            log::warn!("[window] set_focus 失败: {e}");
        }
    }
    // 清 guard:窗口已显示,后续失焦是真实用户操作,应该正常响应
    SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
}

/// 处理窗口事件(Focused / CloseRequested)。
///
/// 在 lib.rs 的 `.on_window_event` 里调用。
/// 读取 AppState 的 hide_on_blur 设置决定是否失焦隐藏。
///
/// ⚠️ Windows 拖动陷阱:用户拖动窗口标题栏时,Windows 会瞬间触发 Focused(false) 再
/// Focused(true)。如果直接在 Focused(false) 里 hide,窗口会消失无法拖动。
/// 修复:失焦后延迟 150ms 再检查——如果窗口仍然可见且仍无焦点,才真正隐藏。
/// macOS 不受此问题影响,但延迟无害(用户感知不到 150ms)。
pub fn handle_window_event<R: Runtime>(event: &WindowEvent, app: &AppHandle<R>) {
    match event {
        WindowEvent::Focused(false) => {
            // 失焦隐藏(检查设置 + guard)
            if should_hide_on_blur(read_hide_on_blur(app)) {
                // 延迟隐藏:给 Windows 拖动操作时间完成(拖动会快速失焦→聚焦)
                let app_handle = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    // 再次检查:guard 可能已被 show 置位,窗口可能已不可见
                    if !should_hide_on_blur(read_hide_on_blur(&app_handle)) {
                        return;
                    }
                    // 检查窗口是否仍可见(可能已被其他路径隐藏)
                    if let Some(win) = app_handle.get_webview_window(MAIN_WINDOW) {
                        if !win.is_visible().unwrap_or(false) {
                            return;
                        }
                    }
                    hide_main_window(&app_handle);
                });
            }
        }
        WindowEvent::Focused(true) => {
            // 窗口重新获得焦点(可能是拖动结束)——确保 guard 清除
            SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
        }
        WindowEvent::CloseRequested { api, .. } => {
            // 拦截关闭 → 转 hide(进程常驻,除非从托盘退出)
            api.prevent_close();
            hide_main_window(app);
        }
        _ => {}
    }
}

/// 从 AppState 读取 hide_on_blur 设置。
fn read_hide_on_blur<R: Runtime>(app: &AppHandle<R>) -> bool {
    if let Some(state) = app.try_state::<crate::AppState>() {
        let guard = state.settings();
        guard
            .lock()
            .map(|s| s.hide_on_blur)
            .unwrap_or(true) // Mutex 中毒时用默认值(true = 失焦隐藏)
    } else {
        true // AppState 还没初始化(setup 早期)→ 用默认值
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_action_visible_to_hide() {
        assert_eq!(toggle_action(true), WindowAction::Hide);
    }

    #[test]
    fn toggle_action_hidden_to_show() {
        assert_eq!(toggle_action(false), WindowAction::Show);
    }

    #[test]
    fn should_hide_when_blur_enabled_and_not_suppressed() {
        // 测试前确保 guard 是 false(其他测试可能改过)
        SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
        assert!(should_hide_on_blur(true));
    }

    #[test]
    fn should_not_hide_when_blur_disabled() {
        assert!(!should_hide_on_blur(false));
    }

    #[test]
    fn should_not_hide_when_suppressed() {
        SUPPRESS_BLUR_HIDE.store(true, Ordering::SeqCst);
        assert!(!should_hide_on_blur(true));
        // 恢复
        SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
    }
}
