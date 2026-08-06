/**
 * Tauri invoke 通信层封装
 *
 * sidecar 时代用 axios 调 localhost:9880,现在改用 Tauri invoke 直接调 Rust command。
 * 无 HTTP、无端口、无网络开销——前端与 Rust 通过 IPC 直接通信。
 *
 * 设计:
 * - call<T>(cmd, args?): 调 Rust command,返回 Promise<T>
 * - Rust 侧 `Result<T, AppError>` 的 Err 会被 Tauri reject 成 {code, message}
 * - 本封装把 reject 包成 InvokeError 异常,前端用 try/catch 捕获
 *
 * ⚠️ 错误码两端镜像:本文件 InvokeErrorCode ↔ Rust `error.rs` codes 模块。
 *    新增/删除错误码必须两端同步(constraints.md 错误码镜像契约)。
 *
 * 用法:
 *   import { call } from '@core/api/invoke'
 *   const pong = await call<string>('ping')
 *   try {
 *     const data = await call<ClipboardItem[]>('list_items')
 *   } catch (e) {
 *     if (e instanceof InvokeError && e.code === InvokeErrorCode.NOT_FOUND) { ... }
 *   }
 */
import { invoke } from '@tauri-apps/api/core'

// ============================================================
// 错误码(与 Rust src-tauri/src/error.rs codes 模块镜像)
// ============================================================

export const InvokeErrorCode = {
  /** 通用未知错误 */
  UNKNOWN: 'UNKNOWN',
  /** 内部错误(服务端异常) */
  INTERNAL_ERROR: 'INTERNAL_ERROR',
  /** 资源不存在(M1:剪贴板条目 id 查不到) */
  NOT_FOUND: 'NOT_FOUND',
  /** 写剪贴板失败(M1:copy_back 写回失败) */
  CLIPBOARD_WRITE_FAILED: 'CLIPBOARD_WRITE_FAILED',
} as const

export type InvokeErrorCode = (typeof InvokeErrorCode)[keyof typeof InvokeErrorCode]

// ============================================================
// 错误类型
// ============================================================

/**
 * invoke 调用失败异常。
 *
 * Rust 侧返回 `Err(AppError)` 时,Tauri 把 AppError 序列化成 `{code, message}` reject。
 * 本封装 catch 后包成 InvokeError,保留 code 供前端分支处理。
 */
export class InvokeError extends Error {
  public readonly code: InvokeErrorCode

  constructor(code: string, message: string) {
    super(message)
    this.name = 'InvokeError'
    // Rust 传过来的 code 可能不在 InvokeErrorCode 枚举内(两端失同步时),归为 UNKNOWN
    this.code = (
      Object.values(InvokeErrorCode).includes(code as InvokeErrorCode)
        ? code
        : InvokeErrorCode.UNKNOWN
    ) as InvokeErrorCode
  }
}

// ============================================================
// 对外 API
// ============================================================

/**
 * 调用 Rust command(类型安全封装)。
 *
 * @param cmd Rust 侧 `#[tauri::command]` 函数名
 * @param args 参数对象(Rust 侧用 serde 反序列化)
 * @returns Rust command 的返回值
 * @throws {InvokeError} Rust 侧返回 Err 时
 */
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    // Tauri reject 的值是 AppError 序列化后的 {code, message}
    if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
      const { code, message } = e as { code: string; message: string }
      throw new InvokeError(code, message)
    }
    // 非预期格式(如 Rust panic / 序列化失败),归为 UNKNOWN
    throw new InvokeError(
      InvokeErrorCode.UNKNOWN,
      e instanceof Error ? e.message : String(e),
    )
  }
}
