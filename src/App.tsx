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
import { cn } from '@lib/utils'

type View = 'main' | 'settings'

function App() {
  const [view, setView] = useState<View>('main')
  const { settings, loading, error, update } = useSettings()

  if (view === 'settings') {
    if (loading) {
      return (
        <div
          data-tauri-drag-region
          className={cn(
            'flex h-full w-full items-center justify-center rounded-lg border border-border bg-background text-foreground shadow-2xl',
          )}
        >
          <span className="text-xs text-muted-foreground">加载设置中…</span>
        </div>
      )
    }
    if (error || !settings) {
      return (
        <div
          data-tauri-drag-region
          className={cn(
            'flex h-full w-full flex-col items-center justify-center gap-3 rounded-lg border border-border bg-background text-foreground shadow-2xl',
          )}
        >
          <span className="text-xs text-destructive">
            设置加载失败:{error ?? '未知错误'}
          </span>
          <button
            onClick={() => setView('main')}
            className="rounded px-3 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            ← 返回
          </button>
        </div>
      )
    }
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
