//! 设置持久化层 — 用户可配置项的读写
//!
//! M3 新增。使用 tauri-plugin-store(settings.json in app_data_dir)持久化。
//! 与 rusqlite(历史数据)职责分离。
//!
//! 配置项(8 项):
//! - shortcut:全局唤起快捷键(默认 "Cmd+Shift+V")
//! - launch_visible:启动时打开窗口(默认 true)
//! - max_items:历史记录上限(默认 50)
//! - retention_days:保留天数,0=不限(默认 0)
//! - max_storage_mb:容量上限 MB,0=不限(默认 0)
//! - hide_on_blur:失焦自动隐藏(默认 true)
//! - enable_sensitive_filter:敏感内容过滤(默认 true,M4 新增)
//! - theme:界面配色主题(默认 "dark",多主题切换新增)
//!
//! autostart 状态由 tauri-plugin-autostart 管理(写 LaunchAgent plist),不进 settings.json。
//!
//! 模块化(§1.21):本模块只管读写,不管 UI(那是 SettingsPanel 的事)也不管副作用
//! (改快捷键时的 register/unregister 在 commands/settings.rs 的 update_settings 里做)。

use serde::{Deserialize, Serialize};

use crate::error::{codes, AppError};

/// 默认快捷键(macOS ⌘⇧V)。
pub const DEFAULT_SHORTCUT: &str = "Cmd+Shift+V";

/// 默认历史记录上限。
pub const DEFAULT_MAX_ITEMS: i64 = 50;

/// settings.json 文件名(放 app_data_dir)。
pub const SETTINGS_FILE: &str = "settings.json";

/// 用户可配置项。
///
/// ⚠️ 前端 `src/core/settings/models.ts` 的 SettingsSchema(Zod)与此镜像。
/// 字段增删必须两端同步。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// 全局唤起快捷键(如 "Cmd+Shift+V")。
    #[serde(rename = "shortcut", default = "default_shortcut")]
    pub shortcut: String,
    /// 启动时是否打开窗口(开机自启场景建议 false)。
    #[serde(rename = "launchVisible", default = "default_launch_visible")]
    pub launch_visible: bool,
    /// 历史记录条数上限。
    #[serde(rename = "maxItems", default = "default_max_items")]
    pub max_items: i64,
    /// 保留天数(0 = 不限),超过自动清理。
    #[serde(rename = "retentionDays", default)]
    pub retention_days: i64,
    /// 容量上限 MB(0 = 不限),超过从最旧开始清理。
    #[serde(rename = "maxStorageMb", default)]
    pub max_storage_mb: i64,
    /// 失焦自动隐藏窗口(Spotlight 标准行为)。
    #[serde(rename = "hideOnBlur", default = "default_hide_on_blur")]
    pub hide_on_blur: bool,
    /// 敏感内容过滤(检测 API key/token/私钥,命中则跳过入库)。
    #[serde(rename = "enableSensitiveFilter", default = "default_enable_sensitive_filter")]
    pub enable_sensitive_filter: bool,
    /// 界面配色主题("dark" / "light" / "mid")。
    #[serde(rename = "theme", default = "default_theme")]
    pub theme: String,
}

fn default_shortcut() -> String {
    DEFAULT_SHORTCUT.to_string()
}
fn default_launch_visible() -> bool {
    true
}
fn default_max_items() -> i64 {
    DEFAULT_MAX_ITEMS
}
fn default_hide_on_blur() -> bool {
    true
}
fn default_enable_sensitive_filter() -> bool {
    true
}
fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: default_shortcut(),
            launch_visible: default_launch_visible(),
            max_items: default_max_items(),
            retention_days: 0,
            max_storage_mb: 0,
            hide_on_blur: default_hide_on_blur(),
            enable_sensitive_filter: default_enable_sensitive_filter(),
            theme: default_theme(),
        }
    }
}

/// 把部分更新(patch)合并到现有 Settings 上(不可变更新:clone + 替换)。
///
/// 前端传完整 Settings JSON,serde 反序列化成新 Settings 后直接替换。
/// 此函数用于测试和手动合并场景。
#[cfg(test)]
pub fn merge_settings(base: &Settings, patch: &Settings) -> Settings {
    patch.clone()
}

/// 从 serde_json::Value 反序列化 Settings,缺字段用默认值兜底。
///
/// Store 插件返回的是 `serde_json::Value`,其中可能缺部分键(老版本 settings.json)。
/// serde 的 `#[serde(default)]` / `#[serde(default = "...")]` 属性确保缺字段时用默认值。
/// 类型不匹配(损坏的 settings.json)时 log::warn 后用默认值兜底。
pub fn settings_from_value(value: &serde_json::Value) -> Settings {
    match serde_json::from_value(value.clone()) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[settings] settings.json 解析失败,用默认值: {e}");
            Settings::default()
        }
    }
}

