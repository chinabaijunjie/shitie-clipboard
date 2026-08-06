/**
 * 业务数据模型类型(前端侧)
 *
 * 与 Rust 侧 serde 序列化输出对齐(m1-plan.md 第三节)。
 * Rust: src-tauri/src/models.rs ClipboardItem。
 */
export interface ClipboardItem {
  id: number
  /** 首次复制时间(unix epoch ms) */
  firstCopiedAt: number
  /** 最近复制时间(unix epoch ms) */
  lastCopiedAt: number
  /** 复制次数(去重时累加) */
  copyCount: number
  /** 列表预览文本 */
  title: string
  /** 内容类型(M1 固定 text,M2 扩展为联合类型) */
  contentKind: 'text'
}
