/**
 * useClipboardItems — 剪贴板历史列表数据 hook
 *
 * 职责(单一):管理剪贴板列表数据状态。
 * - 初次加载:invoke('list_items')
 * - 实时更新:listen('clipboard://added') → 把新条目前置(去重)
 * - 删除:invoke('delete_item') → 从列表移除
 *
 * 不关心 UI 怎么展示(那是 ClipboardList 的事),不关心键盘交互(那是 SpotlightPanel 的事)。
 *
 * 随写随测(§1.22):列表更新逻辑抽成纯函数 applyAddedItem / removeItem,可单测。
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'

import { call } from '@core/api'
import { TAURI_EVENTS } from '@core/events'
import { ClipboardItemListSchema, ClipboardItemSchema } from '@core/types'
import type { ClipboardItem } from '@core/types'

/** 列表默认拉取条数(与 Rust DEFAULT_LIST_LIMIT 对齐) */
const DEFAULT_LIMIT = 50

/**
 * 置顶优先排序(不可变)。
 *
 * 排序规则:
 * 1. 置顶项(pinned=true)在前,普通项在后
 * 2. 置顶组内按 pinnedAt 倒序(最近置顶的在前)
 * 3. 普通组内按 lastCopiedAt 倒序(最近复制的在前)
 *
 * 纯函数,可单测。前端所有列表更新(togglePin/applyAddedItem/moveToFront)
 * 最终都经此函数保证「置顶在前」不变量。
 */
export function sortByPinned(items: ClipboardItem[]): ClipboardItem[] {
  return [...items].sort((a, b) => {
    // 置顶优先
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1
    // 置顶组:pinnedAt 倒序
    if (a.pinned && b.pinned) return (b.pinnedAt ?? 0) - (a.pinnedAt ?? 0)
    // 普通组:lastCopiedAt 倒序
    return b.lastCopiedAt - a.lastCopiedAt
  })
}

/**
 * 把新增/更新的条目插入列表(不可变更新),保持置顶优先排序。
 *
 * - 若已存在同 id → 移除旧的,插入更新后的
 * - 新条目经 sortByPinned 找到正确位置(普通项不越过置顶区)
 *
 * 纯函数,可单测。
 */
export function applyAddedItem(items: ClipboardItem[], added: ClipboardItem): ClipboardItem[] {
  const filtered = items.filter((it) => it.id !== added.id)
  return sortByPinned([added, ...filtered])
}

/** 从列表移除指定 id(不可变更新)。纯函数,可单测。 */
export function removeItem(items: ClipboardItem[], id: number): ClipboardItem[] {
  return items.filter((it) => it.id !== id)
}

/**
 * 把指定 id 的条目移到其分区的最前(不可变更新)。纯函数,可单测。
 *
 * - 置顶条目:移到置顶区最前
 * - 普通条目:移到普通区最前(不越过置顶区)
 *
 * M3:回填后将该条移到最前(与后端 UPDATE last_copied_at 对齐)。
 * 若 id 不存在,返回原列表(不变)。
 */
export function moveToFront(items: ClipboardItem[], id: number): ClipboardItem[] {
  const idx = items.findIndex((it) => it.id === id)
  if (idx === -1) return items
  const item = items[idx]
  const rest = items.slice(0, idx).concat(items.slice(idx + 1))

  // 普通项:插到第一个置顶项之后(即置顶区末尾 + 普通区开头)
  if (!item.pinned) {
    const lastPinnedIdx = rest.reduce(
      (last, it, i) => (it.pinned ? i : last),
      -1,
    )
    const insertAt = lastPinnedIdx + 1
    return [...rest.slice(0, insertAt), item, ...rest.slice(insertAt)]
  }

  // 置顶项:插到最前(置顶区开头)
  return [item, ...rest]
}

export interface UseClipboardItemsResult {
  items: ClipboardItem[]
  loading: boolean
  error: string | null
  /** 当前搜索查询(null = 非搜索模式,显示全列表) */
  searchQuery: string | null
  /** 当前类型筛选(空数组 = 不过滤) */
  kindFilter: string[]
  /** 设置类型筛选(空数组 = 清除筛选) */
  setKindFilter: (kinds: string[]) => void
  /** 删除一条(同步更新本地列表)。失败抛错给调用方处理。 */
  deleteItem: (id: number) => Promise<void>
  /** 搜索(空字符串/纯空白 = 清空搜索,回退全列表) */
  search: (query: string) => Promise<void>
  /** 把指定 id 的条目移到列表最前(M3:回填后前置)。同步更新本地列表。 */
  moveToFront: (id: number) => void
  /** 切换置顶状态(置顶/取消置顶)。同步更新本地列表排序。失败抛错给调用方处理。 */
  togglePin: (id: number) => Promise<void>
}

