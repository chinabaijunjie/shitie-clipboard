/**
 * 业务数据模型类型（前端侧）
 *
 * 与后端 ORM 模型的 serialize() 输出对齐。
 * 业务工程起步时，在此追加业务实体类型（如 Image / Task）。
 */

/** Item — 示例实体（与 backend/core/models.py Item.serialize() 对齐） */
export interface Item {
  id: string
  name: string
  description: string | null
  created_at: string | null
  updated_at: string | null
}
