/**
 * useClipboardItems 单测 — 列表数据逻辑 + 搜索状态机
 *
 * 测试覆盖(随写随测 §1.22):
 * - 纯函数 applyAddedItem:新条目前置 / 已存在则更新并前置 / 不可变
 * - 纯函数 removeItem:按 id 移除 / 不可变
 * - 搜索状态机(test-analyzer 7/10 建议):
 *   - 搜索模式忽略 added 事件(searchQueryRef 闭包防过时)
 *   - 非搜索模式接收 added 事件
 *   - search('') 清空搜索回退全列表
 *   - search 引用稳定(防 debounce 闪烁)
 *
 * mock 策略:@tauri-apps/api/event 的 listen、@core/api 的 call。
 */
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { applyAddedItem, removeItem, useClipboardItems } from './useClipboardItems'
import type { ClipboardItem } from '@core/types'

// ============================================================
// Mock Tauri 通信层
// ============================================================

// vi.mock 工厂被提升到文件顶部,不能直接引用外部 let 变量。
// 用 vi.hoisted 把可变状态提到工厂能访问的作用域。
const mockState = vi.hoisted(() => ({
  /** 捕获 listen 注册的回调(测试中手动触发,模拟后端 emit)。 */
  listenCallback: null as ((event: { payload: unknown }) => void) | null,
  /** call mock:按命令名返回不同数据。测试中可覆写。 */
  callData: {} as Record<string, unknown>,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_event: string, cb: (event: { payload: unknown }) => void) => {
    mockState.listenCallback = cb
    return () => {
      mockState.listenCallback = null
    }
  }),
}))

vi.mock('@core/api', () => ({
  call: vi.fn(async (cmd: string) => {
    return mockState.callData[cmd] ?? []
  }),
}))

// ============================================================
// 测试辅助
// ============================================================

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

beforeEach(() => {
  mockState.listenCallback = null
  mockState.callData = {}
  vi.clearAllMocks()
})

afterEach(() => {
  mockState.listenCallback = null
  mockState.callData = {}
})

/**
 * flush 异步:等待 hook 内的 async load 完成(call mock 是 async,
 * 需要 flush 微任务让 promise resolve + React 状态更新生效)。
 */
async function flushAsync() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}

// ============================================================
// 纯函数测试(原有,保持不变)
// ============================================================

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

// ============================================================
// 搜索状态机测试(renderHook)
// ============================================================

describe('useClipboardItems 搜索状态机', () => {
  it('初次加载:list_items → 填充列表', async () => {
    const listData = [makeItem(1, '第一条'), makeItem(2, '第二条')]
    mockState.callData = { list_items: listData }

    const { result } = renderHook(() => useClipboardItems())

    // 等待异步 load 完成
    await flushAsync()

    expect(result.current.loading).toBe(false)
    expect(result.current.items).toHaveLength(2)
    expect(result.current.searchQuery).toBeNull()
  })

  it('非搜索模式:added 事件 → 前置新条目', async () => {
    // 初次加载 1 条
    mockState.callData = { list_items: [makeItem(1, '已有')] }
    const { result } = renderHook(() => useClipboardItems())

    await flushAsync()

    // 模拟后端 emit added 事件(searchQueryRef.current === null,非搜索模式)
    const newItem = makeItem(2, '新复制')
    act(() => {
      mockState.listenCallback?.({ payload: newItem })
    })

    // 新条目应被前置
    expect(result.current.items).toHaveLength(2)
    expect(result.current.items[0].id).toBe(2)
  })

  it('搜索模式:added 事件 → 忽略(不更新列表)', async () => {
    // 初次加载 2 条
    const initialList = [makeItem(1, '已有A'), makeItem(2, '已有B')]
    mockState.callData = {
      list_items: initialList,
      search_items: [makeItem(1, '已有A')], // 搜索只返回 1 条
    }
    const { result } = renderHook(() => useClipboardItems())

    await flushAsync()

    // 进入搜索模式
    await act(async () => {
      await result.current.search('已有')
    })

    expect(result.current.searchQuery).toBe('已有')
    expect(result.current.items).toHaveLength(1) // 搜索结果

    // 模拟后端 emit added 事件 → 搜索模式下应忽略
    const newItem = makeItem(3, '新复制')
    act(() => {
      mockState.listenCallback?.({ payload: newItem })
    })

    // 列表不应更新(仍为搜索结果 1 条,不含新条目)
    expect(result.current.items).toHaveLength(1)
    expect(result.current.items.find((i) => i.id === 3)).toBeUndefined()
  })

  it("search('') → 清空搜索,回退全列表", async () => {
    const fullList = [makeItem(1, '苹果'), makeItem(2, '香蕉'), makeItem(3, '橙子')]
    mockState.callData = {
      list_items: fullList,
      search_items: [makeItem(1, '苹果')],
    }
    const { result } = renderHook(() => useClipboardItems())

    await flushAsync()

    // 先搜索
    await act(async () => {
      await result.current.search('苹果')
    })
    expect(result.current.searchQuery).toBe('苹果')
    expect(result.current.items).toHaveLength(1)

    // 清空搜索 → 回退全列表
    await act(async () => {
      await result.current.search('')
    })

    expect(result.current.searchQuery).toBeNull()
    expect(result.current.items).toHaveLength(3)
  })

  it("search('   ') → 纯空白也清空搜索(回退全列表)", async () => {
    const fullList = [makeItem(1, 'A'), makeItem(2, 'B')]
    mockState.callData = {
      list_items: fullList,
      search_items: [makeItem(1, 'A')],
    }
    const { result } = renderHook(() => useClipboardItems())

    await flushAsync()

    // 先搜索
    await act(async () => {
      await result.current.search('A')
    })
    expect(result.current.searchQuery).toBe('A')

    // 纯空白 → 清空(trim 后为空)
    await act(async () => {
      await result.current.search('   ')
    })

    expect(result.current.searchQuery).toBeNull()
    expect(result.current.items).toHaveLength(2)
  })

  it('search 引用稳定(useCallback,防 debounce 闪烁)', async () => {
    mockState.callData = { list_items: [] }
    const { result, rerender } = renderHook(() => useClipboardItems())

    await flushAsync()

    const searchRef1 = result.current.search

    // 重渲染(useCallback deps [] → 引用不变)
    rerender()

    const searchRef2 = result.current.search

    expect(searchRef1).toBe(searchRef2)
  })

  it('搜索模式切回非搜索后:added 事件恢复接收', async () => {
    const fullList = [makeItem(1, 'A')]
    mockState.callData = {
      list_items: fullList,
      search_items: [makeItem(1, 'A')],
    }
    const { result } = renderHook(() => useClipboardItems())

    await flushAsync()

    // 进入搜索 → 退出搜索
    await act(async () => {
      await result.current.search('A')
    })
    await act(async () => {
      await result.current.search('')
    })

    expect(result.current.searchQuery).toBeNull()

    // 此时 added 事件应恢复接收(searchQueryRef.current === null)
    const newItem = makeItem(2, 'B')
    act(() => {
      mockState.listenCallback?.({ payload: newItem })
    })

    expect(result.current.items).toHaveLength(2)
    expect(result.current.items[0].id).toBe(2)
  })
})
