import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App as AntApp, ConfigProvider, theme as antdTheme } from 'antd'

import App from './App'
import { STORAGE_KEYS } from '@core/constants'
import { storage } from '@core/storage'
import './styles/design-tokens.css'
import './styles/global.css'

const rootElement = document.getElementById('root')
if (!rootElement) {
  throw new Error('找不到 #root 挂载点')
}

// 初始化 data-theme：从 localStorage 读用户选的主题，避免首帧闪烁。
// 业务工程起步时可接入完整主题系统（多主题 map），这里保持最简（light/dark）。
type ThemeId = 'light' | 'dark'
const savedTheme = storage.get<ThemeId>(STORAGE_KEYS.THEME, 'light')
const validTheme: ThemeId = savedTheme === 'dark' ? 'dark' : 'light'
document.documentElement.setAttribute('data-theme', validTheme)

// Antd 主题与 data-theme 同步（亮 defaultAlgorithm / 暗 darkAlgorithm）
const antdAlgorithm = validTheme === 'dark' ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm

createRoot(rootElement).render(
  <StrictMode>
    <ConfigProvider
      theme={{
        algorithm: antdAlgorithm,
        token: {
          colorPrimary: '#3B82F6',
        },
      }}
    >
      {/* AntApp 包裹让 useNotify（App.useApp context）可用 */}
      <AntApp>
        <App />
      </AntApp>
    </ConfigProvider>
  </StrictMode>,
)
