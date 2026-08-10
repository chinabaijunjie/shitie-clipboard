/**
 * Tailwind 配置 — 苹果审美改造(深色毛玻璃 ①)
 *
 * 设计依据:wiki/dev/ui-design-spec.md §2 + §3.1
 *
 * 变更要点(相对 M0 默认):
 * - darkMode: 'class'(Tailwind 默认监听 .dark;main.tsx 同时加 .dark + .theme-dark;
 *   主题切换架构预留见 design-spec §4)
 * - shadcn 颜色键沿用 hsl(var(--x)),配色值在 index.css 改成苹果系统色
 * - 新增 UI 专用 token 接线:
 *   boxShadow.window / transitionTimingFunction / borderRadius 分档 / colors 扩展
 * - 注释修正:M0 注释称「不启用 preflight」但代码未实际禁用 —— 本项目实际启用
 *   preflight(Tailwind reset),组件依赖它,此处维持启用,注释纠正
 *
 * shadcn 颜色键:background/foreground/card/popover/primary/secondary/muted/
 * accent/destructive/border/input/ring。
 * 扩展键:window/card-small/thumb/caption(本组件专用)
 */
/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // --- shadcn 兼容键(hsl 三元组,组件层引用)---
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },

        // --- UI 专用键(直接引 CSS 变量,非 hsl 格式)---
        // 文字层级:苹果 systemLabel 三级(带透明度)
        'text-secondary': 'var(--text-secondary)',
        'text-tertiary': 'var(--text-tertiary)',
        // 列表交互态
        'bg-hover': 'var(--bg-hover)',
        'bg-selected': 'var(--bg-selected)',
        // 徽标 / 类型图标
        'badge-bg': 'var(--badge-bg)',
        'badge-text': 'var(--badge-text)',
        'badge-gold': 'var(--badge-gold)',
        // 多主题语义化键(随 .theme-* 切换)
        divider: 'var(--divider)',
        'card-raised': 'var(--card-raised)',
      },
      borderRadius: {
        // shadcn 标准(基准 --radius)
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
        // 苹果材质分档(设计规格 §2.1)
        window: 'var(--radius-window)',
        card: 'var(--radius-card)',
        'card-small': 'var(--radius-small)',
        thumb: 'var(--radius-thumb)',
      },
      boxShadow: {
        // 窗口阴影:多层半透明 + 顶边高光(苹果质感核心)
        window: 'var(--shadow-window)',
      },
      transitionTimingFunction: {
        // emilkowalski 纪律:绝不近似
        'ease-out-strong': 'var(--ease-out)',
        'ease-in-out-strong': 'var(--ease-in-out)',
      },
      transitionDuration: {
        fast: 'var(--duration-fast)',
        base: 'var(--duration-base)',
      },
      fontSize: {
        // Spotlight 窗口:较小字号
        xs: ['0.75rem', { lineHeight: '1rem' }],
      },
      letterSpacing: {
        // 苹果字体层级:大字负字距,小字正字距(设计规格 §2.6)
        tightish: '-0.01em',
        caption: '0.01em',
      },
    },
  },
  plugins: [],
}
