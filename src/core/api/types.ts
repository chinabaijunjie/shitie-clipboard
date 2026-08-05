/**
 * 通用 API 响应类型与错误码定义
 *
 * 所有后端响应统一走 ApiResponse<T> 包装，前端 client 自动拆包。
 *
 * ⚠️ 前后端错误码镜像契约：本文件的 ApiErrorCode 与 backend/core/errors.py 的 ErrorCode
 *    是同一份契约的两端实现，任一端新增/删除错误码必须同步另一端。
 */

// ============================================================
// 统一响应包装
// ============================================================

export interface ApiResponse<T> {
  /** 业务是否成功（HTTP 200 且无业务错误） */
  success: boolean
  /** 业务数据，失败时为 null */
  data: T | null
  /** 错误信息，成功时为 null */
  error: ApiError | null
  /** 请求追踪 ID（后端生成，用于日志关联） */
  trace_id?: string
}

export interface ApiError {
  /** 错误码，见下方 ApiErrorCode 枚举 */
  code: ApiErrorCode
  /** 面向用户的友好提示 */
  message: string
  /** 面向开发者的详细信息（堆栈/原始异常等） */
  detail?: string
}

// ============================================================
// 错误码（与后端 backend/core/errors.py 保持一致 — 镜像契约）
// 业务工程起步时，在两端同步追加业务错误码（如 IMAGE_GEN_FAILED）
// ============================================================
export const ApiErrorCode = {
  // 通用
  UNKNOWN: 'UNKNOWN',
  BAD_REQUEST: 'BAD_REQUEST',
  UNAUTHORIZED: 'UNAUTHORIZED',
  FORBIDDEN: 'FORBIDDEN',
  NOT_FOUND: 'NOT_FOUND',
  CONFLICT: 'CONFLICT',
  INTERNAL_ERROR: 'INTERNAL_ERROR',
  SERVICE_UNAVAILABLE: 'SERVICE_UNAVAILABLE',

  // 基础设施
  SIDECAR_NOT_READY: 'SIDECAR_NOT_READY',
  REQUEST_TIMEOUT: 'REQUEST_TIMEOUT',
} as const

export type ApiErrorCode = (typeof ApiErrorCode)[keyof typeof ApiErrorCode]

// ============================================================
// 进度推送（SSE 事件载荷）
// ============================================================
export interface ProgressEvent<T = unknown> {
  /** 任务 ID */
  task_id: string
  /** 当前步骤 */
  current: number
  /** 总步骤 */
  total: number
  /** 进度消息 */
  message: string
  /** 自定义负载 */
  payload?: T
}
