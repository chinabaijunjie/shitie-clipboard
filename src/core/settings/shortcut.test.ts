/**
 * shortcut 工具函数测试(纯函数,随写随测 §1.22)
 *
 * 🔴 核心测试目标:确保用 e.code(物理键位)而非 e.key(字符),
 * 避免输入法特殊字符(如 √)导致 Tauri 无法识别。
 */
import { describe, expect, it } from 'vitest'

import { codeToTauriKey, formatShortcut, isValidShortcut, parseKeyEvent } from './shortcut'

describe('isValidShortcut', () => {
  it('合法:1 个 modifier + 字母键', () => {
    expect(isValidShortcut(['Cmd'], 'KeyV')).toBe(true)
  })

  it('合法:多个 modifier + 字母键', () => {
    expect(isValidShortcut(['Cmd', 'Shift'], 'KeyV')).toBe(true)
  })

  it('不合法:无 modifier', () => {
    expect(isValidShortcut([], 'KeyV')).toBe(false)
  })

  it('不合法:无 key', () => {
    expect(isValidShortcut(['Cmd'], '')).toBe(false)
  })

  it('不合法:未知 modifier', () => {
    expect(isValidShortcut(['Foo'], 'KeyV')).toBe(false)
  })
})

describe('formatShortcut', () => {
  it('单个 modifier', () => {
    expect(formatShortcut(['Cmd'], 'KeyV')).toBe('Cmd+KeyV')
  })

  it('多个 modifier(保持顺序)', () => {
    expect(formatShortcut(['Cmd', 'Shift', 'Alt'], 'KeyV')).toBe('Cmd+Shift+Alt+KeyV')
  })
})

describe('codeToTauriKey', () => {
  it('字母键 KeyA-KeyZ 直接返回', () => {
    expect(codeToTauriKey('KeyA')).toBe('KeyA')
    expect(codeToTauriKey('KeyZ')).toBe('KeyZ')
  })

  it('数字键 Digit0-Digit9 直接返回', () => {
    expect(codeToTauriKey('Digit0')).toBe('Digit0')
    expect(codeToTauriKey('Digit9')).toBe('Digit9')
  })

  it('F 键直接返回', () => {
    expect(codeToTauriKey('F1')).toBe('F1')
    expect(codeToTauriKey('F12')).toBe('F12')
  })

  it('空格 → Space', () => {
    expect(codeToTauriKey('Space')).toBe('Space')
  })

  it('回车 → Enter', () => {
    expect(codeToTauriKey('Enter')).toBe('Enter')
  })

  it('箭头键映射', () => {
    expect(codeToTauriKey('ArrowUp')).toBe('ArrowUp')
    expect(codeToTauriKey('ArrowDown')).toBe('ArrowDown')
  })

  it('标点键映射', () => {
    expect(codeToTauriKey('Slash')).toBe('Slash')
    expect(codeToTauriKey('Comma')).toBe('Comma')
    expect(codeToTauriKey('Period')).toBe('Period')
  })

  it('无法识别的 code → 空字符串', () => {
    expect(codeToTauriKey('UnknownKey')).toBe('')
  })
})

describe('parseKeyEvent', () => {
  /** 构造 KeyboardEvent 的辅助函数(用 e.code 物理键位) */
  function makeEvent(opts: {
    code: string
    metaKey?: boolean
    ctrlKey?: boolean
    shiftKey?: boolean
    altKey?: boolean
  }): KeyboardEvent {
    return {
      code: opts.code,
      metaKey: opts.metaKey ?? false,
      ctrlKey: opts.ctrlKey ?? false,
      shiftKey: opts.shiftKey ?? false,
      altKey: opts.altKey ?? false,
    } as KeyboardEvent
  }

  it('Cmd+Shift+V(code=KeyV)', () => {
    const result = parseKeyEvent(makeEvent({ code: 'KeyV', metaKey: true, shiftKey: true }))
    expect(result.modifiers).toEqual(['Cmd', 'Shift'])
    expect(result.key).toBe('KeyV')
  })

  it('空格键', () => {
    const result = parseKeyEvent(makeEvent({ code: 'Space', metaKey: true }))
    expect(result.key).toBe('Space')
  })

  it('Ctrl → Control(非 macOS)', () => {
    const result = parseKeyEvent(makeEvent({ code: 'KeyC', ctrlKey: true }))
    expect(result.modifiers).toEqual(['Control'])
    expect(result.key).toBe('KeyC')
  })

  it('Alt 键', () => {
    const result = parseKeyEvent(makeEvent({ code: 'KeyX', metaKey: true, altKey: true }))
    expect(result.modifiers).toEqual(['Cmd', 'Alt'])
    expect(result.key).toBe('KeyX')
  })

  it('🔴 输入法特殊字符不干扰:中文输入法下按 V 键 code 仍是 KeyV', () => {
    // 即使 e.key 可能是 "√" 或其他字符,e.code 始终是 "KeyV"(物理键位)
    const result = parseKeyEvent(makeEvent({ code: 'KeyV', metaKey: true }))
    expect(result.key).toBe('KeyV')
    expect(isValidShortcut(result.modifiers, result.key)).toBe(true)
  })
})
