/**
 * 窗口控制封装(前端侧)
 *
 * 对 @tauri-apps/api/window 的薄封装,统一窗口 hide/show 调用入口。
 * 后端的窗口控制(SUPPRESS_BLUR_HIDE guard 等)在 window_control.rs。
 *
 * 模块化(§1.21):本模块只管「从前端发 hide/show 指令」,不管后端 guard 逻辑。
 */

/**
 * 隐藏主窗口。
 *
 * 用于:回填成功后 / Esc 按下 / 其他需要关闭弹窗的场景。
 */
export async function hideWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().hide()
}

/**
 * 显示主窗口并聚焦。
 */
export async function showWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const win = getCurrentWindow()
  await win.show()
  await win.setFocus()
}
