/**
 * App — 应用根组件
 *
 * Spotlight 风格应用:无 SetupGate 启动守卫(Rust 即时启动,不需要等 sidecar 解压)。
 * 后端就绪探活由 SpotlightPanel 内部 invoke ping 完成。
 *
 * M3:加视图切换(main ↔ settings)。单窗口不需要路由,用 state 控制。
 */
import { useState } from 'react'

import { SpotlightPanel } from '@components/SpotlightPanel'
import { SettingsPanel } from '@components/SettingsPanel'
import { useSettings } from '@hooks/useSettings'

type View = 'main' | 'settings'

function App() {
  const [view, setView] = useState<View>('main')
  const { settings, update } = useSettings()

  if (view === 'settings' && settings) {
    return (
      <SettingsPanel
        settings={settings}
        onUpdate={update}
        onBack={() => setView('main')}
      />
    )
  }

  return <SpotlightPanel onOpenSettings={() => setView('settings')} />
}

export default App
