/**
 * 快捷键格式化工具(纯函数,可单测)
 *
 * 把键盘事件格式化成 Tauri global-shortcut 字符串格式。
 *
 * 🔴 关键设计:用 `e.code`(物理键位)而非 `e.key`(字符)。
 * 原因:Tauri global-shortcut 底层用 global-hotkey crate 的 Code enum,
 * 它认的是物理键位(如 "KeyV")不是字符(如 "v")。
 * 某些输入法下 e.key 会变成特殊字符(如中文输入法下打 "√"),
 * Tauri 不认识会报 "Couldn't recognize ... as a valid key"。
 *
 * Tauri JS 端接受的格式(参考 global-hotkey Code enum):
 * - 字母:KeyA .. KeyZ(物理键位,不区分大小写)
 * - 数字:Digit0 .. Digit9
 * - F 键:F1 .. F12
 * - 特殊:Space, Enter, Tab, Escape, Backspace, etc.
 * - 标点:Slash, Period, Comma, etc.
 *
 * modifier 名:CommandOrControl(跨平台), Cmd(macOS), Control, Shift, Alt, Super
 */

/** Tauri 快捷键字符串合法的 modifier 名 */
const MODIFIER_KEYS = new Set(['Cmd', 'Control', 'Shift', 'Alt', 'Super', 'CommandOrControl'])

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
 * 例:["Cmd", "Shift"] + "KeyV" → "Cmd+Shift+KeyV"
 */
export function formatShortcut(modifiers: string[], key: string): string {
  return [...modifiers, key].join('+')
}

/**
 * 把 e.code(KeyboardEvent.code)映射成 Tauri 认识的 key 名。
 *
 * 大部分 e.code 已经是 Tauri 认识的格式(KeyA, Digit0, Space, F1 等),
 * 这里主要做箭头键等少量映射 + 大小写归一化。
 *
 * @returns 映射后的 key 名,若无法识别返回 ''
 */
export function codeToTauriKey(code: string): string {
  // 字母键 KeyA-KeyZ 直接用
  if (/^Key[A-Z]$/.test(code)) {
    return code
  }
  // 数字键 Digit0-Digit9 直接用
  if (/^Digit[0-9]$/.test(code)) {
    return code
  }
  // F 键 F1-F12 直接用
  if (/^F(1[0-2]|[1-9])$/.test(code)) {
    return code
  }

  // 特殊键映射
  const specialMap: Record<string, string> = {
    Space: 'Space',
    Enter: 'Enter',
    Tab: 'Tab',
    Escape: 'Escape',
    Backspace: 'Backspace',
    Delete: 'Delete',
    Insert: 'Insert',
    Home: 'Home',
    End: 'End',
    PageUp: 'PageUp',
    PageDown: 'PageDown',
    ArrowUp: 'ArrowUp',
    ArrowDown: 'ArrowDown',
    ArrowLeft: 'ArrowLeft',
    ArrowRight: 'ArrowRight',
    // 标点符号(物理键位)
    Minus: 'Minus',
    Equal: 'Equal',
    BracketLeft: 'BracketLeft',
    BracketRight: 'BracketRight',
    Backslash: 'Backslash',
    Semicolon: 'Semicolon',
    Quote: 'Quote',
    Backquote: 'Backquote',
    Comma: 'Comma',
    Period: 'Period',
    Slash: 'Slash',
  }

  return specialMap[code] ?? ''
}

/**
 * 从 KeyboardEvent 提取 modifier 列表 + Tauri 格式 key 名。
 *
 * macOS: Meta → Cmd;其他平台: Ctrl → Control。
 * 用 e.code(物理键位)而非 e.key(字符),避免输入法特殊字符问题。
 */
export function parseKeyEvent(e: KeyboardEvent): { modifiers: string[]; key: string } {
  const modifiers: string[] = []
  if (e.metaKey) modifiers.push('Cmd')
  if (e.ctrlKey) modifiers.push('Control')
  if (e.shiftKey) modifiers.push('Shift')
  if (e.altKey) modifiers.push('Alt')

  const key = codeToTauriKey(e.code)

  return { modifiers, key }
}
