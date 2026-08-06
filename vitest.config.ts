/**
 * vitest 配置 — 独立于 vite.config.ts
 *
 * 理由：Vite v5+ 从自身类型删了 test 字段，嵌入 vite.config.ts 要加三斜线指令（不优雅）。
 * 独立文件用 vitest/config 的 defineConfig，alias 与 vite.config.ts / tsconfig.app.json 保持一致。
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
      '@lib': path.resolve(__dirname, './src/lib'),
      '@components': path.resolve(__dirname, './src/components'),
      '@hooks': path.resolve(__dirname, './src/hooks'),
      '@pages': path.resolve(__dirname, './src/pages'),
    },
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
})
