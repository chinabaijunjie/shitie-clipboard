/**
 * SettingsPanel — 设置面板(M3)
 *
 * 7 项配置:
 * - 开机自启(Toggle,直接调 autostart 插件)
 * - 唤起快捷键(快捷键录入器)
 * - 启动时打开窗口(Toggle)
 * - 历史记录上限(下拉)
 * - 保留天数(下拉)
 * - 容量上限(下拉)
 * - 失焦自动隐藏(Toggle)
 *
 * 改完即时保存(调 update_settings)+ 错误提示。
 * 顶栏返回按钮切回主面板。
 */
import { useCallback, useEffect, useRef, useState } from 'react'

import { cn } from '@lib/utils'
import { isValidShortcut, parseKeyEvent, formatShortcut } from '@core/settings/shortcut'
import type { Settings } from '@core/settings/models'

interface SettingsPanelProps {
  settings: Settings
  onUpdate: (newSettings: Settings) => Promise<void>
  onBack: () => void
}

/** 下拉选项常量 */
const MAX_ITEMS_OPTIONS = [10, 20, 50, 100, 200, 500]
const RETENTION_OPTIONS = [
  { value: 0, label: '永久保留' },
  { value: 7, label: '7 天' },
  { value: 30, label: '30 天' },
  { value: 90, label: '90 天' },
]
const STORAGE_OPTIONS = [
  { value: 0, label: '不限' },
  { value: 50, label: '50 MB' },
  { value: 100, label: '100 MB' },
  { value: 200, label: '200 MB' },
  { value: 500, label: '500 MB' },
]

