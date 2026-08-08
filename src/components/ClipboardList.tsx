/**
 * ClipboardList — 剪贴板历史列表(纯渲染组件)
 *
 * 职责(单一):给定 items + selectedIndex,渲染列表。
 * 不关心数据怎么来(那是 useClipboardItems),不关心键盘交互(那是 SpotlightPanel)。
 *
 * 纯展示组件,便于用 testing-library 测试渲染输出。
 */
import { convertFileSrc } from '@tauri-apps/api/core'

import { cn } from '@lib/utils'
import type { ClipboardItem } from '@core/types'

interface ClipboardListProps {
  items: ClipboardItem[]
  /** 当前选中索引(-1 表示无选中) */
  selectedIndex: number
  /** 选中条目被点击时触发(鼠标点击) */
  onSelect?: (index: number) => void
}

/** 列表项预览的最大字符数(过长截断显示,完整内容在 title) */
const PREVIEW_MAX_CHARS = 60

/** 格式化时间为 HH:MM */
function formatTime(epochMs: number): string {
  const d = new Date(epochMs)
  const h = String(d.getHours()).padStart(2, '0')
  const m = String(d.getMinutes()).padStart(2, '0')
  return `${h}:${m}`
}

export function ClipboardList({ items, selectedIndex, onSelect }: ClipboardListProps) {
  if (items.length === 0) {
    return (
      <p className="py-8 text-center text-xs text-muted-foreground/60">
        还没有复制记录,复制点东西试试…
      </p>
    )
  }

  return (
    <ul className="flex flex-col">
      {items.map((item, index) => (
        <li
          key={item.id}
          onClick={() => onSelect?.(index)}
          className={cn(
            'flex cursor-default items-center gap-2 px-3 py-2 text-xs',
            index === selectedIndex
              ? 'bg-accent text-accent-foreground'
              : 'text-foreground hover:bg-accent/50',
          )}
        >
          {/* 图片类:显示缩略图 */}
          {item.thumbPath ? (
            <img
              src={convertFileSrc(item.thumbPath)}
              alt=""
              className="h-8 w-8 shrink-0 rounded object-cover"
              draggable={false}
            />
          ) : (
            <span className="shrink-0 w-2" />
          )}
          <span className="flex-1 truncate">
            {item.title.slice(0, PREVIEW_MAX_CHARS) || '<空>'}
            {item.title.length > PREVIEW_MAX_CHARS && '…'}
          </span>
          {item.copyCount > 1 && (
            <span className="shrink-0 rounded bg-muted px-1 text-[10px] text-muted-foreground">
              ×{item.copyCount}
            </span>
          )}
          <span className="shrink-0 text-[10px] text-muted-foreground/70">
            {formatTime(item.lastCopiedAt)}
          </span>
        </li>
      ))}
    </ul>
  )
}
