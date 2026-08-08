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
 * 把新增/更新的条目前置到列表(不可变更新)。
 *
 * - 若已存在同 id → 移到最前,更新字段(copy_count/lastCopiedAt 等)
 * - 若不存在 → 放到最前
 *
 * 纯函数,可单测。
 */
export function applyAddedItem(items: ClipboardItem[], added: ClipboardItem): ClipboardItem[] {
  const filtered = items.filter((it) => it.id !== added.id)
  return [added, ...filtered]
}

/** 从列表移除指定 id(不可变更新)。纯函数,可单测。 */
export function removeItem(items: ClipboardItem[], id: number): ClipboardItem[] {
  return items.filter((it) => it.id !== id)
}

/**
 * 把指定 id 的条目移到列表最前(不可变更新)。纯函数,可单测。
 *
 * M3:回填后将该条移到最前(与后端 UPDATE last_copied_at 对齐)。
 * 若 id 不存在,返回原列表(不变)。
 */
export function moveToFront(items: ClipboardItem[], id: number): ClipboardItem[] {
  const idx = items.findIndex((it) => it.id === id)
  if (idx === -1) return items
  if (idx === 0) return items // 已在最前
  const item = items[idx]
  return [item, ...items.slice(0, idx), ...items.slice(idx + 1)]
}

export interface UseClipboardItemsResult {
  items: ClipboardItem[]
  loading: boolean
  error: string | null
  /** 当前搜索查询(null = 非搜索模式,显示全列表) */
  searchQuery: string | null
  /** 删除一条(同步更新本地列表)。失败抛错给调用方处理。 */
  deleteItem: (id: number) => Promise<void>
  /** 搜索(空字符串/纯空白 = 清空搜索,回退全列表) */
  search: (query: string) => Promise<void>
  /** 把指定 id 的条目移到列表最前(M3:回填后前置)。同步更新本地列表。 */
  moveToFront: (id: number) => void
}

export function useClipboardItems(): UseClipboardItemsResult {
  const [items, setItems] = useState<ClipboardItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState<string | null>(null)
  // ref 跟踪 searchQuery,让 listen 回调(只挂载一次)能读到最新值,避免闭包过时
  const searchQueryRef = useRef<string | null>(null)
  useEffect(() => {
    searchQueryRef.current = searchQuery
  }, [searchQuery])

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

    // 监听后端新增/更新事件(仅非搜索模式下更新;搜索时忽略新内容,清空搜索后自然可见)
    const unlistenPromise = listen<unknown>(TAURI_EVENTS.CLIPBOARD_ADDED, (event) => {
      if (cancelled) return
      if (searchQueryRef.current !== null) return // 搜索模式:忽略新内容
      try {
        // §8 边界校验:事件 payload 同样经 Zod 校验
        const item = ClipboardItemSchema.parse(event.payload)
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

  // useCallback 保证引用稳定(否则 debounce effect 会因依赖变化无限重建 → 闪烁)
  const search = useCallback(async (query: string) => {
    const trimmed = query.trim()
    if (trimmed === '') {
      // 清空搜索:回退到全列表
      setSearchQuery(null)
      setLoading(true)
      try {
        const raw = await call<unknown>('list_items', { limit: DEFAULT_LIMIT })
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

  return { items, loading, error, searchQuery, deleteItem, search, moveToFront: moveToFrontAction }
}
