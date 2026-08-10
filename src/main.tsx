import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import { applyTheme, getInitialTheme } from '@core/theme'
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

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