export function SettingsPanel({ settings, onUpdate, onBack }: SettingsPanelProps) {
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [autostartLoading, setAutostartLoading] = useState(true)
  const [shortcutEditing, setShortcutEditing] = useState(false)
  const [pendingShortcut, setPendingShortcut] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState<string | null>(null)
  const shortcutRef = useRef<HTMLDivElement>(null)

  // 读取 autostart 状态(直接调插件,不走 settings.json)
  useEffect(() => {
    let cancelled = false
    const load = async () => {
      try {
        const { isEnabled } = await import('@tauri-apps/plugin-autostart')
        const enabled = await isEnabled()
        if (!cancelled) {
          setAutostartEnabled(enabled)
          setAutostartLoading(false)
        }
      } catch {
        if (!cancelled) setAutostartLoading(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [])

  // 快捷键录入器:监听 keydown
  useEffect(() => {
    if (!shortcutEditing) return
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      // 单独按 modifier 不触发录入
      const { modifiers, key } = parseKeyEvent(e)
      if (key === '' || ['Cmd', 'Control', 'Shift', 'Alt', 'Super'].includes(key)) return
      if (isValidShortcut(modifiers, key)) {
        setPendingShortcut(formatShortcut(modifiers, key))
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [shortcutEditing])

  /** 保存设置(即时) + 3 秒「已保存」提示 */
  const save = useCallback(
    async (patch: Partial<Settings>) => {
      setError(null)
      try {
        const newSettings = { ...settings, ...patch }
        await onUpdate(newSettings)
        setSaved('已保存')
        window.setTimeout(() => setSaved(null), 2000)
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [settings, onUpdate],
  )

  /** 切换开机自启 */
  const toggleAutostart = useCallback(async () => {
    setError(null)
    try {
      const { enable, disable } = await import('@tauri-apps/plugin-autostart')
      if (autostartEnabled) {
        await disable()
        setAutostartEnabled(false)
      } else {
        await enable()
        setAutostartEnabled(true)
      }
    } catch (e) {
      setError(`自启设置失败:${e instanceof Error ? e.message : String(e)}`)
    }
  }, [autostartEnabled])

  /** 确认快捷键修改 */
  const confirmShortcut = useCallback(async () => {
    if (!pendingShortcut) return
    setShortcutEditing(false)
    await save({ shortcut: pendingShortcut })
  }, [pendingShortcut, save])

  return (
    <div
      data-tauri-drag-region
      className={cn(
        'flex h-full w-full flex-col rounded-lg border border-border bg-background text-foreground shadow-2xl',
      )}
    >
      {/* 顶栏:返回 + 标题 */}
      <div className="flex items-center gap-2 border-b border-border/50 px-3 py-2" data-tauri-drag-region>
        <button
          onClick={onBack}
          className="rounded px-2 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          ← 返回
        </button>
        <span className="text-xs font-semibold tracking-wide">设置</span>
        {saved && <span className="ml-auto text-[10px] text-primary">{saved}</span>}
      </div>

      {/* 设置列表 */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        {/* 开机自启 */}
        <SettingRow label="开机自启" desc="登录时自动启动(后台运行)">
          <Toggle checked={autostartEnabled} disabled={autostartLoading} onChange={() => void toggleAutostart()} />
        </SettingRow>

        {/* 唤起快捷键 */}
        <SettingRow label="唤起快捷键" desc="全局快捷键,随时唤起/隐藏窗口">
          {shortcutEditing ? (
            <div className="flex items-center gap-1">
              <div
                ref={shortcutRef}
                className={cn(
                  'rounded border border-primary px-2 py-0.5 text-[11px]',
                  pendingShortcut ? 'text-foreground' : 'text-muted-foreground',
                )}
              >
                {pendingShortcut || '按下快捷键…'}
              </div>
              <button
                onClick={confirmShortcut}
                disabled={!pendingShortcut}
                className="rounded bg-primary px-2 py-0.5 text-[10px] text-primary-foreground disabled:opacity-50"
              >
                确定
              </button>
              <button
                onClick={() => {
                  setShortcutEditing(false)
                  setPendingShortcut('')
                }}
                className="rounded px-2 py-0.5 text-[10px] text-muted-foreground hover:text-foreground"
              >
                取消
              </button>
            </div>
          ) : (
            <button
              onClick={() => {
                setPendingShortcut('')
                setShortcutEditing(true)
              }}
              className="rounded border border-border px-2 py-0.5 text-[11px] text-foreground hover:border-primary"
            >
              {settings.shortcut}
            </button>
          )}
        </SettingRow>

        {/* 启动时打开窗口 */}
        <SettingRow label="启动时打开窗口" desc="关闭后开机自启只进托盘">
          <Toggle
            checked={settings.launchVisible}
            onChange={(v) => void save({ launchVisible: v })}
          />
        </SettingRow>

        {/* 历史记录上限 */}
        <SettingRow label="历史记录上限" desc="最多保留多少条历史">
          <Select
            value={settings.maxItems}
            options={MAX_ITEMS_OPTIONS.map((v) => ({ value: v, label: `${v} 条` }))}
            onChange={(v) => void save({ maxItems: v })}
          />
        </SettingRow>

        {/* 保留天数 */}
        <SettingRow label="保留天数" desc="超过天数的记录自动清理">
          <Select
            value={settings.retentionDays}
            options={RETENTION_OPTIONS}
            onChange={(v) => void save({ retentionDays: v })}
          />
        </SettingRow>

        {/* 容量上限 */}
        <SettingRow label="容量上限" desc="超过容量从最旧开始清理">
          <Select
            value={settings.maxStorageMb}
            options={STORAGE_OPTIONS}
            onChange={(v) => void save({ maxStorageMb: v })}
          />
        </SettingRow>

        {/* 失焦自动隐藏 */}
        <SettingRow label="失焦自动隐藏" desc="点击窗口外自动隐藏(Spotlight 行为)">
          <Toggle
            checked={settings.hideOnBlur}
            onChange={(v) => void save({ hideOnBlur: v })}
          />
        </SettingRow>

        {/* 敏感内容过滤 */}
        <SettingRow label="敏感内容过滤" desc="自动拦截 API key / Token / 私钥等敏感信息">
          <Toggle
            checked={settings.enableSensitiveFilter}
            onChange={(v) => void save({ enableSensitiveFilter: v })}
          />
        </SettingRow>
      </div>

      {/* 底栏:错误提示 */}
      <div className="border-t border-border/50 px-3 py-1.5 text-[10px]">
        {error ? (
          <span className="text-destructive">{error}</span>
        ) : (
          <span className="text-muted-foreground/70">设置即时保存</span>
        )}
      </div>
    </div>
  )
}

// ============================================================
// 子组件(内联,不单独抽文件)
// ============================================================

function SettingRow({
  label,
  desc,
  children,
}: {
  label: string
  desc: string
  children: React.ReactNode
}) {
  return (
    <div className="flex items-center justify-between border-b border-border/30 py-2.5">
      <div className="mr-3">
        <div className="text-xs font-medium text-foreground">{label}</div>
        <div className="text-[10px] text-muted-foreground/70">{desc}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  )
}

function Toggle({
  checked,
  disabled,
  onChange,
}: {
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        'relative h-5 w-9 rounded-full transition-colors',
        checked ? 'bg-primary' : 'bg-muted',
        disabled && 'opacity-50',
      )}
    >
      <span
        className={cn(
          'absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform',
          checked ? 'translate-x-4' : 'translate-x-0.5',
        )}
      />
    </button>
  )
}

function Select<T extends number>({
  value,
  options,
  onChange,
}: {
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(Number(e.target.value) as T)}
      className="rounded border border-border bg-transparent px-2 py-0.5 text-[11px] text-foreground focus:border-primary focus:outline-none"
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value} className="bg-background">
          {opt.label}
        </option>
      ))}
    </select>
  )
}
