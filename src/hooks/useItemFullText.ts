/**
 * useItemFullText — 预览面板全文获取 hook
 *
 * 职责(单一):给定选中条目,返回其「应展示的文本」。
 * - 短路:title < TITLE_MAX_CHARS 时 title 即全文(绝大多数条目零 invoke)
 * - 超长:debounce 后 invoke get_item_text 取全文(键盘连续导航不狂发请求)
 * - 竞态:按 id 匹配,切换条目后旧响应作废(不会把 A 的全文显示在 B 上)
 *
 * 不关心展示(ItemPreviewPanel 的事),不关心选中怎么来(SpotlightPanel 的事)。
 */
import { useEffect, useState } from 'react'

import { call } from '@core/api'
import { TITLE_MAX_CHARS } from '@core/constants'
import type { ClipboardItem } from '@core/types'
import { isFileItem, isImageItem } from '@core/type-meta'

/** 超长文本拉取 debounce(ms):键盘 ↑↓ 连续导航时合并请求 */
const FETCH_DEBOUNCE_MS = 200

interface FetchedText {
  readonly id: number
  readonly text: string
}

/** 条目是否需要拉全文(纯函数,可单测)。
 *  图片/文件条目的 title 恒短(尺寸/文件名),永远不需要;文本类达到截断阈值才需要。 */
export function needsFullText(item: ClipboardItem | null): boolean {
  if (!item) return false
  if (isImageItem(item) || isFileItem(item)) return false
  return item.title.length >= TITLE_MAX_CHARS
}

/** 返回选中条目应展示的文本(无选中 = '')。 */
export function useItemFullText(item: ClipboardItem | null): string {
  const [fetched, setFetched] = useState<FetchedText | null>(null)

  useEffect(() => {
    if (!needsFullText(item) || !item) return
    const id = item.id
    const handle = window.setTimeout(() => {
      call<string>('get_item_text', { id })
        .then((text) => setFetched({ id, text }))
        .catch((e) => {
          // 失败兜底:继续显示截断 title(UI 不中断),留 console 供排查
          console.error('[get_item_text] 失败:', e)
        })
    }, FETCH_DEBOUNCE_MS)
    return () => window.clearTimeout(handle)
  }, [item])

  if (!item) return ''
  // 只认当前条目的响应(切换后旧响应自然作废)
  return fetched?.id === item.id ? fetched.text : item.title
}
