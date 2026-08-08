/**
 * shortcut 工具函数测试(纯函数,随写随测 §1.22)
 */
import { describe, expect, it } from 'vitest'

import { formatShortcut, isValidShortcut, parseKeyEvent } from './shortcut'

describe('isValidShortcut', () => {
  it('合法:1 个 modifier + 字母键', () => {
    expect(isValidShortcut(['Cmd'], 'V')).toBe(true)
  })

  it('合法:多个 modifier + 字母键', () => {
    expect(isValidShortcut(['Cmd', 'Shift'], 'V')).toBe(true)
  })

  it('不合法:无 modifier', () => {
    expect(isValidShortcut([], 'V')).toBe(false)
  })

  it('不合法:无 key', () => {
    expect(isValidShortcut(['Cmd'], '')).toBe(false)
  })

  it('不合法:未知 modifier', () => {
    expect(isValidShortcut(['Foo'], 'V')).toBe(false)
  })
})

describe('formatShortcut', () => {
  it('单个 modifier', () => {
    expect(formatShortcut(['Cmd'], 'V')).toBe('Cmd+V')
  })

  it('多个 modifier(保持顺序)', () => {
    expect(formatShortcut(['Cmd', 'Shift', 'Alt'], 'V')).toBe('Cmd+Shift+Alt+V')
  })

  it('空 modifier + key', () => {
    expect(formatShortcut([], 'Space')).toBe('Space')
  })
})

describe('parseKeyEvent', () => {
  /** 构造 KeyboardEvent 的辅助函数 */
  function makeEvent(opts: {
    key: string
    metaKey?: boolean
    ctrlKey?: boolean
    shiftKey?: boolean
    altKey?: boolean
  }): KeyboardEvent {
    return {
      key: opts.key,
      metaKey: opts.metaKey ?? false,
      ctrlKey: opts.ctrlKey ?? false,
      shiftKey: opts.shiftKey ?? false,
      altKey: opts.altKey ?? false,
    } as KeyboardEvent
  }

  it('Cmd+Shift+V', () => {
    const result = parseKeyEvent(makeEvent({ key: 'v', metaKey: true, shiftKey: true }))
    expect(result.modifiers).toEqual(['Cmd', 'Shift'])
    expect(result.key).toBe('V')
  })

  it('单字母转大写', () => {
    const result = parseKeyEvent(makeEvent({ key: 'a', metaKey: true }))
    expect(result.key).toBe('A')
  })

  it('空格 → Space', () => {
    const result = parseKeyEvent(makeEvent({ key: ' ', metaKey: true }))
    expect(result.key).toBe('Space')
  })

  it('Ctrl → Control(非 macOS)', () => {
    const result = parseKeyEvent(makeEvent({ key: 'c', ctrlKey: true }))
    expect(result.modifiers).toEqual(['Control'])
  })

  it('Alt 键', () => {
    const result = parseKeyEvent(makeEvent({ key: 'x', metaKey: true, altKey: true }))
    expect(result.modifiers).toEqual(['Cmd', 'Alt'])
  })

  it('箭头键映射', () => {
    const result = parseKeyEvent(makeEvent({ key: 'ArrowUp', metaKey: true }))
    expect(result.key).toBe('Up')
  })
})
