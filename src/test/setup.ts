/**
 * vitest 全局 setup — 每个测试文件执行前运行
 *
 * 回流自 ai-audiobook 工程（TODO-029，2026-07-03），通用资产。
 *
 * EventSource / matchMedia 等不在 happy-dom globalThis 的 API，由各测试文件在
 * beforeEach 里 vi.stubGlobal 注入（按需，不全局 stub 避免污染不需要它的测试）。
 */
import '@testing-library/jest-dom/vitest'
