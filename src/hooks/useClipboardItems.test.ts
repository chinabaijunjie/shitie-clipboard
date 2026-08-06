/**
 * useClipboardItems 单测 — 列表数据逻辑
 *
 * 测试覆盖(随写随测 §1.22):
 * - 纯函数 applyAddedItem:新条目前置 / 已存在则更新并前置 / 不可变
 * - 纯函数 removeItem:按 id 移除 / 不可变
 *
 * mock 策略:@tauri-apps/api/event 的 listen、@core/api 的 call。
 */
import { describe, expect, it, vi } from 'vitest'

import { applyAddedItem, removeItem } from './useClipboardItems'
import type { ClipboardItem } from '@core/types'

// mock Tauri 通信层(hook 内部用,纯函数测试不依赖)
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock('@core/api', () => ({
  call: vi.fn(() => Promise.resolve([])),
}))

function makeItem(id: number, title: string, extra?: Partial<ClipboardItem>): ClipboardItem {
  return {
    id,
    firstCopiedAt: 1000,
    lastCopiedAt: 1000,
    copyCount: 1,
    title,
    contentKind: 'text',
    ...extra,
  }
}

describe('applyAddedItem', () => {
  it('新条目 → 放到最前', () => {
    const items = [makeItem(1, '旧'), makeItem(2, '更旧')]
    const added = makeItem(3, '新')

    const result = applyAddedItem(items, added)

    expect(result[0].id).toBe(3)
    expect(result).toHaveLength(3)
  })

  it('已存在同 id → 更新字段并移到最前', () => {
    const items = [makeItem(1, '第一'), makeItem(2, '第二')]
    // id=2 的条目被再次复制,copy_count 累加
    const updated = makeItem(2, '第二', { copyCount: 5, lastCopiedAt: 9999 })

    const result = applyAddedItem(items, updated)

    expect(result).toHaveLength(2) // 不新增
    expect(result[0].id).toBe(2) // 移到最前
    expect(result[0].copyCount).toBe(5) // 字段更新
    expect(result[1].id).toBe(1)
  })

  it('空列表 → 返回单元素列表', () => {
    const added = makeItem(1, '首条')
    expect(applyAddedItem([], added)).toEqual([added])
  })

  it('不可变性:不修改原数组', () => {
    const original = [makeItem(1, 'A')]
    const snapshot = [...original]

    applyAddedItem(original, makeItem(2, 'B'))

    expect(original).toEqual(snapshot) // 原数组未变
  })
})

describe('removeItem', () => {
  it('按 id 移除', () => {
    const items = [makeItem(1, 'A'), makeItem(2, 'B'), makeItem(3, 'C')]

    const result = removeItem(items, 2)

    expect(result).toHaveLength(2)
    expect(result.map((i) => i.id)).toEqual([1, 3])
  })

  it('id 不存在 → 原样返回(不报错)', () => {
    const items = [makeItem(1, 'A')]

    const result = removeItem(items, 999)

    expect(result).toEqual(items)
  })

  it('不可变性:不修改原数组', () => {
    const original = [makeItem(1, 'A'), makeItem(2, 'B')]
    const snapshot = [...original]

    removeItem(original, 1)

    expect(original).toEqual(snapshot)
  })
})
