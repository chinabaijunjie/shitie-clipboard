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
export const APP_VERSION = '2.0.0'

// ============================================================
// localStorage 键名前缀(避免与其他应用冲突)
// ============================================================
export const STORAGE_KEY_PREFIX = 'clipboard-manager:'
export const STORAGE_KEYS = {
  THEME: `${STORAGE_KEY_PREFIX}theme`,
} as const
