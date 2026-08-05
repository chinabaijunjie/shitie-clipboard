import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import { STORAGE_KEYS } from '@core/constants'
import { storage } from '@core/storage'
import './index.css'

const rootElement = document.getElementById('root')
if (!rootElement) {
  throw new Error('找不到 #root 挂载点')
}

// 初始化主题:从 localStorage 读用户选的主题,通过 .dark class 控制 Tailwind 主题。
// 避免 FOUC(首帧闪烁):在 createRoot 之前同步设置 class。
type ThemeId = 'light' | 'dark'
const savedTheme = storage.get<ThemeId>(STORAGE_KEYS.THEME, 'light')
const isDark = savedTheme === 'dark'
document.documentElement.classList.toggle('dark', isDark)

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
