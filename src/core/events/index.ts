/**
 * 轻量事件总线 — 应用内 pub/sub
 *
 * 用于解耦模块之间的非直接调用通信（如：sidecar 启动后通知 UI、
 * 任务完成后刷新列表）。跨窗口/跨进程通信走 Tauri Events，不走这里。
 *
 * 使用方式：
 *   import { eventBus } from '@core/events'
 *   const off = eventBus.on('item:created', (payload) => {})
 *   eventBus.emit('item:created', { id: '1' })
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
// 全局事件名常量（避免拼写错误，业务工程起步时在此追加业务事件）
// ============================================================
export const EVENTS = {
  SIDECAR_STARTED: 'sidecar:started',
  SIDECAR_STOPPED: 'sidecar:stopped',
  SIDECAR_HEALTH_CHANGED: 'sidecar:health-changed',
  THEME_CHANGED: 'theme:changed',
} as const
