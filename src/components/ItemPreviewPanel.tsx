/**
 * ItemPreviewPanel — 右侧预览面板(纯展示)
 *
 * 职责(单一):展示选中条目的完整内容 + 元信息。
 * - 上半内容区:图片原图(可缩小适配)/ 文件信息 / 完整文字(可滚动)
 * - 下半元信息区:类型 / 首次复制 / 最近复制 / 复制次数 / 路径 / 置顶
 *
 * 不关心列表数据怎么来(useClipboardItems),不关心全文怎么取
 * (useItemFullText 在 SpotlightPanel 装配,fullText 作为 prop 传入)。
 *
 * 样式只用现有 CSS 变量/工具类(三主题自动适配,见 ui-design-spec)。
 */
import { convertFileSrc } from '@tauri-apps/api/core'
import { Pin, Clipboard as ClipboardIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import type { ClipboardItem } from '@core/types'
import { getTypeMeta, isFileItem, isImageItem } from '@core/type-meta'
import { cn } from '@lib/utils'

interface ItemPreviewPanelProps {
  /** 当前选中条目(null = 空态) */
  item: ClipboardItem | null
  /** 应展示的文本(useItemFullText 解析后的最终值) */
  fullText: string
}

/** 完整时间格式 YYYY-MM-DD HH:mm:ss。纯函数,可单测。
 *  列表是 HH:MM 紧凑格式;预览区跨天历史需要日期,避免误导。 */
export function formatDateTime(epochMs: number): string {
  const d = new Date(epochMs)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

/** 元信息单行:固定宽 label + 可折行 value */
function MetaRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-2 text-[10px] leading-relaxed">
      <span className="w-14 shrink-0 select-none text-text-tertiary">{label}</span>
      <span className="min-w-0 break-all text-text-secondary">{children}</span>
    </div>
  )
}

export function ItemPreviewPanel({ item, fullText }: ItemPreviewPanelProps) {
  // 空态:无选中(空列表/未聚焦)
  if (!item) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2.5 border-l border-divider text-center">
        <ClipboardIcon className="h-9 w-9 text-text-tertiary" strokeWidth={1.5} aria-hidden />
        <p className="flex flex-col gap-0.5 text-[11px] text-text-tertiary">
          <span>选择一条记录</span>
          <span>查看详情</span>
        </p>
      </div>
    )
  }

  const meta = getTypeMeta(item)
  const TypeIcon = meta.icon
  const image = isImageItem(item)

  return (
    <div className="flex h-full min-h-0 flex-col border-l border-divider">
      {/* 内容区:图片/文件居中,文字顶部左对齐(避免 flex 居中 + overflow 裁掉顶部) */}
      <div
        className={cn(
          'scrollbar-thin flex min-h-0 flex-1 overflow-y-auto p-3',
          image || isFileItem(item) ? 'items-center justify-center' : 'items-start justify-start',
        )}
      >
        {image && (item.imagePath || item.thumbPath) ? (
          // 原图优先(缩略图仅 200px,预览区要清晰);原图缺失时退缩略图
          <img
            src={convertFileSrc(item.imagePath ?? item.thumbPath!)}
            alt={item.title}
            className="max-h-full max-w-full rounded-card object-contain"
            draggable={false}
          />
        ) : isFileItem(item) ? (
          <div className="flex flex-col items-center gap-2.5 py-4 text-center">
            <TypeIcon className="h-10 w-10" style={{ color: meta.color }} strokeWidth={1.5} aria-hidden />
            <span className="text-xs text-foreground" title={item.title}>
              {item.title}
            </span>
          </div>
        ) : (
          <p className="w-full whitespace-pre-wrap break-all text-xs leading-relaxed text-foreground/90">
            {fullText || '<空>'}
          </p>
        )}
      </div>

      {/* 元信息区(固定,不随内容滚动) */}
      <div className="flex shrink-0 select-none flex-col gap-1 border-t border-divider px-3 py-2.5">
        <MetaRow label="类型">
          <span className="inline-flex items-center gap-1">
            <TypeIcon className="h-3 w-3" style={{ color: meta.color }} aria-hidden />
            {meta.label}
          </span>
        </MetaRow>
        <MetaRow label="首次复制">{formatDateTime(item.firstCopiedAt)}</MetaRow>
        <MetaRow label="最近复制">{formatDateTime(item.lastCopiedAt)}</MetaRow>
        <MetaRow label="复制次数">{item.copyCount} 次</MetaRow>
        {item.imagePath && <MetaRow label="图片路径">{item.imagePath}</MetaRow>}
        {item.fileUrl && <MetaRow label="文件路径">{item.fileUrl}</MetaRow>}
        <MetaRow label="置顶">
          {item.pinned ? (
            <span className="inline-flex items-center gap-1">
              <Pin className="h-3 w-3 shrink-0" style={{ color: 'var(--badge-gold)', fill: 'var(--badge-gold)' }} aria-hidden />
              {formatDateTime(item.pinnedAt ?? item.firstCopiedAt)}
            </span>
          ) : (
            '未置顶'
          )}
        </MetaRow>
      </div>
    </div>
  )
}
