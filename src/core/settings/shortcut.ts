/**
 * 快捷键格式化工具(纯函数,可单测)
 *
 * 把键盘事件格式化成 Tauri global-shortcut 字符串格式:
 * "Cmd+Shift+V" / "Cmd+Alt+Space" / "Ctrl+Shift+C" 等
 */

/** Tauri 快捷键字符串合法的 modifier 名 */
const MODIFIER_KEYS = new Set(['Cmd', 'Control', 'Shift', 'Alt', 'Super'])

/**
 * 判断是否是有效的快捷键(至少一个 modifier + 一个非 modifier 键)。
 */
export function isValidShortcut(modifiers: string[], key: string): boolean {
  if (key === '') return false
  if (modifiers.length === 0) return false
  return modifiers.every((m) => MODIFIER_KEYS.has(m))
}

/**
 * 把 modifier 数组 + 键名格式化成 Tauri 快捷键字符串。
 *
 * 例:["Cmd", "Shift"] + "V" → "Cmd+Shift+V"
 */
export function formatShortcut(modifiers: string[], key: string): string {
  return [...modifiers, key].join('+')
}

/**
 * 从 KeyboardEvent 提取 modifier 列表 + 键名(用于快捷键录入器)。
 *
 * macOS: Meta → Cmd;其他平台: Ctrl → Control。
 */
export function parseKeyEvent(e: KeyboardEvent): { modifiers: string[]; key: string } {
  const modifiers: string[] = []
  if (e.metaKey) modifiers.push('Cmd')
  if (e.ctrlKey) modifiers.push('Control')
  if (e.shiftKey) modifiers.push('Shift')
  if (e.altKey) modifiers.push('Alt')

  // 特殊键映射
  const keyMap: Record<string, string> = {
    ' ': 'Space',
    Enter: 'Enter',
    Tab: 'Tab',
    Backspace: 'Backspace',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
  }

  let key = keyMap[e.key] ?? e.key
  // 单字母转大写
  if (key.length === 1) {
    key = key.toUpperCase()
  }

  return { modifiers, key }
}
