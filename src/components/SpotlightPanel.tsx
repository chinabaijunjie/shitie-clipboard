/**
 * SpotlightPanel — Spotlight 风格主面板(M1 历史列表)
 *
 * 窗口形态:无边框 + 透明 + 圆角 + 阴影(tauri.conf.json + index.css)。
 *
 * M1 职责(容器):
 * - 装配 useClipboardItems(数据)+ ClipboardList(展示)
 * - 键盘交互:↑↓ 导航 / ⏎ 回填 / ⌫ 删除
 * - 探活:M0 遗留的 ping 探活(列表加载成功即代表就绪)
 *
 * 探活机制(constraints.md「Tauri v2 setup 时序坑」):用 invoke 调通即就绪。
 *
 * 模块化(§1.21):本组件是容器,数据逻辑在 hook,展示在 ClipboardList,键盘导航
 * 边界计算在 nextIndex 纯函数(可单测)。
 */
import { useCallback, useEffect, useState } from 'react'

import { call } from '@core/api'
import { useClipboardItems } from '@hooks/useClipboardItems'
import { cn } from '@lib/utils'
import { ClipboardList } from '@components/ClipboardList'

type ProbeStatus = 'checking' | 'ready' | 'timeout'

/** 探活超时(ms) */
const PROBE_TIMEOUT_MS = 5000

/**
 * 计算键盘导航后的新索引(纯函数,可单测)。
 *
 * - 方向 -1 = 上,方向 +1 = 下
 * - 循环:到顶按上 → 跳到底;到底按下 → 跳到顶
 * - 空列表 → 始终 -1
 */
export function nextIndex(current: number, direction: -1 | 1, length: number): number {
  if (length === 0) return -1
  // (current + direction) 模 length,处理负数(上到顶循环到底)
  return ((current + direction) % length + length) % length
}

export function SpotlightPanel() {
  const { items, loading, error, deleteItem } = useClipboardItems()
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [probe, setProbe] = useState<ProbeStatus>('checking')
  const [actionError, setActionError] = useState<string | null>(null)

  /** 设置操作错误(回填/删除失败时),3 秒后自动清除 */
  const showActionError = useCallback((msg: string) => {
    setActionError(msg)
    window.setTimeout(() => setActionError(null), 3000)
  }, [])

  // 探活:M0 遗留逻辑,列表加载成功即代表后端就绪
  useEffect(() => {
    let cancelled = false
    const deadline = window.setTimeout(() => {
      if (!cancelled) setProbe('timeout')
    }, PROBE_TIMEOUT_MS)

    const probeOnce = async () => {
      if (cancelled) return
      try {
        await call<string>('ping')
        if (!cancelled) {
          window.clearTimeout(deadline)
          setProbe('ready')
        }
      } catch {
        if (!cancelled) window.setTimeout(probeOnce, 100)
      }
    }
    void probeOnce()

    return () => {
      cancelled = true
      window.clearTimeout(deadline)
    }
  }, [])

  // 选中索引边界保护:items 变化时不能越界
  useEffect(() => {
    if (selectedIndex >= items.length) {
      setSelectedIndex(items.length > 0 ? 0 : -1)
    }
  }, [items.length, selectedIndex])

  // 回填选中条目
  const handleCopyBack = useCallback(
    async (index: number) => {
      const item = items[index]
      if (!item) return
      try {
        await call<string>('copy_back', { id: item.id })
      } catch (e) {
        // UI 层给用户友好提示(§8「绝不静默吞错」),保留 console.error 供调试
        console.error('[copy_back] 失败:', e)
        showActionError(`回填失败:${e instanceof Error ? e.message : String(e)}`)
      }
    },
    [items, showActionError],
  )

  // 删除选中条目
  const handleDelete = useCallback(
    async (index: number) => {
      const item = items[index]
      if (!item) return
      try {
        await deleteItem(item.id)
      } catch (e) {
        console.error('[delete] 失败:', e)
        showActionError(`删除失败:${e instanceof Error ? e.message : String(e)}`)
      }
    },
    [items, deleteItem, showActionError],
  )

  // 键盘交互
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (items.length === 0) return
      switch (e.key) {
        case 'ArrowUp':
          e.preventDefault()
          setSelectedIndex((i) => nextIndex(i, -1, items.length))
          break
        case 'ArrowDown':
          e.preventDefault()
          setSelectedIndex((i) => nextIndex(i, 1, items.length))
          break
        case 'Enter':
          e.preventDefault()
          void handleCopyBack(selectedIndex)
          break
        case 'Backspace':
        case 'Delete':
          e.preventDefault()
          void handleDelete(selectedIndex)
          break
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [items.length, selectedIndex, handleCopyBack, handleDelete])

  return (
    <div
      data-tauri-drag-region
      className={cn(
        'flex h-full w-full flex-col rounded-lg border border-border bg-background text-foreground shadow-2xl',
      )}
    >
      {/* 顶栏:标题 + 状态 */}
      <div className="flex items-center justify-between border-b border-border/50 px-3 py-2">
        <span className="text-xs font-semibold tracking-wide">Clipboard Manager</span>
        <span className="text-[10px] text-muted-foreground">
          {probe === 'ready' ? (
            <span className="text-primary">●</span>
          ) : probe === 'checking' ? (
            '连接中…'
          ) : (
            <span className="text-destructive">● 超时</span>
          )}
        </span>
      </div>

      {/* 列表区(可滚动:超出窗口高度的内容滚动显示) */}
      <div className="flex-1 overflow-y-auto">
        {error ? (
          <p className="py-8 text-center text-xs text-destructive">加载失败:{error}</p>
        ) : loading ? (
          <p className="py-8 text-center text-xs text-muted-foreground">加载中…</p>
        ) : (
          <ClipboardList
            items={items}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
          />
        )}
      </div>

      {/* 底栏:快捷键提示 / 操作错误(有错误时覆盖显示) */}
      <div className="border-t border-border/50 px-3 py-1.5 text-[10px]">
        {actionError ? (
          <span className="text-destructive">{actionError}</span>
        ) : (
          <span className="text-muted-foreground/70">↑↓ 导航 · ⏎ 回填 · ⌫ 删除</span>
        )}
      </div>
    </div>
  )
}
