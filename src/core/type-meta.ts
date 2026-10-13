/**
 * 类型展示元数据 — 列表图标与预览面板类型标签共用
 *
 * 预览面板规划 §3.2:从 ClipboardList 抽取,消除列表/预览的重复判定。
 * 判定顺序与原 getTypeIcon 一致:fileUrl → subtype → contentKind。
 * 类型中文名与设置页统计口径一致(text + rich 都算「文字」)。
 */
import { FileText, Type, Layers, Link2, FileCode, Image as ImageIcon } from 'lucide-react'

import type { ClipboardItem } from '@core/types'

export interface TypeMeta {
  readonly icon: typeof FileText
  readonly color: string
  /** 类型中文名(文字/图片/文件/混合/链接/Markdown) */
  readonly label: string
}

/** 图片条目:有缩略图且非文件引用 → 列表用缩略图替代图标,预览区渲染原图 */
export function isImageItem(item: ClipboardItem): boolean {
  return !!item.thumbPath && !item.fileUrl
}

/** 文件条目:有 fileUrl 引用 */
export function isFileItem(item: ClipboardItem): boolean {
  return !!item.fileUrl
}

/** 取条目的类型展示元数据(图标 + 颜色 + 中文名)。纯函数,可单测。 */
export function getTypeMeta(item: ClipboardItem): TypeMeta {
  if (isFileItem(item)) {
    return { icon: FileText, color: 'var(--type-file)', label: '文件' }
  }
  if (item.contentSubtype === 'link') {
    return { icon: Link2, color: 'var(--accent)', label: '链接' }
  }
  if (item.contentSubtype === 'markdown') {
    return { icon: FileCode, color: 'var(--type-md)', label: 'Markdown' }
  }
  switch (item.contentKind) {
    case 'image':
      return { icon: ImageIcon, color: 'var(--text-secondary)', label: '图片' }
    case 'mixed':
      return { icon: Layers, color: 'var(--text-secondary)', label: '混合' }
    default:
      // text + rich 都归「文字」(与统计卡片口径一致)
      return { icon: Type, color: 'var(--text-secondary)', label: '文字' }
  }
}
