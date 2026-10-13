/**
 * useItemFullText 单测 — 全文获取状态机
 *
 * 测试覆盖(随写随测 §1.22):
 * - 短路:title < 1000 → 不发 invoke,直接返回 title
 * - debounce:200ms 内不发请求(键盘连续导航合并)
 * - 超长拉取:达到阈值 → invoke get_item_text → 返回全文
 * - 竞态:快速切换条目,旧响应不作数(按 id 匹配)
 * - 图片/文件条目永远不拉(title 恒短)
 *
 * mock 策略:@core/api 的 call。
 */
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { needsFullText, useItemFullText } from './useItemFullText'
import type { ClipboardItem } from '@core/types'

const callMock = vi.hoisted(() => vi.fn())

vi.mock('@core/api', () => ({
  call: callMock,
}))

/** 长文本条目 fixture */
function makeItem(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 1,
    firstCopiedAt: 1728800000000,
    lastCopiedAt: 1728800000000,
    copyCount: 1,
    title: '短文本',
    contentKind: 'text',
    pinned: false,
    ...overrides,
  }
}

describe('needsFullText(纯函数)', () => {
  it('短 title → false', () => {
    expect(needsFullText(makeItem())).toBe(false)
  })

  it('title 达到 1000 字符阈值 → true', () => {
    expect(needsFullText(makeItem({ title: 'a'.repeat(1000) }))).toBe(true)
  })

  it('图片/文件条目恒 false(即使 title 达阈值)', () => {
    const long = 'a'.repeat(1000)
    expect(needsFullText(makeItem({ title: long, contentKind: 'image', thumbPath: '/t.png', imagePath: '/o.png' }))).toBe(false)
    expect(needsFullText(makeItem({ title: long, contentKind: 'file', fileUrl: 'file:///a.txt' }))).toBe(false)
  })

  it('null → false', () => {
    expect(needsFullText(null)).toBe(false)
  })
})

describe('useItemFullText', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    callMock.mockReset()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('title < 1000 → 零 invoke,返回 title 本身', () => {
    const item = makeItem({ title: '常规长度' })
    const { result } = renderHook(() => useItemFullText(item))
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(callMock).not.toHaveBeenCalled()
    expect(result.current).toBe('常规长度')
  })

  it('debounce:200ms 内不发起请求', () => {
    const item = makeItem({ title: 'a'.repeat(1000) })
    renderHook(() => useItemFullText(item))
    act(() => {
      vi.advanceTimersByTime(199)
    })
    expect(callMock).not.toHaveBeenCalled()
  })

  it('超长 title → 200ms 后 invoke get_item_text,返回全文', async () => {
    const item = makeItem({ id: 7, title: 'a'.repeat(1000) })
    callMock.mockResolvedValue('这是完整的长文本内容')
    const { result } = renderHook(() => useItemFullText(item))
    await act(async () => {
      vi.advanceTimersByTime(200)
    })
    expect(callMock).toHaveBeenCalledWith('get_item_text', { id: 7 })
    expect(result.current).toBe('这是完整的长文本内容')
  })

  it('切换条目:旧响应不作数,按 id 匹配', async () => {
    const itemA = makeItem({ id: 1, title: 'a'.repeat(1000) })
    const itemB = makeItem({ id: 2, title: 'b'.repeat(1000) })
    callMock.mockResolvedValue('B 的全文')
    const { result, rerender } = renderHook(({ item }) => useItemFullText(item), {
      initialProps: { item: itemA as ClipboardItem | null },
    })
    // A 的 debounce 未到就切到 B → A 的定时器被清理,只发 B 的请求
    act(() => {
      vi.advanceTimersByTime(100)
    })
    rerender({ item: itemB })
    await act(async () => {
      vi.advanceTimersByTime(200)
    })
    expect(callMock).toHaveBeenCalledTimes(1)
    expect(callMock).toHaveBeenCalledWith('get_item_text', { id: 2 })
    expect(result.current).toBe('B 的全文')
  })

  it('无选中(null)→ 返回空字符串', () => {
    const { result } = renderHook(() => useItemFullText(null))
    expect(result.current).toBe('')
  })
})
