/**
 * useClipboardItems 单测 — 列表数据逻辑 + 搜索状态机 + 分页加载
 *
 * 测试覆盖(随写随测 §1.22):
 * - 纯函数 applyAddedItem / removeItem / mergePage:不可变 + 去重
 * - 搜索状态机(test-analyzer 7/10 建议):搜索模式忽略 added / search('') 回退 / 引用稳定
 * - 分页:初次加载 ListPage / loadMore 追加去重 / hasMore 边界 / added 与删除同步 total
 *
 * mock 策略:@tauri-apps/api/event 的 listen、@core/api 的 call。
 * call mock 的返回值:数组 → 自动包成 {items, total}(兼容旧用例);
 * 函数 → 以 invoke args 调用后取返回值(分页按 offset 出不同页)。
 */
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { applyAddedItem, mergePage, moveToFront, removeItem, sortByPinned, useClipboardItems } from './useClipboardItems'
import type { ClipboardItem } from '@core/types'

// ============================================================
// Mock Tauri 通信层
// ============================================================

// vi.mock 工厂被提升到文件顶部,不能直接引用外部 let 变量。
// 用 vi.hoisted 把可变状态提到工厂能访问的作用域。
const mockState = vi.hoisted(() => ({
  /** 捕获 listen 注册的回调(测试中手动触发,模拟后端 emit)。 */
  listenCallback: null as ((event: { payload: unknown }) => void) | null,
  /** call mock:按命令名返回数据。数组自动包 ListPage;函数按 args 求值。 */
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
  call: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    const data = mockState.callData[cmd]
    const resolved = typeof data === 'function' ? (data as (a: unknown) => unknown)(args) : data
    if (resolved === undefined) return { items: [], total: 0 }
    return Array.isArray(resolved) ? { items: resolved, total: resolved.length } : resolved
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
    pinned: false,
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

describe('mergePage(分页追加去重)', () => {
  it('新 id → 追加到尾部', () => {
    const existing = [makeItem(1, 'A'), makeItem(2, 'B')]
    const result = mergePage(existing, [makeItem(3, 'C'), makeItem(4, 'D')])
    expect(result.map((i) => i.id)).toEqual([1, 2, 3, 4])
  })

  it('滚动期间数据变化的边界重复 id → 以旧列表为准,不重复追加', () => {
    const existing = [makeItem(1, 'A'), makeItem(2, 'B')]
    // 第 2 页里混入了已有 id=2(期间该条被更新过)
    const result = mergePage(existing, [makeItem(2, 'B-新'), makeItem(3, 'C')])
    expect(result.map((i) => i.id)).toEqual([1, 2, 3])
    expect(result[1].title).toBe('B') // 旧列表条目保留
  })

  it('整页重复(重复触发)→ 列表不变', () => {
    const existing = [makeItem(1, 'A')]
    const result = mergePage(existing, [makeItem(1, 'A')])
    expect(result).toHaveLength(1)
  })

  it('不可变性:不修改原数组', () => {
    const original = [makeItem(1, 'A')]
    const snapshot = [...original]
    mergePage(original, [makeItem(2, 'B')])
    expect(original).toEqual(snapshot)
  })
})

describe('moveToFront', () => {
  it('把中间条目移到最前', () => {
    const items = [makeItem(1, 'A'), makeItem(2, 'B'), makeItem(3, 'C')]

    const result = moveToFront(items, 2)

    expect(result.map((i) => i.id)).toEqual([2, 1, 3])
  })

  it('把末尾条目移到最前', () => {
    const items = [makeItem(1, 'A'), makeItem(2, 'B'), makeItem(3, 'C')]

    const result = moveToFront(items, 3)

    expect(result.map((i) => i.id)).toEqual([3, 1, 2])
  })

  it('已在最前 → 原样返回', () => {
    const items = [makeItem(1, 'A'), makeItem(2, 'B')]

    const result = moveToFront(items, 1)

    expect(result.map((i) => i.id)).toEqual([1, 2])
  })

  it('id 不存在 → 原样返回', () => {
    const items = [makeItem(1, 'A')]

    const result = moveToFront(items, 999)

    expect(result.map((i) => i.id)).toEqual([1])
  })

  it('不可变性:不修改原数组', () => {
    const original = [makeItem(1, 'A'), makeItem(2, 'B')]
    const snapshot = [...original]

    moveToFront(original, 2)

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

// ============================================================
// 分页加载测试(renderHook)
// ============================================================

describe('useClipboardItems 分页加载', () => {
  /** 造一批连续 id 的条目 */
  function makeRange(from: number, to: number): ClipboardItem[] {
    return Array.from({ length: to - from + 1 }, (_, i) => makeItem(from + i, `条目${from + i}`))
  }

  it('初次加载 ListPage:items + total + hasMore', async () => {
    mockState.callData = {
      list_items: { items: makeRange(1, 50), total: 120 },
    }
    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    expect(result.current.items).toHaveLength(50)
    expect(result.current.total).toBe(120)
    expect(result.current.hasMore).toBe(true)
    expect(result.current.loadingMore).toBe(false)
  })

  it('total 不大于已加载数 → hasMore false', async () => {
    mockState.callData = {
      list_items: { items: makeRange(1, 3), total: 3 },
    }
    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    expect(result.current.hasMore).toBe(false)
    // 无下一页时 loadMore 不发请求(callData 计数不变 — 通过 items/total 稳定验证)
    await act(async () => {
      await result.current.loadMore()
    })
    expect(result.current.items).toHaveLength(3)
  })

  it('loadMore:按 offset 拉下一页并追加(去重)', async () => {
    mockState.callData = {
      list_items: (args: { offset?: number } | undefined) =>
        (args?.offset ?? 0) === 0
          ? { items: makeRange(1, 50), total: 52 }
          : { items: [makeItem(50, '条目50'), ...makeRange(51, 52)], total: 52 }, // 故意混入边界重复 id=50
    }
    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()
    expect(result.current.items).toHaveLength(50)

    await act(async () => {
      await result.current.loadMore()
    })

    // 第 2 页混入 id=50 重复 → 去重后只追加 51/52
    expect(result.current.items).toHaveLength(52)
    expect(result.current.items.map((i) => i.id)).toEqual([...Array.from({ length: 50 }, (_, i) => i + 1), 51, 52])
    expect(result.current.hasMore).toBe(false)
  })

  it('added 事件 → total 同步 +1;删除 → total 同步 -1', async () => {
    mockState.callData = {
      list_items: { items: makeRange(1, 2), total: 2 },
    }
    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    act(() => {
      mockState.listenCallback?.({ payload: makeItem(99, '新') })
    })
    // 新条目入列表,total 同步增长(items 3 / total 3,hasMore 仍 false)
    expect(result.current.total).toBe(3)
    expect(result.current.items).toHaveLength(3)
    expect(result.current.hasMore).toBe(false)

    await act(async () => {
      await result.current.deleteItem(99)
    })
    expect(result.current.total).toBe(2)
    expect(result.current.items).toHaveLength(2)
  })
})

// ============================================================
// moveToFront action 集成测试(renderHook)
// ============================================================

describe('useClipboardItems moveToFront action', () => {
  it('把指定 id 的条目移到最前', async () => {
    const list = [makeItem(1, 'A'), makeItem(2, 'B'), makeItem(3, 'C')]
    mockState.callData = { list_items: list }

    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    expect(result.current.items.map((i) => i.id)).toEqual([1, 2, 3])

    // 把 id=3 移到最前
    act(() => {
      result.current.moveToFront(3)
    })

    expect(result.current.items.map((i) => i.id)).toEqual([3, 1, 2])
  })

  it('不存在的 id → 列表不变', async () => {
    const list = [makeItem(1, 'A'), makeItem(2, 'B')]
    mockState.callData = { list_items: list }

    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    act(() => {
      result.current.moveToFront(999)
    })

    expect(result.current.items.map((i) => i.id)).toEqual([1, 2])
  })

  it('连续 moveToFront 不丢失中间状态(函数式 setState)', async () => {
    const list = [makeItem(1, 'A'), makeItem(2, 'B'), makeItem(3, 'C')]
    mockState.callData = { list_items: list }

    const { result } = renderHook(() => useClipboardItems())
    await flushAsync()

    // 先移 id=2,再移 id=3
    act(() => {
      result.current.moveToFront(2)
    })
    act(() => {
      result.current.moveToFront(3)
    })

    // 最终顺序:3 在最前(后移的),2 其次,1 最后
    expect(result.current.items.map((i) => i.id)).toEqual([3, 2, 1])
  })
})

// ============================================================
// 置顶功能测试
// ============================================================
describe('置顶排序', () => {
  it('sortByPinned:置顶项在前,普通项在后', () => {
    const items = [
      makeItem(1, '普通A', { pinned: false, lastCopiedAt: 3000 }),
      makeItem(2, '置顶B', { pinned: true, pinnedAt: 1000, lastCopiedAt: 2000 }),
      makeItem(3, '普通C', { pinned: false, lastCopiedAt: 1000 }),
    ]
    const sorted = sortByPinned(items)
    expect(sorted.map((i) => i.id)).toEqual([2, 1, 3])
  })

  it('sortByPinned:置顶组内按 pinnedAt 倒序', () => {
    const items = [
      makeItem(1, '先置顶', { pinned: true, pinnedAt: 1000 }),
      makeItem(2, '后置顶', { pinned: true, pinnedAt: 2000 }),
    ]
    const sorted = sortByPinned(items)
    // pinnedAt 大的(后置顶)在前
    expect(sorted.map((i) => i.id)).toEqual([2, 1])
  })

  it('sortByPinned:普通组内按 lastCopiedAt 倒序', () => {
    const items = [
      makeItem(1, '旧', { pinned: false, lastCopiedAt: 1000 }),
      makeItem(2, '新', { pinned: false, lastCopiedAt: 2000 }),
    ]
    const sorted = sortByPinned(items)
    expect(sorted.map((i) => i.id)).toEqual([2, 1])
  })

  it('sortByPinned:不可变(不修改原数组)', () => {
    const items = [makeItem(1, 'A'), makeItem(2, 'B', { pinned: true, pinnedAt: 1 })]
    const original = [...items]
    sortByPinned(items)
    expect(items.map((i) => i.id)).toEqual(original.map((i) => i.id))
  })
})

describe('applyAddedItem 置顶分区', () => {
  it('新普通项不越过置顶项', () => {
    const items = [
      makeItem(1, '置顶', { pinned: true, pinnedAt: 1000 }),
      makeItem(2, '普通旧', { pinned: false, lastCopiedAt: 1000 }),
    ]
    const added = makeItem(3, '普通新', { pinned: false, lastCopiedAt: 9000 })
    const result = applyAddedItem(items, added)
    // 新条目应在置顶之后:1(置顶) → 3(新普通) → 2(旧普通)
    expect(result.map((i) => i.id)).toEqual([1, 3, 2])
  })

  it('新置顶项排最前', () => {
    const items = [
      makeItem(1, '置顶旧', { pinned: true, pinnedAt: 1000 }),
      makeItem(2, '普通', { pinned: false }),
    ]
    const added = makeItem(3, '置顶新', { pinned: true, pinnedAt: 9000 })
    const result = applyAddedItem(items, added)
    expect(result.map((i) => i.id)).toEqual([3, 1, 2])
  })
})

describe('moveToFront 置顶分区', () => {
  it('回填普通项:移到普通区最前,不越过置顶区', () => {
    const items = [
      makeItem(1, '置顶', { pinned: true, pinnedAt: 1000 }),
      makeItem(2, '普通A', { pinned: false, lastCopiedAt: 2000 }),
      makeItem(3, '普通B', { pinned: false, lastCopiedAt: 1000 }),
    ]
    // 移 id=3(普通B)到前面
    const result = moveToFront(items, 3)
    // 应该是:1(置顶) → 3(普通B 前移) → 2(普通A)
    expect(result.map((i) => i.id)).toEqual([1, 3, 2])
  })

  it('回填置顶项:移到置顶区最前(整个列表最前)', () => {
    const items = [
      makeItem(1, '置顶A', { pinned: true, pinnedAt: 2000 }),
      makeItem(2, '置顶B', { pinned: true, pinnedAt: 1000 }),
      makeItem(3, '普通', { pinned: false }),
    ]
    // 移 id=2(置顶B)到前面
    const result = moveToFront(items, 2)
    // 应该是:2(置顶B 前移) → 1(置顶A) → 3(普通)
    expect(result.map((i) => i.id)).toEqual([2, 1, 3])
  })
})
