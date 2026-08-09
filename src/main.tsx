import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import './index.css'

const rootElement = document.getElementById('root')
if (!rootElement) {
  throw new Error('找不到 #root 挂载点')
}

/**
 * 初始化主题 —— 苹果审美改造(深色 ①)。
 *
 * 当前固定深色主题(.theme-dark + .dark 双 class):
 * - .theme-dark:应用 UI 专用 token(毛玻璃/缓动/阴影等,见 index.css)
 * - .dark:Tailwind darkMode 触发器(让 dark: 前缀的类生效)
 *
 * 主题切换架构已预留(design-spec §4),未来加「外观」设置项时,
 * 这里改成读 settings.theme 动态切换 .theme-{dark/light/mid}。
 * 避免 FOUC:在 createRoot 之前同步设置 class。
 */
const rootHtml = document.documentElement
rootHtml.classList.add('theme-dark', 'dark')

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
