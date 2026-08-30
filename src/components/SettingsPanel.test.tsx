/**
 * SettingsPanel 组件测试 — 开机自启开关的 dev 门控(v2.0.2)
 *
 * vitest 环境 import.meta.env.DEV 恒为 true,天然覆盖 dev 分支:
 * - 开关禁用(disabled)+ 说明文案(防止登录项指向调试版)
 * - 不调用 isEnabled(dev 下后端未注册 autostart 插件,查询必然报错)
 *
 * release 分支(查询/切换 autostart)依赖真实插件调用,不在组件测试范围。
 * invoke 一律 mock 为拒绝(@core/api),存储统计卡片走错误兜底不阻断渲染。
 */
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { render, screen, within } from '@testing-library/react'

import { SettingsPanel } from '@components/SettingsPanel'
import type { Settings } from '@core/settings/models'

vi.mock('@tauri-apps/plugin-autostart', () => ({
  isEnabled: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
}))

vi.mock('@core/api', () => ({
  call: vi.fn().mockRejectedValue(new Error('invoke 不可用(测试环境)')),
}))

const baseSettings: Settings = {
  shortcut: 'Alt+KeyV',
  launchVisible: true,
  maxItems: 50,
  retentionDays: 0,
  maxStorageMb: 500,
  hideOnBlur: true,
  enableSensitiveFilter: true,
  theme: 'dark',
  windowOpacity: 80,
}

function renderPanel() {
  render(<SettingsPanel settings={baseSettings} onUpdate={vi.fn()} onBack={vi.fn()} />)
}

/** 定位「开机自启」行的开关(label 与 switch 同在 SettingRow 容器内)。 */
function autostartToggle(): HTMLElement {
  const label = screen.getByText('开机自启')
  const row = label.parentElement?.parentElement
  expect(row).toBeDefined()
  return within(row!).getByRole('switch')
}

describe('SettingsPanel dev 门控(开机自启)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('dev 构建下自启开关禁用 + 文案说明', () => {
    renderPanel()
    expect(autostartToggle()).toHaveAttribute('disabled')
    expect(
      screen.getByText('开发模式不可用(防止登录项指向调试版)'),
    ).toBeInTheDocument()
  })

  it('dev 构建下不查询 autostart 状态(后端未注册插件,查询会报错)', async () => {
    renderPanel()
    // 等 useEffect 执行完
    await new Promise((resolve) => setTimeout(resolve, 0))
    const { isEnabled } = await import('@tauri-apps/plugin-autostart')
    expect(isEnabled).not.toHaveBeenCalled()
  })
})
