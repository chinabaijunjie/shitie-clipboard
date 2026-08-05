/**
 * App — 应用根组件
 *
 * Spotlight 风格应用:无 SetupGate 启动守卫(Rust 即时启动,不需要等 sidecar 解压)。
 * 后端就绪探活由 SpotlightPanel 内部 invoke ping 完成。
 *
 * M1 起此组件接入剪贴板历史列表 + 键盘导航。
 */
import { SpotlightPanel } from '@components/SpotlightPanel'

function App() {
  return <SpotlightPanel />
}

export default App
