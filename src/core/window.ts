/**
 * 窗口控制封装(前端侧)
 *
 * hideWindow 走后端 command(统一 SUPPRESS_BLUR_HIDE guard 管理),
 * 不直接调 getCurrentWindow().hide()(会绕过 guard 导致 Focused(false) 连锁)。
 * showWindow 由全局快捷键/托盘在后端触发,前端不需要。
 */

import { call } from '@core/api'

/**
 * 隐藏主窗口。
 *
 * ⚠️ 必须走后端 hide_window command(而非 getCurrentWindow().hide()),
 * 以确保 SUPPRESS_BLUR_HIDE guard 被正确置位,防 Focused(false) 连锁。
 */
export async function hideWindow(): Promise<void> {
  await call('hide_window')
}
