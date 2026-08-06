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
      '@lib': path.resolve(__dirname, './src/lib'),
      '@components': path.resolve(__dirname, './src/components'),
      '@hooks': path.resolve(__dirname, './src/hooks'),
      '@pages': path.resolve(__dirname, './src/pages'),
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
      // 不监听 src-tauri,避免 Rust 改动触发前端热重载
      ignored: ['**/src-tauri/**'],
    },
  },
})