/// 从 Store 读取 Settings。
///
/// 内部用 tauri-plugin-store 的 StoreExt trait。
/// 若 settings.json 不存在或解析失败,返回默认值(首次启动/文件损坏兜底)。
pub fn load_settings(app: &tauri::AppHandle) -> Result<Settings, AppError> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(SETTINGS_FILE)
        .map_err(|e| AppError::new(codes::SETTINGS_LOAD_FAILED, format!("打开 store 失败: {e}")))?;

    // store.entries() 或 store.get("settings") — 我们用单 key "settings" 存整个对象
    match store.get("settings") {
        Some(value) => Ok(settings_from_value(&value)),
        None => {
            // 首次启动:写入默认值,后续读取不再走这个分支
            let default = Settings::default();
            save_settings(app, &default)?;
            Ok(default)
        }
    }
}

/// 把 Settings 写入 Store 并落盘。
pub fn save_settings(app: &tauri::AppHandle, settings: &Settings) -> Result<(), AppError> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(SETTINGS_FILE)
        .map_err(|e| AppError::new(codes::SETTINGS_UPDATE_FAILED, format!("打开 store 失败: {e}")))?;

    let value = serde_json::to_value(settings)
        .map_err(|e| AppError::new(codes::SETTINGS_UPDATE_FAILED, format!("序列化设置失败: {e}")))?;

    store.set("settings", value);
    store
        .save()
        .map_err(|e| AppError::new(codes::SETTINGS_UPDATE_FAILED, format!("保存设置失败: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let s = Settings::default();
        assert_eq!(s.shortcut, "Cmd+Shift+V");
        assert!(s.launch_visible);
        assert_eq!(s.max_items, 50);
        assert_eq!(s.retention_days, 0);
        assert_eq!(s.max_storage_mb, 0);
        assert!(s.hide_on_blur);
        assert!(s.enable_sensitive_filter);
        assert_eq!(s.theme, "dark");
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let s = Settings {
            shortcut: "Cmd+Shift+Q".into(),
            launch_visible: false,
            max_items: 100,
            retention_days: 30,
            max_storage_mb: 200,
            hide_on_blur: false,
            enable_sensitive_filter: false,
            theme: "light".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn serde_uses_camel_case() {
        let s = Settings::default();
        let json = serde_json::to_value(&s).unwrap();
        let obj = json.as_object().unwrap();
        // 确认 key 是 camelCase(与前端对齐)
        assert!(obj.contains_key("launchVisible"));
        assert!(obj.contains_key("maxItems"));
        assert!(obj.contains_key("retentionDays"));
        assert!(obj.contains_key("maxStorageMb"));
        assert!(obj.contains_key("hideOnBlur"));
        assert!(obj.contains_key("enableSensitiveFilter"));
        assert!(obj.contains_key("theme"));
        // 不含 snake_case
        assert!(!obj.contains_key("launch_visible"));
        assert!(!obj.contains_key("max_items"));
        assert!(!obj.contains_key("enable_sensitive_filter"));
    }

    #[test]
    fn from_value_missing_fields_uses_defaults() {
        // 空对象 → 全部默认值
        let empty = serde_json::json!({});
        let s = settings_from_value(&empty);
        assert_eq!(s.shortcut, "Cmd+Shift+V");
        assert!(s.launch_visible);
        assert_eq!(s.max_items, 50);
        assert!(s.hide_on_blur);
        assert!(s.enable_sensitive_filter);
        assert_eq!(s.theme, "dark");
    }

    #[test]
    fn from_value_partial_object() {
        // 只给部分字段,其余用默认值
        let partial = serde_json::json!({"shortcut": "Cmd+Shift+X", "maxItems": 200});
        let s = settings_from_value(&partial);
        assert_eq!(s.shortcut, "Cmd+Shift+X");
        assert_eq!(s.max_items, 200);
        // 其余字段用默认值
        assert!(s.launch_visible);
        assert!(s.hide_on_blur);
        assert!(s.enable_sensitive_filter);
        assert_eq!(s.retention_days, 0);
    }

    #[test]
    fn from_value_invalid_returns_default() {
        // 无效 JSON value(string 而非 object)→ 返回默认值(不 panic)
        let invalid = serde_json::json!("not an object");
        let s = settings_from_value(&invalid);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn merge_replaces_all_fields() {
        let base = Settings::default();
        let patch = Settings {
            shortcut: "Cmd+Shift+Q".into(),
            launch_visible: false,
            max_items: 100,
            retention_days: 7,
            max_storage_mb: 50,
            hide_on_blur: false,
            enable_sensitive_filter: false,
            theme: "mid".into(),
        };
        let merged = merge_settings(&base, &patch);
        assert_eq!(merged, patch);
    }
}
