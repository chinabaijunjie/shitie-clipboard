import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import { applyTheme, getInitialTheme } from '@core/theme'
import { call } from '@core/api/invoke'
import './index.css'

const rootElement = document.getElementById('root')
if (!rootElement) {
  throw new Error('找不到 #root 挂载点')
}

/**
 * 初始化主题 —— 多主题切换。
 *
 * 启动时同步从 localStorage 读上次主题(避免 FOUC),settings 加载后如不一致再纠正。
 * - .theme-{dark/light/mid}:index.css 三套变量块(UI token)
 * - .dark:Tailwind darkMode 触发器(让 dark: 前缀的类生效,始终保留)
 */
applyTheme(getInitialTheme())

/**
 * 窗口拖动标志 —— Windows 拖动陷阱修复。
 *
 * Windows 上 data-tauri-drag-region 触发 start_dragging 时会瞬间失焦(Focused(false)),
 * 导致失焦隐藏把窗口关掉。在 mousedown 时通知 Rust 置位 IS_DRAGGING 标志,
 * should_hide_on_blur 检查该标志跳过隐藏。拖动结束后窗口重新获得焦点自动清除。
 */
document.addEventListener('mousedown', (e) => {
  const target = e.target as HTMLElement | null
  if (target?.closest?.('[data-tauri-drag-region]')) {
    call('set_dragging').catch(() => { /* 非关键路径,静默 */ })
  }
})

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
