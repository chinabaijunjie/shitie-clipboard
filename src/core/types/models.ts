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
  // 内容子类型(link/markdown,仅 text/rich 可能有值)。可选。
  contentSubtype: z.enum(['link', 'markdown']).nullable().optional(),
  // 图片原图绝对路径(仅 image/mixed 有,供 UI 显示存储位置)。可选。
  imagePath: z.string().nullable().optional(),
  // 文件 URL(仅 file 有,原始文件路径引用,供 UI 显示)。可选。
  fileUrl: z.string().nullable().optional(),
  // 图片缩略图绝对路径(仅 image/mixed 有,前端 convertFileSrc 转 asset URL)。可选。
  thumbPath: z.string().nullable().optional(),
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
  /** 内容子类型(link/markdown,仅 text/rich 可能有值) */
  readonly contentSubtype?: 'link' | 'markdown' | null
  /** 图片原图绝对路径(仅 image/mixed 有,供 UI 显示存储位置) */
  readonly imagePath?: string | null
  /** 文件 URL(仅 file 有,原始文件路径引用) */
  readonly fileUrl?: string | null
  /** 图片缩略图绝对路径(仅 image/mixed 有,前端 convertFileSrc 转 asset URL) */
  readonly thumbPath?: string | null
}

// ============================================================
// 存储统计(设置面板「存储」模块)
// ============================================================

/** 按内容类型的分类计数 */
export interface StorageByKind {
  readonly text: number
  readonly rich: number
  readonly image: number
  readonly file: number
  readonly mixed: number
  /** 链接(text/rich 中识别为 link 的子类型) */
  readonly link: number
  /** Markdown(text/rich 中识别为 markdown 的子类型) */
  readonly markdown: number
}

/** 存储统计快照 */
export interface StorageStats {
  /** 历史记录总条数 */
  readonly total: number
  /** 按类型分组计数 */
  readonly byKind: StorageByKind
  /** 磁盘占用字节数 */
  readonly storageBytes: number
}

/** StorageStats 的 Zod schema(校验后端 invoke 返回) */
export const StorageStatsSchema = z.object({
  total: z.number().int().nonnegative(),
  byKind: z.object({
    text: z.number().int().nonnegative(),
    rich: z.number().int().nonnegative(),
    image: z.number().int().nonnegative(),
    file: z.number().int().nonnegative(),
    mixed: z.number().int().nonnegative(),
    link: z.number().int().nonnegative(),
    markdown: z.number().int().nonnegative(),
  }),
  storageBytes: z.number().int().nonnegative(),
})
