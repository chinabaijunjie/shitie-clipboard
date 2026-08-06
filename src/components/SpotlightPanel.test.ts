/**
 * SpotlightPanel 单测 — 键盘导航边界计算(纯函数)
 *
 * 测试覆盖(随写随测 §1.22):
 * - nextIndex:下移 / 上移 / 循环(顶↔底)/ 空列表
 *
 * 注:组件整体渲染涉及 Tauri invoke/listen,此处只测抽出的纯函数。
 * 组件级交互测试(键盘事件触发)留作后续,需更复杂的 mock。
 */
import { describe, expect, it } from 'vitest'

import { nextIndex } from './SpotlightPanel'

describe('nextIndex(键盘导航边界)', () => {
  it('向下移动 → 索引 +1', () => {
    expect(nextIndex(0, 1, 5)).toBe(1)
    expect(nextIndex(2, 1, 5)).toBe(3)
  })

  it('向上移动 → 索引 -1', () => {
    expect(nextIndex(3, -1, 5)).toBe(2)
    expect(nextIndex(4, -1, 5)).toBe(3)
  })

  it('到底按下 → 循环回顶部', () => {
    expect(nextIndex(4, 1, 5)).toBe(0)
  })

  it('到顶按上 → 循环回底部', () => {
    expect(nextIndex(0, -1, 5)).toBe(4)
  })

  it('空列表 → 始终 -1', () => {
    expect(nextIndex(0, 1, 0)).toBe(-1)
    expect(nextIndex(0, -1, 0)).toBe(-1)
  })

  it('单元素列表 → 始终 0(循环到自己)', () => {
    expect(nextIndex(0, 1, 1)).toBe(0)
    expect(nextIndex(0, -1, 1)).toBe(0)
  })
})
