/**
 * theme.ts 纯函数测试
 *
 * happy-dom 自带 localStorage / document.documentElement,无需 mock。
 * 每个测试前 clear 避免状态泄漏。
 */
import { afterEach, describe, expect, it } from 'vitest'

import { STORAGE_KEYS } from '@core/constants'
import { applyTheme, getInitialTheme, isThemeName } from '@core/theme'

describe('isThemeName', () => {
  it('合法主题名返回 true', () => {
    expect(isThemeName('dark')).toBe(true)
    expect(isThemeName('light')).toBe(true)
    expect(isThemeName('mid')).toBe(true)
  })

  it('非法值返回 false', () => {
    expect(isThemeName('blue')).toBe(false)
    expect(isThemeName('')).toBe(false)
    expect(isThemeName(null)).toBe(false)
    expect(isThemeName(undefined)).toBe(false)
    expect(isThemeName(42)).toBe(false)
  })
})

describe('applyTheme', () => {
  afterEach(() => {
    localStorage.clear()
    document.documentElement.className = ''
  })

  it('设置 <html> class 为 theme-{name} dark', () => {
    applyTheme('dark')
    expect(document.documentElement.className).toBe('theme-dark dark')

    applyTheme('light')
    expect(document.documentElement.className).toBe('theme-light dark')

    applyTheme('mid')
    expect(document.documentElement.className).toBe('theme-mid dark')
  })

  it('写入 localStorage 缓存', () => {
    applyTheme('light')
    expect(localStorage.getItem(STORAGE_KEYS.THEME)).toBe('light')
  })

  it('切换主题时替换而非追加 class', () => {
    applyTheme('dark')
    applyTheme('light')
    // 不应有两个 theme- class
    expect(document.documentElement.className).toBe('theme-light dark')
    expect(document.documentElement.classList.contains('theme-dark')).toBe(false)
  })
})

describe('getInitialTheme', () => {
  afterEach(() => {
    localStorage.clear()
    document.documentElement.className = ''
  })

  it('localStorage 有缓存时返回缓存值', () => {
    localStorage.setItem(STORAGE_KEYS.THEME, 'light')
    expect(getInitialTheme()).toBe('light')
  })

  it('localStorage 无缓存时返回 dark 默认值', () => {
    expect(getInitialTheme()).toBe('dark')
  })

  it('localStorage 有非法值时返回 dark 默认值', () => {
    localStorage.setItem(STORAGE_KEYS.THEME, 'invalid-theme')
    expect(getInitialTheme()).toBe('dark')
  })
})
