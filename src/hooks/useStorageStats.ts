/**
 * useStorageStats — 存储统计 hook
 *
 * 职责(单一):加载存储统计数据 + 清除后刷新。
 * - 初次加载:invoke('get_storage_stats')
 * - 清除历史:invoke('clear_history') → 成功后 refresh
 *
 * 不关心 UI 怎么展示(那是 StorageStatsPanel 的事)。
 */
import { useCallback, useEffect, useState } from 'react'

import { call } from '@core/api'
import { StorageStatsSchema } from '@core/types/models'
import type { StorageStats } from '@core/types/models'

export interface UseStorageStatsResult {
  stats: StorageStats | null
  loading: boolean
  /** 重新加载统计数据 */
  refresh: () => Promise<void>
  /** 清空所有历史(调后端 clear_history),成功后自动刷新 */
  clearAll: () => Promise<void>
}

export function useStorageStats(): UseStorageStatsResult {
  const [stats, setStats] = useState<StorageStats | null>(null)
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(async () => {
    try {
      const raw = await call<unknown>('get_storage_stats')
      // §8 边界校验:后端响应进入即用 Zod 校验
      const parsed = StorageStatsSchema.parse(raw)
      setStats(parsed)
    } catch (e) {
      console.error('[useStorageStats] 加载统计失败:', e)
    } finally {
      setLoading(false)
    }
  }, [])

  // 初次加载
  useEffect(() => {
    void refresh()
  }, [refresh])

  const clearAll = useCallback(async () => {
    await call<void>('clear_history')
    await refresh()
  }, [refresh])

  return { stats, loading, refresh, clearAll }
}