export function useClipboardItems(): UseClipboardItemsResult {
  const [items, setItems] = useState<ClipboardItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState<string | null>(null)
  const [kindFilter, setKindFilterState] = useState<string[]>([])
  // ref 跟踪 searchQuery / kindFilter,让 listen 回调和 reload(只挂载一次)能读到最新值
  const searchQueryRef = useRef<string | null>(null)
  const kindFilterRef = useRef<string[]>([])
  useEffect(() => {
    searchQueryRef.current = searchQuery
  }, [searchQuery])
  useEffect(() => {
    kindFilterRef.current = kindFilter
  }, [kindFilter])

  // 重新加载列表(用当前 searchQuery + kindFilter 的 ref 值)
  const reload = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const q = searchQueryRef.current
      const kinds = kindFilterRef.current
      const raw = q
        ? await call<unknown>('search_items', { query: q, limit: DEFAULT_LIMIT })
        : await call<unknown>('list_items', { limit: DEFAULT_LIMIT, kinds: kinds.length > 0 ? kinds : undefined })
      setItems(ClipboardItemListSchema.parse(raw))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  // 初次加载 + 监听实时更新(只挂载一次,依赖 [])
  useEffect(() => {
    let cancelled = false

    const load = async () => {
      try {
        const raw = await call<unknown>('list_items', { limit: DEFAULT_LIMIT })
        // §8 边界校验:后端响应进入即用 Zod 校验,畸形数据快速失败
        const list = ClipboardItemListSchema.parse(raw)
        if (!cancelled) {
          setItems(list)
          setLoading(false)
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e))
          setLoading(false)
        }
      }
    }

    void load()

    // 监听后端新增/更新事件(仅非搜索模式 + 无类型筛选时更新)
    const unlistenPromise = listen<unknown>(TAURI_EVENTS.CLIPBOARD_ADDED, (event) => {
      if (cancelled) return
      if (searchQueryRef.current !== null) return // 搜索模式:忽略新内容
      try {
        // §8 边界校验:事件 payload 同样经 Zod 校验
        const item = ClipboardItemSchema.parse(event.payload)
        // 有类型筛选时,检查新 item 是否匹配任一筛选条件
        const kinds = kindFilterRef.current
        if (kinds.length > 0) {
          const matches = kinds.some((filter) => {
            if (filter.startsWith('subtype:')) {
              return item.contentSubtype === filter.slice('subtype:'.length)
            }
            return item.contentKind === filter
          })
          if (!matches) return
        }
        setItems((prev) => applyAddedItem(prev, item))
      } catch (e) {
        console.error('[clipboard] added 事件 payload 校验失败:', e)
      }
    })

    return () => {
      cancelled = true
      // 处理 listen Promise rejection(注册失败时),避免 unhandled rejection
      unlistenPromise.catch((e) => console.error('[clipboard] listen 注册失败:', e))
      void unlistenPromise.then((fn) => fn())
    }
  }, [])

  const deleteItem = useCallback(async (id: number) => {
    await call('delete_item', { id })
    // 后端确认成功后,移除本地(不可变更新)
    setItems((prev) => removeItem(prev, id))
  }, [])

  // 设置类型筛选(同时同步 ref + 触发重新加载)
  // ⚠️ 必须先更新 ref 再 reload:reload 读的是 ref 的值,
  //    如果只更新 state,useEffect(ref 同步)在 render 后才跑,reload 会读到旧值。
  const setKindFilter = useCallback((kinds: string[]) => {
    kindFilterRef.current = kinds
    setKindFilterState(kinds)
    void reload()
  }, [reload])

  // useCallback 保证引用稳定(否则 debounce effect 会因依赖变化无限重建 → 闪烁)
  const search = useCallback(async (query: string) => {
    const trimmed = query.trim()
    if (trimmed === '') {
      // 清空搜索:回退到全列表(带 kindFilter)
      setSearchQuery(null)
      setLoading(true)
      try {
        const kinds = kindFilterRef.current
        const raw = await call<unknown>('list_items', { limit: DEFAULT_LIMIT, kinds: kinds.length > 0 ? kinds : undefined })
        setItems(ClipboardItemListSchema.parse(raw))
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
      } finally {
        setLoading(false)
      }
      return
    }
    setSearchQuery(trimmed)
    setLoading(true)
    try {
      const raw = await call<unknown>('search_items', { query: trimmed, limit: DEFAULT_LIMIT })
      setItems(ClipboardItemListSchema.parse(raw))
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  // M3:把指定 id 的条目移到列表最前(回填后前置)
  const moveToFrontAction = useCallback((id: number) => {
    setItems((prev) => moveToFront(prev, id))
  }, [])

  // 切换置顶:后端 toggle_pin 返回新状态 → 本地更新 + 重新排序
  const togglePin = useCallback(async (id: number) => {
    const newPinned = await call<boolean>('toggle_pin', { id })
    setItems((prev) =>
      sortByPinned(
        prev.map((it) =>
          it.id === id
            ? { ...it, pinned: newPinned, pinnedAt: newPinned ? Date.now() : null }
            : it,
        ),
      ),
    )
  }, [])

  return { items, loading, error, searchQuery, kindFilter, setKindFilter, deleteItem, search, moveToFront: moveToFrontAction, togglePin }
}
