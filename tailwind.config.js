/**
 * Tailwind 配置 — shadcn/ui 风格
 *
 * 设计要点:
 * - darkMode: 'class'(通过 data-theme / class 切换,M0 沿用原 light/dark 主题)
 * - 颜色走 CSS 变量(--background / --foreground 等),支持主题切换
 * - content 覆盖 src 全部 .ts/.tsx(含 components/ui shadcn 组件)
 * - 不启用 Tailwind 的 preflight(shadcn 的 globals.css 已含 reset)
 *
 * shadcn 组件默认用以下颜色键:background/foreground/card/popover/primary/
 * secondary/muted/accent/destructive/border/input/ring。
 */
/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
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
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      // Spotlight 窗口:较小字号 + 紧凑间距
      fontSize: {
        xs: ['0.75rem', { lineHeight: '1rem' }],
      },
    },
  },
  plugins: [],
}
