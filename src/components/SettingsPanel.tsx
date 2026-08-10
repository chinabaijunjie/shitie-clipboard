/**
 * SettingsPanel — 设置面板(M3 + 苹果审美改造)
 *
 * 8 项配置(分 3 组):
 * - 通用:外观(主题) / 开机自启 / 唤起快捷键 / 启动时打开窗口
 * - 存储:历史记录上限 / 保留天数 / 容量上限
 * - 行为:失焦自动隐藏 / 敏感内容过滤
 *
 * 苹果审美改造(深色 ①):
 * - 毛玻璃外壳(glass-window)+ 渐变蒙版顶/底栏
 * - 设置项分组(通用/存储/行为),组标题大写小字距
 * - Toggle/Select/SettingRow 控件苹果化
 * - 返回按钮用 lucide ArrowLeft 图标
 * - 隐形细滚动条(scrollbar-thin)
 *
 * 改完即时保存(调 update_settings)+ 错误提示。
 * 顶栏返回按钮切回主面板。
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { ArrowLeft, Check } from 'lucide-react'

import { cn } from '@lib/utils'
import { WINDOW_OPACITY_OPTIONS } from '@core/theme'
import { isValidShortcut, parseKeyEvent, formatShortcut } from '@core/settings/shortcut'
import type { Settings } from '@core/settings/models'
import { ThemeSwitcher } from '@components/ThemeSwitcher'
import { StorageStatsPanel } from '@components/StorageStatsPanel'
import { useStorageStats } from '@hooks/useStorageStats'

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
  const { stats, loading: statsLoading, clearAll } = useStorageStats()

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
      } catch (e) {
        if (!cancelled) {
          console.error('[autostart] isEnabled 查询失败:', e)
          setError('开机自启状态读取失败,请重启应用')
          setAutostartLoading(false)
        }
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

  /** 保存设置(即时) + 2 秒「已保存」提示 */
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
      className="glass-window flex h-full w-full flex-col text-foreground"
    >
      {/* 顶栏:返回(左) + 标题居中 + 保存提示(右) —— 三等分 flex,标题天然居中
          - data-tauri-drag-region 支持拖动窗口 */}
      <div
        className="mask-fade-bottom flex items-center px-3.5 py-2.5 select-none"
        data-tauri-drag-region
      >
        {/* 左:返回 —— 固定宽度,和右侧对称 */}
        <div className="flex-1">
          <button
            onClick={onBack}
            className="flex items-center gap-0.5 rounded-card-small px-1.5 py-0.5 text-xs text-primary transition-colors duration-fast ease-out-strong hover:bg-hover"
          >
            <ArrowLeft className="h-3.5 w-3.5" aria-hidden />
            返回
          </button>
        </div>
        {/* 中:标题 —— flex-1 居中文字 */}
        <div className="flex-1 text-center">
          <span
            data-tauri-drag-region
            className="text-xs font-semibold tracking-tightish select-none"
          >
            设置
          </span>
        </div>
        {/* 右:保存提示 —— 固定宽度,和左侧对称 */}
        <div className="flex-1 text-right">
          {saved && (
            <span className="inline-flex items-center justify-end gap-1 text-[10px] text-primary">
              <Check className="h-3 w-3" aria-hidden />
              {saved}
            </span>
          )}
        </div>
      </div>

      {/* 设置列表(分组 + 隐形细滚动条) */}
      <div className="scrollbar-thin flex-1 overflow-y-auto px-3.5 py-1">
        {/* --- 通用 --- */}
        <SettingGroupTitle>通用</SettingGroupTitle>
        <SettingRow label="开机自启" desc="登录时自动启动(后台运行)">
          {autostartLoading ? (
            /* 查询系统 LaunchAgent 状态期间显示占位,避免初始 false → true 跳变 */
            <span className="text-[10px] text-text-tertiary">读取中…</span>
          ) : (
            <Toggle
              checked={autostartEnabled}
              onChange={() => void toggleAutostart()}
            />
          )}
        </SettingRow>
        <SettingRow label="唤起快捷键" desc="全局快捷键,随时唤起/隐藏窗口">
          {shortcutEditing ? (
            <div className="flex items-center gap-1">
              <div
                ref={shortcutRef}
                className={cn(
                  'rounded-card-small border border-primary px-2 py-0.5 text-[11px]',
                  pendingShortcut ? 'text-foreground' : 'text-text-tertiary',
                )}
              >
                {pendingShortcut || '按下快捷键…'}
              </div>
              <button
                onClick={confirmShortcut}
                disabled={!pendingShortcut}
                className="rounded-card-small bg-primary px-2 py-0.5 text-[10px] text-primary-foreground transition-opacity duration-fast disabled:opacity-50"
              >
                确定
              </button>
              <button
                onClick={() => {
                  setShortcutEditing(false)
                  setPendingShortcut('')
                }}
                className="rounded-card-small px-2 py-0.5 text-[10px] text-text-secondary transition-colors duration-fast hover:text-foreground"
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
              className="rounded-card-small border border-border px-2 py-0.5 text-[11px] text-foreground transition-colors duration-fast hover:border-primary"
            >
              {settings.shortcut}
            </button>
          )}
        </SettingRow>
        <SettingRow label="启动时打开窗口" desc="关闭后开机自启只进托盘">
          <Toggle
            checked={settings.launchVisible}
            onChange={(v) => void save({ launchVisible: v })}
          />
        </SettingRow>
        <SettingRow label="外观" desc="选择界面配色风格">
          <ThemeSwitcher
            value={settings.theme}
            onChange={(theme) => void save({ theme })}
          />
        </SettingRow>
        <SettingRow label="背景不透明度" desc="值越大越不透明(15% 较透 ~ 100% 不透明)">
          <Select
            value={settings.windowOpacity}
            options={WINDOW_OPACITY_OPTIONS.map((v) => ({ value: v, label: `${v}%` }))}
            onChange={(v) => void save({ windowOpacity: v })}
          />
        </SettingRow>

        {/* --- 行为 --- */}
        <SettingGroupTitle>行为</SettingGroupTitle>
        <SettingRow label="失焦自动隐藏" desc="点击窗口外自动隐藏(Spotlight 行为)">
          <Toggle
            checked={settings.hideOnBlur}
            onChange={(v) => void save({ hideOnBlur: v })}
          />
        </SettingRow>
        <SettingRow label="敏感内容过滤" desc="自动拦截 API key / Token / 私钥等敏感信息">
          <Toggle
            checked={settings.enableSensitiveFilter}
            onChange={(v) => void save({ enableSensitiveFilter: v })}
          />
        </SettingRow>

        {/* --- 存储 --- */}
        <SettingGroupTitle>存储</SettingGroupTitle>
        <SettingRow label="历史记录上限" desc="最多保留多少条历史">
          <Select
            value={settings.maxItems}
            options={MAX_ITEMS_OPTIONS.map((v) => ({ value: v, label: `${v} 条` }))}
            onChange={(v) => void save({ maxItems: v })}
          />
        </SettingRow>
        <SettingRow label="保留天数" desc="超过天数的记录自动清理">
          <Select
            value={settings.retentionDays}
            options={RETENTION_OPTIONS}
            onChange={(v) => void save({ retentionDays: v })}
          />
        </SettingRow>
        <SettingRow label="容量上限" desc="超过容量从最旧开始清理">
          <Select
            value={settings.maxStorageMb}
            options={STORAGE_OPTIONS}
            onChange={(v) => void save({ maxStorageMb: v })}
          />
        </SettingRow>

        {/* 存储统计 + 清除历史 */}
        <StorageStatsPanel
          total={stats?.total ?? 0}
          byKind={stats?.byKind ?? {
            text: 0, rich: 0, image: 0, file: 0, mixed: 0, link: 0, markdown: 0,
          }}
          storageBytes={stats?.storageBytes ?? 0}
          loading={statsLoading}
          onClear={clearAll}
        />
      </div>

      {/* 底栏:错误提示 / 保存提示 —— 顶部半透明分隔线 */}
      <div className="border-t border-divider px-3.5 py-1.5 text-[10px]">
        {error ? (
          <span className="text-destructive">{error}</span>
        ) : (
          <span className="text-primary">设置即时保存</span>
        )}
      </div>
    </div>
  )
}

