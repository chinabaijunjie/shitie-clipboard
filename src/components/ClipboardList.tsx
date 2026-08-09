/**
 * ClipboardList — 剪贴板历史列表(纯渲染组件)
 *
 * 职责(单一):给定 items + selectedIndex,渲染列表。
 * 不关心数据怎么来(那是 useClipboardItems),不关心键盘交互(那是 SpotlightPanel)。
 *
 * 苹果审美改造(深色 ①):
 * - 胶囊选中态(rounded-card + bg-selected)替代方块 bg-accent
 * - hover 过渡(duration-fast ease-out-strong)+ 按压反馈(active:scale-[0.985])
 * - 类型图标预留位(16px 固定宽,纯文字占位保持对齐)—— 为未来智能渲染铺路
 * - 字距分级(时间戳 tracking-caption 正字距)
 *
 * 纯展示组件,便于用 testing-library 测试渲染输出。
 */
import { useEffect } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { FileText, Clipboard as ClipboardIcon } from 'lucide-react'

import { cn } from '@lib/utils'
import type { ClipboardItem } from '@core/types'

interface ClipboardListProps {
  items: ClipboardItem[]
  /** 当前选中索引(-1 表示无选中) */
  selectedIndex: number
  /** 选中条目被点击时触发(鼠标单击) */
  onSelect?: (index: number) => void
  /** 选中条目被双击时触发(回填到剪贴板并关闭窗口) */
  onCopyBack?: (index: number) => void
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

export function ClipboardList({ items, selectedIndex, onSelect, onCopyBack }: ClipboardListProps) {
  // 选中索引变化时,自动把选中项滚动到可视区(block:nearest 只在不可见时滚,避免抖动)
  useEffect(() => {
    if (selectedIndex < 0) return
    const el = document.querySelector<HTMLElement>(`li[data-index="${selectedIndex}"]`)
    // nearest:选中项已在可视区则不滚动;否则最小幅度滚动让它可见
    el?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2.5 py-12 text-center">
        <ClipboardIcon
          className="h-9 w-9 text-text-tertiary"
          strokeWidth={1.5}
          aria-hidden
        />
        <p className="text-xs text-text-secondary">还没有复制记录</p>
        <p className="text-[11px] text-text-tertiary">
          复制点东西试试,它会自动出现在这里
        </p>
      </div>
    )
  }

  return (
    <ul className="flex flex-col overflow-hidden">
      {items.map((item, index) => {
        const isSelected = index === selectedIndex
        const isImage = !!item.thumbPath
        const isFile = item.contentKind === 'file'

        return (
          <li
            key={item.id}
            data-index={index}
            onClick={() => onSelect?.(index)}
            onDoubleClick={() => onCopyBack?.(index)}
            style={{
              ...(isSelected
                ? {
                    backgroundColor: 'var(--bg-selected)',
                    boxShadow: 'inset 0 0.5px 0 var(--selected-highlight)',
                  }
                : {}),
              display: 'grid',
              // 列1 auto(图标/缩略图) 列2 1fr(内容) 列3 auto(时间)
              gridTemplateColumns: 'auto 1fr auto',
              // 选中时顶部对齐(图片放大后内容跟缩略图顶部对齐)
              alignItems: isSelected && isImage ? 'start' : 'center',
              columnGap: '8px',
            }}
            className={cn(
              'mx-2 my-1 cursor-default overflow-hidden rounded-card px-3.5 py-2.5',
              'list-item transition-[background,transform] duration-fast ease-out-strong',
              'active:scale-[0.985]',
              isSelected ? 'text-primary' : 'text-foreground/90',
            )}
          >
            {/* 列1:图标/缩略图 —— 选中图片时放大到 40×40
                isImage && item.thumbPath 双条件让 TS 自动窄化为 string,免除非空断言 */}
            {isImage && item.thumbPath ? (
              <img
                src={convertFileSrc(item.thumbPath)}
                alt=""
                className={cn(
                  'rounded-thumb object-cover',
                  isSelected ? 'h-10 w-10' : 'h-5 w-5',
                )}
                draggable={false}
              />
            ) : isFile ? (
              <FileText
                className="h-4 w-4"
                style={{ color: 'var(--type-file)' }}
                aria-hidden
              />
            ) : null}

            {/* 列2:主内容
                非选中:JS 截断 60 字符 + 单行 ellipsis(紧凑)
                选中文字:显示完整 title,line-clamp-3 限制最多三行(让 CSS 决定截断点)
                选中图片:仍单行(图片本身已放大) */}
            <span
              className={cn(
                'min-w-0 overflow-hidden text-xs',
                isSelected && !isImage
                  ? 'line-clamp-3 whitespace-normal break-all'
                  : 'text-ellipsis whitespace-nowrap',
              )}
              title={item.title}
            >
              {isSelected && !isImage
                ? (item.title || '<空>')
                : (item.title.slice(0, PREVIEW_MAX_CHARS) || '<空>') + (item.title.length > PREVIEW_MAX_CHARS ? '…' : '')}
            </span>

            {/* 列3:时间戳 */}
            <span className={cn(
              'whitespace-nowrap text-[10px] tracking-caption',
              isSelected ? 'text-text-secondary' : 'text-text-tertiary',
            )}>
              {formatTime(item.lastCopiedAt)}
            </span>
          </li>
        )
      })}
    </ul>
  )
}
