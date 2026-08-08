/**
 * Settings 类型(前端侧)
 *
 * 与 Rust 侧 settings::Settings 对齐(serde rename_all=camelCase)。
 * Rust: src-tauri/src/settings/mod.rs Settings。
 *
 * 字段全 readonly:编译期阻止 mutate(§8 不可变性原则)。
 * 后端响应经 Zod schema 校验后才赋值(§8 边界校验)。
 *
 * ⚠️ autostart 状态由 tauri-plugin-autostart 管理(不走 get_settings/update_settings),
 *    SettingsPanel 读取时直接调 isEnabled()。
 */
import { z } from 'zod'

/** Settings 的 Zod schema(校验后端 invoke 返回,§8 边界校验) */
export const SettingsSchema = z.object({
  shortcut: z.string(),
  launchVisible: z.boolean(),
  maxItems: z.number(),
  retentionDays: z.number(),
  maxStorageMb: z.number(),
  hideOnBlur: z.boolean(),
})

export interface Settings {
  readonly shortcut: string
  /** 启动时是否打开窗口 */
  readonly launchVisible: boolean
  /** 历史记录条数上限 */
  readonly maxItems: number
  /** 保留天数(0 = 不限) */
  readonly retentionDays: number
  /** 容量上限 MB(0 = 不限) */
  readonly maxStorageMb: number
  /** 失焦自动隐藏 */
  readonly hideOnBlur: boolean
}