// ============================================================
// 子组件(内联,不单独抽文件)
// ============================================================

/** 组标题:大写 + 小字距,苹果设置面板风格 */
function SettingGroupTitle({ children }: { children: React.ReactNode }) {
  return (
    <div className="pt-3 pb-1.5 pl-1 text-[10px] font-semibold uppercase tracking-[0.05em] text-text-tertiary">
      {children}
    </div>
  )
}

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
    <div className="flex items-center justify-between border-b border-divider py-2.5 last:border-b-0">
      <div className="mr-3">
        <div className="text-xs font-medium text-foreground">{label}</div>
        <div className="mt-0.5 text-[10px] text-text-tertiary">{desc}</div>
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
        'relative h-5 w-9 rounded-full transition-colors duration-base ease-out-strong',
        checked ? 'bg-primary' : 'bg-muted',
        disabled && 'opacity-50',
      )}
    >
      <span
        className="absolute left-0.5 top-0.5 h-4 w-4 rounded-full transition-transform duration-base ease-out-strong"
        style={{
          transform: checked ? 'translateX(16px)' : 'translateX(0)',
          background: 'var(--toggle-thumb)',
          boxShadow: 'var(--thumb-shadow)',
        }}
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
      className="rounded-card-small border border-border bg-transparent px-2 py-0.5 text-[11px] text-text-secondary transition-colors duration-fast focus:border-primary focus:outline-none"
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value} className="bg-background text-foreground">
          {opt.label}
        </option>
      ))}
    </select>
  )
}
