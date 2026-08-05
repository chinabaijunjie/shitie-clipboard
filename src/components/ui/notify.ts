/**
 * Message — 全局消息提示（命令式 + Hook 双版本）
 *
 * 在 antd message 基础上提供两类 API：
 *
 * 1. `notify`（命令式，无 React context 依赖）：
 *    notify.success('保存成功')
 *    notify.error(err)  // 自动 unwrap Error.message
 *
 * 2. `useNotify()`（Hook，受 ConfigProvider 控制，推荐页面里使用）：
 *    const notify = useNotify()
 *    notify.success('保存成功')
 *
 * useNotify 必须在 antd <App> 包裹下使用（main.tsx 已包裹）。
 */
import { App, message as antMessage } from 'antd'
import { useMemo } from 'react'
import { unwrapErrorMessage } from '@core/utils'

/** message 接受的内容类型：字符串、Error 实例、unknown（自动 unwrap） */
type MessageContent = string | Error | unknown

export interface NotifyAPI {
  /** 成功提示 */
  success(content: string, duration?: number): void
  /** 错误提示，自动 unwrap Error.message */
  error(content: MessageContent, duration?: number): void
  /** 警告提示 */
  warning(content: string, duration?: number): void
  /** 信息提示 */
  info(content: string, duration?: number): void
  /** loading 提示，返回 dismiss 函数 */
  loading(content: string): () => void
}

// ============================================================
// 命令式版本（无 React context 依赖，简单调用即可）
// ============================================================
export const notify: NotifyAPI = {
  success: (content, duration) => antMessage.success(content, duration),
  error: (content, duration) => antMessage.error(unwrapErrorMessage(content), duration),
  warning: (content, duration) => antMessage.warning(content, duration),
  info: (content, duration) => antMessage.info(content, duration),
  loading: (content) => {
    const hide = antMessage.loading(content, 0)
    return () => {
      hide()
    }
  },
}

// ============================================================
// Hook 版本（受 ConfigProvider 控制，推荐页面里使用）
// ============================================================
export function useNotify(): NotifyAPI {
  const { message } = App.useApp()

  return useMemo<NotifyAPI>(
    () => ({
      success: (content, duration) => message.success(content, duration),
      error: (content, duration) => message.error(unwrapErrorMessage(content), duration),
      warning: (content, duration) => message.warning(content, duration),
      info: (content, duration) => message.info(content, duration),
      loading: (content) => {
        const hide = message.loading(content, 0)
        return () => {
          hide()
        }
      },
    }),
    [message],
  )
}
