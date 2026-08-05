/**
 * SpotlightPanel — Spotlight 风格主面板(M0 占位)
 *
 * 窗口形态:无边框 + 透明 + 圆角 + 阴影(由 tauri.conf.json 配置 + index.css 适配)。
 * 本组件是 #root 内的实体面板容器。
 *
 * M0 职责:
 * - 调 invoke('ping') 探活,验证前后端通信链路
 * - 显示「后端就绪 / 等待 M1 剪贴板功能」占位
 *
 * M1 起此组件改为剪贴板历史列表(上下键导航 + 点击回填)。
 *
 * 探活机制(constraints.md「Tauri v2 setup 时序坑」):
 * - setup 不 emit 事件(早于 WebView JS,即发即弃)
 * - 组件 mount 时 invoke ping,带重试 + 超时兜底
 * - 调通 = 后端就绪(invoke 本身就是答案)
 */
import { useEffect, useState } from 'react'

import { call } from '@core/api'
import { cn } from '@lib/utils'

type ProbeStatus = 'checking' | 'ready' | 'timeout'

/** 探活重试间隔(ms)—— invoke 早期偶发失败时重试 */
const PROBE_INTERVAL_MS = 100
/** 探活总超时(ms)—— 超过判定后端异常 */
const PROBE_TIMEOUT_MS = 5000

export function SpotlightPanel() {
  const [status, setStatus] = useState<ProbeStatus>('checking')

  useEffect(() => {
    let cancelled = false
    const deadline = window.setTimeout(() => {
      if (!cancelled) setStatus('timeout')
    }, PROBE_TIMEOUT_MS)

    const probe = async () => {
      if (cancelled) return
      try {
        await call<string>('ping')
        if (!cancelled) setStatus('ready')
      } catch {
        // invoke 失败(后端未就绪 / 序列化失败),重试
        if (!cancelled) {
          window.setTimeout(probe, PROBE_INTERVAL_MS)
        }
      }
    }
    void probe()

    return () => {
      cancelled = true
      window.clearTimeout(deadline)
    }
  }, [])

  return (
    <div
      data-tauri-drag-region
      className={cn(
        'flex h-full w-full flex-col items-center justify-center gap-3',
        'rounded-lg border border-border bg-background text-foreground shadow-2xl',
      )}
    >
      <h1 className="text-sm font-semibold tracking-wide text-foreground">
        Clipboard Manager
      </h1>

      {status === 'checking' && (
        <p className="text-xs text-muted-foreground">正在连接后端…</p>
      )}

      {status === 'ready' && (
        <>
          <p className="text-xs text-muted-foreground">
            <span className="text-primary">●</span> 后端就绪
          </p>
          <p className="text-xs text-muted-foreground/70">
            (M0 占位 · 等待 M1 剪贴板功能)
          </p>
        </>
      )}

      {status === 'timeout' && (
        <p className="text-xs text-destructive">
          后端连接超时,请重启应用
        </p>
      )}
    </div>
  )
}
