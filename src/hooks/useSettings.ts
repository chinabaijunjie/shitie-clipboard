/**
 * useSettings — 设置数据 hook
 *
 * 职责(单一):管理用户设置的加载 + 更新。
 * - 初次加载:invoke('get_settings')
 * - 更新:invoke('update_settings', { ...newSettings })
 * - autostart 状态:直接调 @tauri-apps/plugin-autostart(不走 settings.json)
 *
 * 不关心 UI 怎么展示(那是 SettingsPanel 的事)。
 */
import { useCallback, useEffect, useState } from 'react'

import { call } from '@core/api'
import { SettingsSchema } from '@core/settings/models'
import type { Settings } from '@core/settings/models'

export interface UseSettingsResult {
  settings: Settings | null
  loading: boolean
  error: string | null
  /** 更新设置(完整替换)。失败抛错给调用方处理。 */
  update: (newSettings: Settings) => Promise<void>
}

export function useSettings(): UseSettingsResult {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // 初次加载
  useEffect(() => {
    let cancelled = false

    const load = async () => {
      try {
        const raw = await call<unknown>('get_settings')
        // §8 边界校验:后端响应进入即用 Zod 校验
        const parsed = SettingsSchema.parse(raw)
        if (!cancelled) {
          setSettings(parsed)
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

    return () => {
      cancelled = true
    }
  }, [])

  const update = useCallback(async (newSettings: Settings) => {
    const raw = await call<unknown>('update_settings', { newSettings })
    const parsed = SettingsSchema.parse(raw)
    setSettings(parsed)
  }, [])

  return { settings, loading, error, update }
}
