import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'

// Tauri 默认会通过环境变量注入主机端口和 HMR 配置，这里显式声明 fallback
const host = process.env.TAURI_DEV_HOST

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
  // Tauri 在 dev 模式下用固定端口 5173
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: process.env.TAURI_STAGE === 'tauri' ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // 不监听 src-tauri / backend，避免 Rust/Python 改动触发前端热重载
      ignored: ['**/src-tauri/**', '**/backend/**'],
    },
  },
})
