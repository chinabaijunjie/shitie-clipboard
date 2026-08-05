/**
 * HTTP 客户端 — 基于 axios 的统一封装
 *
 * - base URL 指向 Python sidecar（http://localhost:9880）
 * - 统一拆包 ApiResponse<T>，失败抛 ApiClientError
 * - 超时 / sidecar 未就绪区分处理
 *
 * 使用方式：
 *   import { apiClient } from '@core/api/client'
 *   const item = await apiClient.get<Item>('/api/items/123')
 */

import axios, {
  AxiosError,
  AxiosInstance,
  AxiosRequestConfig,
  AxiosResponse,
} from 'axios'

import { SIDECAR_BASE_URL } from '@core/constants'
import { ApiError, ApiErrorCode, ApiResponse } from '@core/api/types'

// ============================================================
// 自定义错误
// ============================================================

export class ApiClientError extends Error {
  public readonly code: ApiErrorCode
  public readonly status: number
  public readonly detail?: string
  public readonly traceId?: string

  constructor(error: ApiError, status: number, traceId?: string) {
    super(error.message)
    this.name = 'ApiClientError'
    this.code = error.code
    this.status = status
    this.detail = error.detail
    this.traceId = traceId
  }
}

// ============================================================
// axios 实例
// ============================================================

const instance: AxiosInstance = axios.create({
  baseURL: SIDECAR_BASE_URL,
  timeout: 30_000,
  headers: {
    'Content-Type': 'application/json',
  },
})

// 响应拦截：拆 ApiResponse<T>，业务失败转 ApiClientError
instance.interceptors.response.use(
  (response: AxiosResponse<ApiResponse<unknown>>) => {
    const body = response.data
    // 部分流式接口（SSE）不走 ApiResponse 包装，直接放行
    if (!body || typeof body !== 'object' || !('success' in body)) {
      return response
    }
    if (!body.success) {
      const err = body.error ?? {
        code: ApiErrorCode.UNKNOWN,
        message: '未知错误',
      }
      throw new ApiClientError(err, response.status, body.trace_id)
    }
    return response
  },
  (error: AxiosError<ApiResponse<never>>) => {
    // 超时单独处理（不归为「sidecar 未启动」，二者给用户的提示不同）
    const isTimeout =
      error.code === 'ECONNABORTED' ||
      (typeof error.message === 'string' && error.message.includes('timeout'))
    if (isTimeout) {
      throw new ApiClientError(
        {
          code: ApiErrorCode.REQUEST_TIMEOUT,
          message: '请求超时，请稍后重试',
          detail: error.message,
        },
        0,
      )
    }
    // 网络错误 / sidecar 真未启动（无 HTTP 响应且非超时）
    if (!error.response) {
      throw new ApiClientError(
        {
          code: ApiErrorCode.SIDECAR_NOT_READY,
          message: '后端服务未响应，请确认 sidecar 已启动',
          detail: error.message,
        },
        0,
      )
    }
    const body = error.response.data
    const apiError: ApiError = body?.error ?? {
      code: ApiErrorCode.UNKNOWN,
      message: `HTTP ${error.response.status}`,
      detail: error.message,
    }
    throw new ApiClientError(apiError, error.response.status, body?.trace_id)
  },
)

// ============================================================
// 对外 API：get/post/put/patch/delete，返回 T（已拆包）
// ============================================================

async function unwrap<T>(promise: Promise<AxiosResponse<ApiResponse<T>>>): Promise<T> {
  const response = await promise
  return response.data.data as T
}

export const apiClient = {
  get<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return unwrap<T>(instance.get<ApiResponse<T>>(url, config))
  },
  post<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
    return unwrap<T>(instance.post<ApiResponse<T>>(url, data, config))
  },
  put<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
    return unwrap<T>(instance.put<ApiResponse<T>>(url, data, config))
  },
  patch<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
    return unwrap<T>(instance.patch<ApiResponse<T>>(url, data, config))
  },
  delete<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return unwrap<T>(instance.delete<ApiResponse<T>>(url, config))
  },
  /** 暴露原始 axios 实例，供 SSE / 文件上传等特殊场景使用 */
  raw(): AxiosInstance {
    return instance
  },
}
