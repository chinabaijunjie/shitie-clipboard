/**
 * 🔬 SpikeClipboardProbe — M1 剪贴板监听可行性验证(临时组件,验证完删除)
 *
 * 验证 m1-research.md 第四节两个未知项:
 * 1. 🔴 clipboard-rs watcher 后台线程 → emit 前端事件链路是否通(NSPasteboard 线程崩溃坑)
 * 2. 🟡 真实复制场景下 formats + 文本预览是否正常
 *
 * 两条验证路径:
 * - 异步:listen('spike://clipboard-changed') — watcher 检测到变化时自动推
 * - 同步:点按钮 invoke('spike_read_clipboard') — 主动读一次
 *
 * 操作:启动后随便复制几段文字/从 Word 复制富文本,观察下方列表是否实时更新。
 */
import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'

import { call } from '@core/api'

interface ClipboardChange {
  formats: string[]
  preview: string
  len?: number
}

export function SpikeClipboardProbe() {
  const [changes, setChanges] = useState<ClipboardChange[]>([])
  const [error, setError] = useState<string | null>(null)

  // 异步路径:监听 watcher emit 的事件
  useEffect(() => {
    let cancelled = false
    const unlisten = listen<ClipboardChange>('spike://clipboard-changed', (event) => {
      if (cancelled) return
      console.log('[spike] 收到剪贴板变化事件:', event.payload)
      // 不可变更新:新条目放最前,保留最近 10 条
      setChanges((prev) => [{ ...event.payload }, ...prev].slice(0, 10))
    })

    return () => {
      cancelled = true
      void unlisten.then((fn) => fn())
    }
  }, [])

  // 同步路径:主动 invoke 读一次
  const handleManualRead = async () => {
    setError(null)
    try {
      const result = await call<ClipboardChange>('spike_read_clipboard')
      console.log('[spike] 手动读取结果:', result)
      setChanges((prev) => [{ ...result, len: result.preview.length }, ...prev].slice(0, 10))
    } catch (e) {
      console.error('[spike] 手动读取失败:', e)
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <div className="flex w-full max-w-[520px] flex-col gap-2 border-t border-border/50 pt-3">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
          🔬 Spike · 剪贴板监听验证
        </span>
        <button
          onClick={handleManualRead}
          className="rounded border border-border px-2 py-0.5 text-[10px] text-muted-foreground hover:bg-accent"
        >
          手动读一次
        </button>
      </div>

      {error && <p className="text-[10px] text-destructive">错误: {error}</p>}

      {changes.length === 0 ? (
        <p className="text-[10px] text-muted-foreground/60">
          复制点东西试试(观察 Console + 这里是否实时更新)…
        </p>
      ) : (
        <ul className="flex max-h-[200px] flex-col gap-1 overflow-y-auto">
          {changes.map((c, i) => (
            <li key={i} className="rounded bg-accent/30 px-2 py-1 text-[10px]">
              <div className="truncate text-foreground">{c.preview || '<空文本>'}</div>
              <div className="mt-0.5 text-[9px] text-muted-foreground/70">
                {c.len != null && <span>len={c.len} · </span>}
                formats: {c.formats.join(', ') || '无'}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
