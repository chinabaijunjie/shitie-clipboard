/**
 * vitest 配置 — 独立于 vite.config.ts
 *
 * 回流自 ai-audiobook 工程（TODO-029，2026-07-03），通用资产。
 *
 * 理由：Vite v5+ 从自身类型删了 test 字段，嵌入 vite.config.ts 要加三斜线指令（不优雅）。
 * 独立文件用 vitest/config 的 defineConfig，alias 直接复用 vite.config.ts 的约定。
 */
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'node:path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@core': path.resolve(__dirname, './src/core'),
      '@modules': path.resolve(__dirname, './src/modules'),
      '@components': path.resolve(__dirname, './src/components'),
      '@pages': path.resolve(__dirname, './src/pages'),
      '@styles': path.resolve(__dirname, './src/styles'),
    },
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
})
