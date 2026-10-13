/**
 * 全局常量 — 应用元信息 + localStorage 键名
 *
 * 严禁硬编码状态字符串,所有跨模块共享常量都集中在此。
 * 业务工程起步时,在此追加业务状态枚举(如 CLIPBOARD_ITEM_TYPE)。
 *
 * sidecar 时代的 SIDECAR_PORT / SIDECAR_BASE_URL 已删除(改用 Tauri invoke,无 HTTP)。
 */

// ============================================================
// 应用元信息
// ============================================================
export const APP_NAME = '拾贴'

// 版本号唯一真相源是 package.json,Cargo.toml / tauri.conf.json 打包时同步。
// 改版本号只需改 package.json,前端自动跟上。
import pkg from '../../package.json'
export const APP_VERSION = pkg.version

// ============================================================
// localStorage 键名前缀(避免与其他应用冲突)
// ============================================================
export const STORAGE_KEY_PREFIX = 'clipboard-manager:'
export const STORAGE_KEYS = {
  THEME: `${STORAGE_KEY_PREFIX}theme`,
} as const

// ============================================================
// 业务常量
// ============================================================

/** 列表 title 的截断上限(字符)。
 *  与 Rust 侧 models.rs 的 TITLE_MAX_CHARS 对齐(两端镜像,改须同步)。
 *  title 达到该长度视为可能被截断,预览面板据此决定是否 invoke get_item_text 取全文。 */
export const TITLE_MAX_CHARS = 1000
