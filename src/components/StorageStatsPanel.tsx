/**
 * StorageStatsPanel — 存储统计 + 清除历史(设置面板底部模块)
 *
 * 展示按内容类型分类的卡片统计 + 总条数 + 磁盘占用,
 * 提供「清除所有历史」按钮(二次确认)。
 *
 * 纯展示 + 事件转发:数据来自 useStorageStats,清除后自动刷新。
 */
import { useState } from 'react'
import { FileText, Image as ImageIcon, Link2, FileCode, Layers, Type, Trash2, type LucideIcon } from 'lucide-react'

import { cn } from '@lib/utils'
import type { StorageByKind } from '@core/types/models'

interface StorageStatsPanelProps {
  total: number
  byKind: StorageByKind
  storageBytes: number
  loading: boolean
  onClear: () => Promise<void>
}

/** 单个统计卡片 */
function StatCard({
  icon: Icon,
  label,
  count,
  reserved = false,
}: {
  icon: LucideIcon
  label: string
  count: number
  /** 预留位:半透明表示待实现 */
  reserved?: boolean
}) {
  return (
    <div
      className={cn(
        'flex flex-col items-center gap-0.5 rounded-card-small border border-divider px-1 py-1.5',
        reserved && 'opacity-40',
      )}
    >
      <Icon className="h-3.5 w-3.5 text-text-secondary" aria-hidden />
      <span className="text-sm font-semibold text-foreground">{count}</span>
      <span className="text-[9px] text-text-tertiary">{label}</span>
    </div>
  )
}

/** 格式化字节数为可读字符串 */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export function StorageStatsPanel({
  total,
  byKind,
  storageBytes,
  loading,
  onClear,
}: StorageStatsPanelProps) {
  const [confirming, setConfirming] = useState(false)
  const [clearing, setClearing] = useState(false)

  const handleClear = async () => {
    setClearing(true)
    try {
      await onClear()
    } finally {
      setClearing(false)
      setConfirming(false)
    }
  }

  return (
    <div className="px-3.5 pb-2">
      {/* 分组标题 + 右侧总数 */}
      <div className="flex items-center justify-between pt-3 pb-1.5">
        <span className="pl-0.5 text-[10px] font-semibold uppercase tracking-[0.05em] text-text-tertiary">
          已存储数据
        </span>
        {!loading && (
          <span className="text-[10px] text-text-tertiary">
            共 {total} 条 · {formatBytes(storageBytes)}
          </span>
        )}
      </div>

      {/* 分类卡片网格 */}
      {loading ? (
        <div className="py-4 text-center text-[10px] text-text-tertiary">统计中…</div>
      ) : (
        <div className="grid grid-cols-4 gap-1.5">
          <StatCard icon={Type} label="文字" count={byKind.text + byKind.rich} />
          <StatCard icon={ImageIcon} label="图片" count={byKind.image} />
          <StatCard icon={FileText} label="文件" count={byKind.file} />
          <StatCard icon={Layers} label="混合" count={byKind.mixed} />
          {/* 预留位:半透明 */}
          <StatCard icon={Link2} label="链接" count={byKind.link} reserved />
          <StatCard icon={FileCode} label="Markdown" count={byKind.markdown} reserved />
        </div>
      )}

      {/* 清除历史按钮 */}
      <div className="mt-3">
        {!confirming ? (
          <button
            onClick={() => setConfirming(true)}
            disabled={loading || total === 0}
            className="flex w-full items-center justify-center gap-1 rounded-card-small border border-divider py-1.5 text-[11px] text-destructive transition-colors duration-fast hover:bg-destructive/10 disabled:opacity-40"
          >
            <Trash2 className="h-3 w-3" aria-hidden />
            清除所有历史
          </button>
        ) : (
          <div className="flex gap-1.5">
            <button
              onClick={handleClear}
              disabled={clearing}
              className="flex-1 rounded-card-small bg-destructive py-1.5 text-[11px] text-destructive-foreground transition-opacity duration-fast disabled:opacity-50"
            >
              {clearing ? '清除中…' : '确定清除?不可撤销'}
            </button>
            <button
              onClick={() => setConfirming(false)}
              disabled={clearing}
              className="rounded-card-small border border-divider px-3 py-1.5 text-[11px] text-text-secondary transition-colors duration-fast hover:text-foreground disabled:opacity-50"
            >
              取消
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
