/**
 * useSidecarReady — 轮询后端 sidecar 就绪状态
 *
 * 应用启动后，Rust setup 异步 spawn Python sidecar（PyInstaller onefile 首次解压
 * 需要数秒到数十秒）。本 hook 轮询 /api/health 直到就绪或超时，供 SetupGate 等
 * 「依赖后端」的入口组件等待 sidecar 起来。
 *
 * 设计要点：
 * - 每 2s 探测一次 /api/health，最长 90s
 * - 后端未起时 apiClient 抛 SIDECAR_NOT_READY，静默重试（不计入失败）
 * - 卸载时清理定时器（防内存泄漏 + StrictMode 双调用安全）
 * - 返回 status + retry()：超时后可手动重试
 */
import { useCallback, useEffect, useRef, useState } from 'react'

import { apiClient } from './client'

export type SidecarReadyStatus = 'checking' | 'ready' | 'timeout'

/** 轮询间隔（毫秒） */
const POLL_INTERVAL_MS = 2000
/** 最长等待（秒）—— 覆盖 PyInstaller onefile 首次解压 + 余量 */
const MAX_WAIT_SECS = 90

interface UseSidecarReadyResult {
  status: SidecarReadyStatus
  /** 手动重新开始轮询（超时后用户点「重试」用） */
  retry: () => void
}

/**
 * 轮询 /api/health 直到就绪或超时。
 *
 * @returns status='checking'（轮询中）/ 'ready'（health 200）/ 'timeout'（超时未通）
 *          + retry() 重置状态重新轮询
 */
export function useSidecarReady(): UseSidecarReadyResult {
  const [status, setStatus] = useState<SidecarReadyStatus>('checking')
  // retry 计数：retry() 递增它触发 effect 重跑（与 cleared effect 配合实现重置）
  const [retryCount, setRetryCount] = useState(0)
  const cancelledRef = useRef(false)

  const retry = useCallback(() => {
    setStatus('checking')
    setRetryCount((c) => c + 1)
  }, [])

  useEffect(() => {
    cancelledRef.current = false
    let timer: ReturnType<typeof setTimeout> | null = null

    const poll = async () => {
      const deadline = Date.now() + MAX_WAIT_SECS * 1000
      while (Date.now() < deadline && !cancelledRef.current) {
        try {
          await apiClient.get('/api/health')
          if (!cancelledRef.current) setStatus('ready')
          return
        } catch {
          // 后端未就绪，继续轮询
        }
        await new Promise<void>((resolve) => {
          timer = setTimeout(resolve, POLL_INTERVAL_MS)
        })
      }
      if (!cancelledRef.current) setStatus('timeout')
    }

    void poll()

    return () => {
      cancelledRef.current = true
      if (timer) clearTimeout(timer)
    }
  }, [retryCount])

  return { status, retry }
}
