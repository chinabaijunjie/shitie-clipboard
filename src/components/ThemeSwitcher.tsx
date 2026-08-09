/**
 * ThemeSwitcher — 分段按钮主题选择器(macOS 系统设置「外观」风格)
 *
 * 三选一(深色 / 浅色 / 折中),选中态蓝色高亮,即时生效。
 * 纯展示组件:不关心持久化(那是 SettingsPanel save 的事),
 * 只负责渲染当前值 + 触发 onChange。
 */
import { THEME_NAMES, type ThemeName } from '@core/settings/models'
import { cn } from '@lib/utils'

/** 主题选项标签(中文显示) */
const THEME_LABELS: Record<ThemeName, string> = {
  dark: '深色',
  light: '浅色',
  mid: '折中',
}

interface ThemeSwitcherProps {
  /** 当前选中的主题 */
  value: ThemeName
  /** 用户切换主题时触发 */
  onChange: (theme: ThemeName) => void
}

export function ThemeSwitcher({ value, onChange }: ThemeSwitcherProps) {
  return (
    <div
      role="radiogroup"
      aria-label="界面主题"
      className="flex items-center gap-0.5 rounded-card-small border border-divider p-0.5"
    >
      {THEME_NAMES.map((name) => {
        const isSelected = name === value
        return (
          <button
            key={name}
            role="radio"
            aria-checked={isSelected}
            onClick={() => onChange(name)}
            className={cn(
              'rounded-[5px] px-2.5 py-0.5 text-[11px] font-medium',
              'transition-colors duration-base ease-out-strong',
              isSelected
                ? 'bg-primary text-primary-foreground'
                : 'text-text-secondary hover:bg-hover hover:text-foreground',
            )}
          >
            {THEME_LABELS[name]}
          </button>
        )
      })}
    </div>
  )
}
