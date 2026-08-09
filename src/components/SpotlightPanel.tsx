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
import { useCallback, useEffect, useRef, useState } from 'react'
import { Settings, Search } from 'lucide-react'

import { call } from '@core/api'
import { hideWindow } from '@core/window'
import { useClipboardItems } from '@hooks/useClipboardItems'
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

interface SpotlightPanelProps {
  onOpenSettings: () => void
}

export function SpotlightPanel({ onOpenSettings }: SpotlightPanelProps) {
  const { items, loading, error, searchQuery, deleteItem, search, moveToFront } = useClipboardItems()
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [probe, setProbe] = useState<ProbeStatus>('checking')
  const [actionError, setActionError] = useState<string | null>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const [searchText, setSearchText] = useState('')

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

  // 搜索:searchText 变化后 debounce 300ms 触发(避免每次按键都查询)
  useEffect(() => {
    const handle = window.setTimeout(() => {
      void search(searchText)
    }, 300)
    return () => window.clearTimeout(handle)
  }, [searchText, search])

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
        // M3:回填成功后 → 前置该 item(移到最前)+ 隐藏窗口
        moveToFront(item.id)
        // 搜索模式下回填:清空搜索(回退全列表)
        if (searchText) {
          setSearchText('')
        }
        await hideWindow()
      } catch (e) {
        // UI 层给用户友好提示(§8「绝不静默吞错」),保留 console.error 供调试
        console.error('[copy_back] 失败:', e)
        showActionError(`回填失败:${e instanceof Error ? e.message : String(e)}`)
      }
    },
    [items, showActionError, moveToFront, searchText],
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
      const searchFocused = document.activeElement === searchInputRef.current
      // 搜索框聚焦时:Esc 有搜索→清空 / 无搜索→隐藏窗口;其余交给输入框
      if (searchFocused) {
        if (e.key === 'Escape') {
          e.preventDefault()
          if (searchText) {
            setSearchText('')
            searchInputRef.current?.blur()
          } else {
            void hideWindow()
          }
        }
        return
      }
      // 非搜索框聚焦:列表导航 + / 聚焦搜索 + Esc 隐藏
      switch (e.key) {
        case '/':
          e.preventDefault()
          searchInputRef.current?.focus()
          break
        case 'Escape':
          e.preventDefault()
          void hideWindow()
          break
        case 'ArrowUp':
          if (items.length === 0) return
          e.preventDefault()
          setSelectedIndex((i) => nextIndex(i, -1, items.length))
          break
        case 'ArrowDown':
          if (items.length === 0) return
          e.preventDefault()
          setSelectedIndex((i) => nextIndex(i, 1, items.length))
          break
        case 'Enter':
          if (items.length === 0) return
          e.preventDefault()
          void handleCopyBack(selectedIndex)
          break
        case 'Backspace':
        case 'Delete':
          if (items.length === 0) return
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
      className="glass-window flex h-full w-full flex-col text-foreground"
    >
      {/* 顶栏:标题 + 设置 + 状态
          - data-tauri-drag-region 支持拖动窗口
          - select-none 防止文字被选中 */}
      <div
        data-tauri-drag-region
        className="flex items-center justify-between px-3.5 py-2.5 select-none"
      >
        <span
          data-tauri-drag-region
          className="text-xs font-semibold tracking-tightish select-none text-foreground"
        >
          Clipboard Manager
        </span>
        <div className="flex items-center gap-1.5">
          <button
            onClick={onOpenSettings}
            className="flex h-5 w-5 items-center justify-center rounded-card-small text-text-secondary transition-colors duration-fast ease-out-strong hover:bg-hover hover:text-foreground"
            title="设置"
            aria-label="设置"
          >
            <Settings className="h-3.5 w-3.5" aria-hidden />
          </button>
          {probe === 'ready' ? (
            <span
              className="h-[7px] w-[7px] rounded-full"
              style={{
                backgroundColor: 'hsl(var(--status-ready))',
                boxShadow: '0 0 6px hsl(var(--status-ready) / 0.6)',
              }}
              title="后端就绪"
            />
          ) : probe === 'checking' ? (
            <span className="text-[10px] text-text-tertiary">连接中…</span>
          ) : (
            <span
              className="h-[7px] w-[7px] rounded-full bg-destructive"
              title="连接超时"
            />
          )}
        </div>
      </div>

      {/* 搜索框 —— 圆角卡片样式(左右留白 + 半透明背景 + 圆角边框) */}
      <div className="px-3 py-2">
        <div className="flex items-center gap-2 rounded-card border border-divider bg-card-raised px-3 py-1.5">
          <Search className="h-[13px] w-[13px] shrink-0 text-text-tertiary" aria-hidden />
          <input
            ref={searchInputRef}
            type="text"
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            placeholder="搜索剪贴板历史…  (按 / 快速聚焦)"
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground placeholder:text-text-tertiary focus:outline-none"
          />
          {searchQuery && (
            <span className="shrink-0 text-[10px] text-text-tertiary">
              {items.length} 条结果
            </span>
          )}
        </div>
      </div>

      {/* 列表区(可滚动 + 隐形细滚动条 + 禁止横向溢出) */}
      <div className="scrollbar-thin flex-1 overflow-y-auto overflow-x-hidden py-1">
        {error ? (
          <p className="py-8 text-center text-xs text-destructive">加载失败:{error}</p>
        ) : loading ? (
          <p className="py-8 text-center text-xs text-text-secondary">加载中…</p>
        ) : (
          <ClipboardList
            items={items}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
            onCopyBack={(index) => void handleCopyBack(index)}
          />
        )}
      </div>

      {/* 底栏:快捷键提示 / 操作错误 —— 顶部半透明分隔线 */}
      <div className="border-t border-divider px-3.5 py-1.5 text-[10px]">
        {actionError ? (
          <span className="text-destructive">{actionError}</span>
        ) : (
          <span className="text-text-tertiary">
            <kbd className="mx-0.5 rounded-[4px] bg-badge-bg px-1 py-px text-[9.5px] text-badge-text">↑↓</kbd>
            {' '}导航{' · '}
            <kbd className="mx-0.5 rounded-[4px] bg-badge-bg px-1 py-px text-[9.5px] text-badge-text">⏎</kbd>
            <kbd className="mx-0.5 rounded-[4px] bg-badge-bg px-1 py-px text-[9.5px] text-badge-text">双击</kbd>
            {' '}回填{' · '}
            <kbd className="mx-0.5 rounded-[4px] bg-badge-bg px-1 py-px text-[9.5px] text-badge-text">⌫</kbd>
            {' '}删除{' · '}
            <kbd className="mx-0.5 rounded-[4px] bg-badge-bg px-1 py-px text-[9.5px] text-badge-text">/</kbd>
            {' '}搜索
          </span>
        )}
      </div>
    </div>
  )
}
