/**
 * 类型安全的 localStorage 封装
 *
 * - 自动 JSON 序列化/反序列化
 * - 统一 key 前缀，避免与其他应用冲突
 * - 读取异常时返回 fallback，不抛错
 */

import { STORAGE_KEY_PREFIX } from '@core/constants'

type Serializable = string | number | boolean | object | null | undefined

function buildKey(key: string): string {
  return key.startsWith(STORAGE_KEY_PREFIX) ? key : `${STORAGE_KEY_PREFIX}${key}`
}

export const storage = {
  get<T extends Serializable>(key: string, fallback: T): T {
    try {
      const raw = localStorage.getItem(buildKey(key))
      if (raw === null) return fallback
      return JSON.parse(raw) as T
    } catch (error) {
      console.warn(`[storage] 读取 ${key} 失败:`, error)
      return fallback
    }
  },

  set<T extends Serializable>(key: string, value: T): void {
    try {
      localStorage.setItem(buildKey(key), JSON.stringify(value))
    } catch (error) {
      console.error(`[storage] 写入 ${key} 失败:`, error)
    }
  },

  remove(key: string): void {
    localStorage.removeItem(buildKey(key))
  },

  has(key: string): boolean {
    return localStorage.getItem(buildKey(key)) !== null
  },

  clear(): void {
    const prefix = STORAGE_KEY_PREFIX
    const keysToRemove: string[] = []
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i)
      if (k && k.startsWith(prefix)) keysToRemove.push(k)
    }
    keysToRemove.forEach((k) => localStorage.removeItem(k))
  },
}
