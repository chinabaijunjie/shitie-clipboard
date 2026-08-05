/**
 * 通用工具函数
 *
 * 业务工程起步时，在此追加业务工具（如格式化 / 校验）。
 */

/** 从 unknown 解包出错误消息（Error 用 .message，其他转字符串）。 */
export function unwrapErrorMessage(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'string') return err
  try {
    return JSON.stringify(err)
  } catch {
    return String(err)
  }
}

/** 生成 UUID v4（无需依赖 uuid 库，crypto.randomUUID 浏览器原生支持）。 */
export function uuid(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID()
  }
  // 回退：手动拼（极老环境，正常 Tauri webview 不会走到）
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0
    const v = c === 'x' ? r : (r & 0x3) | 0x8
    return v.toString(16)
  })
}

/** 防抖：延迟 ms 内重复调用只执行最后一次。 */
export function debounce<T extends (...args: never[]) => void>(fn: T, ms: number): T {
  let timer: ReturnType<typeof setTimeout> | null = null
  return ((...args: never[]) => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => fn(...args), ms)
  }) as T
}

/** 截流：ms 内最多执行一次。 */
export function throttle<T extends (...args: never[]) => void>(fn: T, ms: number): T {
  let last = 0
  return ((...args: never[]) => {
    const now = Date.now()
    if (now - last >= ms) {
      last = now
      fn(...args)
    }
  }) as T
}
