/**
 * SplashScreen — 应用启动页（sidecar 启动等待期间展示）
 *
 * 通用版：中性配色 + 简洁进度条 + 状态文案。
 * 业务工程起步时，可替换为品牌 Logo / 品牌配色（见 design-tokens.css）。
 *
 * 两态：
 * - status='loading'：不确定进度条 + 文案
 * - status='timeout'：超时提示 + 重试按钮
 *
 * 使用：由 SetupGate 在 checking-sidecar / sidecar-timeout 态渲染。
 * data-tauri-drag-region 让无边框窗口可拖拽。
 */
import { Button } from 'antd'
import { memo, type CSSProperties } from 'react'

export interface SplashScreenProps {
  /** 启动状态 */
  status: 'loading' | 'timeout'
  /** loading 态的状态文案（如"正在启动后端服务…"） */
  text?: string
  /** timeout 态的重试回调 */
  onRetry?: () => void
}

/** 全屏容器：居中布局 + 可拖拽 */
const containerStyle: CSSProperties = {
  position: 'fixed',
  inset: 0,
  zIndex: 1300,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 24,
  background: 'var(--color-bg-base)',
}

const appNameStyle: CSSProperties = {
  fontSize: 22,
  fontWeight: 600,
  letterSpacing: '0.04em',
  color: 'var(--color-text-primary)',
}

const accentStyle: CSSProperties = {
  color: 'var(--color-primary)',
}

/** 不确定进度条容器 */
const progressWrapStyle: CSSProperties = {
  width: 220,
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
  alignItems: 'center',
}

const progressTrackStyle: CSSProperties = {
  position: 'relative',
  width: '100%',
  height: 3,
  background: 'rgba(148, 163, 184, 0.2)',
  borderRadius: 2,
  overflow: 'hidden',
}

const progressFillStyle: CSSProperties = {
  position: 'absolute',
  top: 0,
  left: 0,
  height: '100%',
  width: '40%',
  background: 'linear-gradient(90deg, var(--color-primary), var(--color-primary-hover))',
  borderRadius: 2,
  animation: 'splash-indeterminate 1.8s ease-in-out infinite',
}

const statusStyle: CSSProperties = {
  fontSize: 14,
  color: 'var(--color-text-secondary)',
  letterSpacing: '0.02em',
}

export function SplashScreen({ status, text, onRetry }: SplashScreenProps) {
  const isLoading = status === 'loading'

  return (
    <div style={containerStyle} data-tauri-drag-region>
      <SplashKeyframes />

      {/* 应用名（业务工程替换为品牌名）*/}
      <div style={appNameStyle}>
        Tauri <span style={accentStyle}>Sidecar</span> Template
      </div>

      {isLoading ? (
        <div style={progressWrapStyle}>
          <div style={progressTrackStyle}>
            <div style={progressFillStyle} />
          </div>
          <div style={statusStyle}>{text ?? '正在启动后端服务…'}</div>
        </div>
      ) : (
        <>
          <div style={statusStyle}>后端服务启动超时</div>
          {onRetry && (
            <Button type="primary" onClick={onRetry}>
              重新检测
            </Button>
          )}
        </>
      )}
    </div>
  )
}

/**
 * 关键帧动画注入。
 * inline style 无法定义 @keyframes，React 里标准做法是用 <style> 标签。
 * 用 React.memo + 稳定字符串保证只注入一次。动画名加 splash- 前缀避免冲突。
 */
const KEYFRAMES = `
@keyframes splash-indeterminate {
  0%   { left: -40%; }
  100% { left: 100%; }
}
`

const SplashKeyframes = memo(function SplashKeyframes() {
  return <style>{KEYFRAMES}</style>
})
