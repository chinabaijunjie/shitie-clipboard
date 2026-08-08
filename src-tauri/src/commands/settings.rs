//! 设置 command — 暴露给前端 invoke 的设置读写接口
//!
//! M3 新增。两层结构(与 clipboard.rs 一致):
//! - 核心逻辑(接 AppHandle/State):可测
//! - command 包装(接 State):薄转发
//!
//! command:
//! - get_settings:读当前设置
//! - update_settings:写设置 + 联动(改快捷键→unregister/register,改自启→enable/disable)

use tauri::State;

use crate::error::{codes, AppError};
use crate::settings::{self, Settings};

/// 读当前设置。
#[tauri::command]
pub fn get_settings(state: State<'_, crate::AppState>) -> Result<Settings, AppError> {
    let settings = state.settings();
    Ok(settings.lock().expect("Settings Mutex 中毒").clone())
}

/// 更新设置(完整替换语义)。
///
/// 联动副作用:
/// - shortcut 变化 → unregister 旧快捷键 + register 新快捷键
/// - autostart 由前端直接调 autostart 插件(不走 update_settings),此处不管
///
/// ⚠️ 快捷键 register 失败时回滚 settings(保持旧快捷键)+ 返回错误。
#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    new_settings: Settings,
) -> Result<Settings, AppError> {
    // 读旧设置(用于检测 shortcut 变化)
    let old_settings = {
        let guard = state.settings();
        guard.lock().expect("Settings Mutex 中毒").clone()
    };

    // 快捷键变化时先注销旧的 + 注册新的(失败则回滚)
    let shortcut_changed = old_settings.shortcut != new_settings.shortcut;
    if shortcut_changed {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let gs = app.global_shortcut();

        // 注销旧快捷键(忽略错误:旧快捷键可能已被系统其他 app 占用导致未注册成功)
        let _ = gs.unregister(old_settings.shortcut.as_str());

        // 注册新快捷键
        gs.register(new_settings.shortcut.as_str()).map_err(|e| {
            // 注册失败:尝试恢复旧快捷键
            let _ = gs.register(old_settings.shortcut.as_str());
            AppError::new(
                codes::SHORTCUT_REGISTER_FAILED,
                format!("注册快捷键 {} 失败: {e}", new_settings.shortcut),
            )
        })?;
    }

    // 持久化到 settings.json
    settings::save_settings(&app, &new_settings)?;

    // 更新内存中的设置
    {
        let guard = state.settings();
        *guard.lock().expect("Settings Mutex 中毒") = new_settings.clone();
    }

    log::info!(
        "[settings] 更新设置: shortcut={} launch_visible={} max_items={} retention_days={} max_storage_mb={} hide_on_blur={}",
        new_settings.shortcut,
        new_settings.launch_visible,
        new_settings.max_items,
        new_settings.retention_days,
        new_settings.max_storage_mb,
        new_settings.hide_on_blur
    );

    Ok(new_settings)
}
