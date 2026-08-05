/**
 * SetupGate — 应用启动守卫
 *
 * 职责：等 Python sidecar 就绪后再渲染主应用。
 *
 * 背景：Rust setup 异步 spawn Python sidecar（PyInstaller onefile 首次解压需数秒到
 * 数十秒）。本组件轮询 /api/health 直到就绪或超时，期间显示 SplashScreen loading。
 *
 * - sidecar 就绪 → 渲染子组件（主应用）
 * - sidecar 超时 → 显示错误页（带重试），不降级放行进空壳
 *
 * 业务工程起步时，若需额外启动前置检查（如模型下载 / 登录态），在此追加阶段。
 */
import { useEffect, useState } from 'react'
import { useSidecarReady } from '@core/api'
import { SplashScreen } from './SplashScreen'

type GateState = 'checking-sidecar' | 'ready' | 'sidecar-timeout'

interface SetupGateProps {
  /** sidecar 就绪时渲染的子组件（通常是主应用 Layout） */
  children: React.ReactNode
}

export function SetupGate({ children }: SetupGateProps) {
  const { status: sidecarStatus, retry: retrySidecar } = useSidecarReady()
  const [state, setState] = useState<GateState>('checking-sidecar')

  // sidecar 就绪 → ready；超时 → sidecar-timeout
  useEffect(() => {
    if (sidecarStatus === 'ready' && state === 'checking-sidecar') {
      setState('ready')
    }
    if (sidecarStatus === 'timeout' && state === 'checking-sidecar') {
      setState('sidecar-timeout')
    }
  }, [sidecarStatus, state])

  if (state === 'checking-sidecar') {
    return <SplashScreen status="loading" text="正在启动后端服务…" />
  }

  if (state === 'sidecar-timeout') {
    return (
      <SplashScreen
        status="timeout"
        onRetry={() => {
          retrySidecar()
        }}
      />
    )
  }

  return <>{children}</>
}
