/**
 * App — 应用根组件
 *
 * 启动守卫模式：SetupGate 等待 Python sidecar 就绪后再渲染主应用内容。
 * sidecar 启动期间显示 SplashScreen（loading），就绪后渲染主应用。
 *
 * 业务工程起步时，在此接入主应用 Layout + 路由（react-router）+ 业务页面。
 * 模板用单页面 HomePage 演示前端调后端全链路。
 */
import { SetupGate } from '@components/business/SetupGate'
import { HomePage } from '@pages/HomePage'

function App() {
  return (
    <SetupGate>
      <HomePage />
    </SetupGate>
  )
}

export default App
