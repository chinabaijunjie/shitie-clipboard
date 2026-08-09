/**
 * 主题管理 — 纯函数,无 React 依赖
 *
 * 职责(单一):管理 <html> 上的 .theme-* class + localStorage 缓存。
 * 不关心 UI 渲染(那是 ThemeSwitcher 的事),不关心持久化到 settings.json(那是 useSettings 的事)。
 *
 * 两层持久化:
 * 1. localStorage(同步,启动时读,避免 FOUC)
 * 2. settings.json(异步,Rust 侧,跨设备可信)
 * settings 加载后如发现两者不一致,以 settings.json 为准 → 纠正 localStorage + class。
 *
 * .dark class:Tailwind darkMode 触发器,始终与 .theme-* 并存(让 dark: 前缀的类生效)。
 */
import { STORAGE_KEYS } from '@core/constants'
import { THEME_NAMES, type ThemeName } from '@core/settings/models'

/** 判断值是否为合法主题名 */
export function isThemeName(value: unknown): value is ThemeName {
  return typeof value === 'string' && (THEME_NAMES as readonly string[]).includes(value)
}

/**
 * 应用主题到 DOM + 写入 localStorage 缓存。
 *
 * 设置 <html> 的 class 为 `theme-{theme} dark`:
 * - .theme-{dark/light/mid}:index.css 中的三套变量块
 * - .dark:Tailwind darkMode 触发器(始终保留)
 */
export function applyTheme(theme: ThemeName): void {
  const html = document.documentElement
  html.className = `theme-${theme} dark`
  try {
    localStorage.setItem(STORAGE_KEYS.THEME, theme)
  } catch {
    // localStorage 不可用(隐私模式/磁盘满)—— 主题仍会生效(class 已设置),只是无法跨重启记忆
  }
}

/**
 * 启动时获取初始主题(同步,避免 FOUC)。
 *
 * 读取顺序:
 * 1. localStorage 缓存(上次选择的主题)
 * 2. 无缓存或值非法 → 'dark'(最稳默认)
 *
 * 返回值在 main.tsx 的 createRoot 之前同步使用,确保首帧就带正确主题。
 */
export function getInitialTheme(): ThemeName {
  try {
    const cached = localStorage.getItem(STORAGE_KEYS.THEME)
    if (isThemeName(cached)) return cached
  } catch {
    // localStorage 不可用 —— 用默认值
  }
  return 'dark'
}
