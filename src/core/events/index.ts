/**
 * 轻量事件总线 — 应用内 pub/sub
 *
 * 用于解耦模块之间的非直接调用通信(如:主题切换后通知 UI 刷新)。
 * 跨窗口/跨进程通信走 Tauri Events,不走这里。
 *
 * 使用方式:
 *   import { eventBus } from '@core/events'
 *   const off = eventBus.on('theme:changed', () => {})
 *   eventBus.emit('theme:changed')
 *   off() // 卸载时取消订阅
 */

type EventHandler<T = unknown> = (payload: T) => void

class EventBus {
  private readonly handlers = new Map<string, Set<EventHandler>>()

  on<T = unknown>(event: string, handler: EventHandler<T>): () => void {
    if (!this.handlers.has(event)) {
      this.handlers.set(event, new Set())
    }
    this.handlers.get(event)!.add(handler as EventHandler)
    return () => this.off(event, handler)
  }

  off<T = unknown>(event: string, handler: EventHandler<T>): void {
    this.handlers.get(event)?.delete(handler as EventHandler)
  }

  emit<T = unknown>(event: string, payload?: T): void {
    const set = this.handlers.get(event)
    if (!set || set.size === 0) return
    // 复制一份避免回调中增删订阅导致迭代异常
    for (const handler of [...set]) {
      try {
        handler(payload as T)
      } catch (error) {
        console.error(`[eventBus] 事件 ${event} 处理异常:`, error)
      }
    }
  }

  clear(event?: string): void {
    if (event) {
      this.handlers.delete(event)
    } else {
      this.handlers.clear()
    }
  }
}

export const eventBus = new EventBus()

// ============================================================
// 全局事件名常量(避免拼写错误)
// ============================================================
export const EVENTS = {
  THEME_CHANGED: 'theme:changed',
} as const

// ============================================================
// Tauri 事件名(后端 emit → 前端 listen,与 Rust 侧重名)
// 与 clipboard/handler.rs CLIPBOARD_ADDED_EVENT 对齐
// ============================================================
export const TAURI_EVENTS = {
  /** 新增或去重更新一条剪贴板记录(watcher 捕获后 emit) */
  CLIPBOARD_ADDED: 'clipboard://added',
} as const
