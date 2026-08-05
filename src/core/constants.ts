/**
 * 全局常量 — API 路径、应用元信息、localStorage 键名
 *
 * 严禁硬编码 API URL 或状态字符串，所有跨模块共享常量都集中在此。
 * 业务工程起步时，在此追加业务状态枚举（如 TASK_STATUS / IMAGE_FORMAT）。
 */

// ============================================================
// 应用元信息
// ============================================================
export const APP_NAME = 'Clipboard Manager'
export const APP_VERSION = '1.0.0'

// ============================================================
// 后端 sidecar 通信
// ============================================================

/**
 * Python sidecar 监听端口。
 * ⚠️ 三处对齐：本处 + Rust sidecar.rs SIDECAR_PORT + backend/core/config.py SIDECAR_PORT
 */
export const SIDECAR_PORT = 9880
/** sidecar 基础 URL */
export const SIDECAR_BASE_URL = `http://localhost:${SIDECAR_PORT}`

// ============================================================
// API 路径前缀
// ============================================================
export const API_PREFIX = '/api'

// ============================================================
// localStorage 键名前缀（业务工程改名后同步改前缀，避免冲突）
// ============================================================
export const STORAGE_KEY_PREFIX = 'clipboard-manager:'
export const STORAGE_KEYS = {
  THEME: `${STORAGE_KEY_PREFIX}theme`,
  SIDECAR_PID: `${STORAGE_KEY_PREFIX}sidecar-pid`,
} as const
