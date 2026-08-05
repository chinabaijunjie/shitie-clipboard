/**
 * core/api 模块出口
 *
 * 外部统一从 '@core/api' 导入，不直接引用 client.ts / types.ts 内部文件。
 */
export { apiClient, ApiClientError } from './client'
export type {
  ApiResponse,
  ApiError,
  ApiErrorCode as ApiErrorCodeType,
  ProgressEvent,
} from './types'
export { ApiErrorCode } from './types'
export { useSidecarReady } from './useSidecarReady'
export type { SidecarReadyStatus } from './useSidecarReady'
