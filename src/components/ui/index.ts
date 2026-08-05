/**
 * UI 模块出口
 *
 * 模板只内置 notify（消息提示）。
 * 业务工程起步时，在此追加业务通用组件（如 Button/Card/Loading 封装、EmptyState 等）。
 */
export { notify, useNotify } from './notify'
export type { NotifyAPI } from './notify'
