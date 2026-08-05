/**
 * core/api 模块出口 — Tauri invoke 通信层
 *
 * 外部统一从 '@core/api' 导入,不直接引用 invoke.ts 内部文件。
 *
 * sidecar 时代这里有 axios client + useSidecarReady,现在改为纯 invoke 封装。
 */
export { call, InvokeError, InvokeErrorCode } from './invoke'
export type { InvokeErrorCode as InvokeErrorCodeType } from './invoke'
