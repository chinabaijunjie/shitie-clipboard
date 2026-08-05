# Clipboard Manager

> 轻量常驻的剪贴板历史管理桌面应用。

## 这是什么

一个 macOS / Windows 桌面剪贴板历史管理工具:

- **被动监听**:系统复制时自动捕获,存入本地历史
- **常驻 + 唤起**:后台运行,通过快捷键呼出 Spotlight 风格窗口(M3+)
- **历史回填**:上下键/点击选中某条 → 写入当前剪贴板 → Ctrl+V 粘贴选中的内容

**架构**:纯 Tauri 2 单进程(Rust 既是壳也是后端),无 Python sidecar / 无 HTTP。
轻量、启动快、零网络开销。

---

## 技术栈

- **桌面壳 + 后端**:Tauri 2(Rust)
- **前端**:React 18 + TypeScript + Vite
- **UI**:Tailwind CSS + shadcn/ui(键盘导航友好,适合 Spotlight 风格)
- **状态**:zustand
- **剪贴板访问(M1+)**:平台原生 API(macOS NSPasteboard / Windows Win32)
- **存储(M1+)**:rusqlite + refinery

---

## 快速开始

### 开发环境要求

- Node.js 18+
- Rust(系统 toolchain,`rustup` 安装)
- Tauri 2 系统依赖([官方文档](https://v2.tauri.app/start/prerequisites/))

### 安装 + 运行

```bash
# 1. 装前端依赖
npm install

# 2. 开发模式(自动启动 Tauri 窗口 + 前端热更新)
npm run tauri dev

# 3. 验证后端通信(DevTools Console)
await window.__TAURI__.core.invoke('ping')
# → "ok"  代表前后端通信链路正常
```

### 测试 + 构建

```bash
# 前端测试
npm test

# 前端构建
npm run build

# Rust 检查
cd src-tauri && cargo check

# 打包桌面应用(.app / .exe / .deb)
npm run tauri build
```

---

## 工程结构

```
├── AGENTS.md / CLAUDE.md    AI 工作说明(双工具同步)
├── README.md                产品说明(本文件)
├── src/                     React 前端
│   ├── components/          SpotlightPanel + ui(shadcn)
│   ├── core/                api(invoke 通信层)/ constants / storage / events
│   └── lib/                 shadcn cn() helper
├── src-tauri/               Tauri 2 Rust 层
│   └── src/
│       ├── commands/        ping 探活(M1+ 加 clipboard/storage)
│       ├── error.rs         AppError 错误契约
│       └── lib.rs           应用入口 + AppState
├── scripts/                 hooks/pre-commit(双工具同步检查)
└── wiki/                    项目知识库(架构 / 约束 / 决策原则 / 流程)
```

详细架构 → `wiki/architecture/overview.md`。
技术红线 → `wiki/architecture/constraints.md`。

---

## 开发路线图

| 里程碑 | 目标 | 状态 |
|--------|------|------|
| **M0** | 架构清理 + Spotlight UI 骨架(从 sidecar 模板改成纯 Rust) | ✅ 完成 |
| M1 | macOS 剪贴板监听 MVP(NSPasteboard.changeCount + rusqlite + 历史列表 + 回填) | 进行中 |
| M2 | 搜索 + 富文本/图片支持(SQLite FTS5) | 待开始 |
| M3 | 常驻体验(全局快捷键 + 托盘 + 自启 + Windows 补全) | 待开始 |
| M4 | 生产化(打包 dmg/exe + 去重 + 容量上限 + 敏感过滤) | 待开始 |

M0 规划详情 → `wiki/dev/m0-plan.md`。

---

## 出身说明

本工程 2026-08-05 从模板 [`tauri-sidecar-template`](../tauri-sidecar-template) 复制创建,
经 M0 架构清理从「Tauri + Python sidecar 双进程」改造为「纯 Tauri 单进程」。
模板的工程方法论(20 条通用工程纪律 + AI 协作风格 + 流程清单)沉淀在 `wiki/` 下,
随本工程持续维护。sidecar 时代的方法论已归档(`wiki/dev/archive/`)。

## License

MIT
