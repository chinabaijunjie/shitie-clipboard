/**
 * 业务数据模型类型(前端侧)
 *
 * 与 Rust 侧 serde 序列化输出对齐(m1-plan.md 第三节)。
 * Rust: src-tauri/src/models.rs ClipboardItem。
 *
 * 字段全 readonly:编译期阻止 mutate(§8 不可变性原则)。
 * 后端响应经 Zod schema 校验后才赋值(§8 边界校验,见 ClipboardItemSchema)。
 */
import { z } from 'zod'

/** 内容类型(与后端 ContentKind enum 对齐,serde rename_all=lowercase) */
export type ContentKind = 'text' | 'rich' | 'image' | 'file' | 'mixed'

/** ClipboardItem 的 Zod schema(校验后端 invoke 返回,§8 边界校验) */
export const ClipboardItemSchema = z.object({
  id: z.number(),
  firstCopiedAt: z.number(),
  lastCopiedAt: z.number(),
  copyCount: z.number(),
  title: z.string(),
  contentKind: z.enum(['text', 'rich', 'image', 'file', 'mixed']),
})

/** 列表的 Zod schema */
export const ClipboardItemListSchema = z.array(ClipboardItemSchema)

export interface ClipboardItem {
  readonly id: number
  /** 首次复制时间(unix epoch ms) */
  readonly firstCopiedAt: number
  /** 最近复制时间(unix epoch ms) */
  readonly lastCopiedAt: number
  /** 复制次数(去重时累加) */
  readonly copyCount: number
  /** 列表预览文本 */
  readonly title: string
  /** 内容类型 */
  readonly contentKind: ContentKind
}
