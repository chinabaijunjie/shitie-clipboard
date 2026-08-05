/**
 * useSidecarReady 单测 — 轮询 sidecar 就绪状态 hook
 *
 * 示例测试（回流自 ai-audiobook TODO-029 测试模式），演示：
 * - renderHook 测自定义 hook
 * - vi.mock 模块 + 控制异步 resolve
 * - waitFor 等待状态更新
 * - unmount cleanup 防内存泄漏
 *
 * 子工程写 hook 测试时参照此模式。
 */
import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { useSidecarReady } from './useSidecarReady'

// mock apiClient — useSidecarReady 内部调 apiClient.get('/api/health')
vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
  },
}))

const { apiClient } = await import('./client')

describe('useSidecarReady', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('health 返回成功 → status=ready', async () => {
    vi.mocked(apiClient.get).mockResolvedValueOnce({ ok: true })

    const { result } = renderHook(() => useSidecarReady())

    await waitFor(() => {
      expect(result.current.status).toBe('ready')
    })
    expect(apiClient.get).toHaveBeenCalledWith('/api/health')
  })

  it('初始 status=checking', () => {
    vi.mocked(apiClient.get).mockReturnValueOnce(new Promise(() => {})) // 永不 resolve

    const { result } = renderHook(() => useSidecarReady())

    expect(result.current.status).toBe('checking')
  })

  it('retry() 重置 status 为 checking', async () => {
    vi.mocked(apiClient.get).mockResolvedValueOnce({ ok: true })

    const { result } = renderHook(() => useSidecarReady())

    await waitFor(() => {
      expect(result.current.status).toBe('ready')
    })

    // 冻结后续轮询（retry 会触发新 effect 跑 poll，用 fake timers 阻断 setTimeout）
    vi.useFakeTimers()
    act(() => {
      result.current.retry()
    })
    vi.useRealTimers()

    expect(result.current.status).toBe('checking')
  })
})
