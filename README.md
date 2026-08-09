# 拾贴

> 拾起每一次复制,粘贴你想要的过去。

轻量常驻的剪贴板历史管理桌面应用。复制过的文字、图片、文件、链接,随手找回,一键回填。

## ✨ 特性

- **📋 全格式捕获** — 文字 / 富文本 / 图片 / 文件 / 链接 / Markdown,复制即收录
- **🔍 秒级搜索** — SQLite FTS5 全文检索 + 中文 bigram 分词,千条历史也不卡
- **⚡ 唤起即用** — 全局快捷键呼出 Spotlight 风格窗口,上下键选中,回车/双击回填
- **🗄️ 存储管理** — 按类型统计占用,一键清除历史,可设保留天数/容量/条数上限
- **🔒 隐私守护** — 自动识别并过滤 API Key / Token / 私钥等敏感信息(可开关)
- **🎨 三套主题** — 深色 / 浅色 / 折中,毛玻璃质感 + 窗口透明度可调
- **🚀 常驻后台** — 系统托盘 + 开机自启,无感运行,不占注意力

## 📸 预览

<!-- TODO: 补充截图 -->

## 📦 安装

### macOS

1. 从 [Releases](../../releases) 下载 `拾贴_x.x.x_aarch64.dmg`(Apple Silicon)或 `x64.dmg`(Intel)
2. 打开 DMG,拖动「拾贴」到 Applications
3. 首次打开:右键 → 打开(未签名需手动放行)

### Windows

1. 下载 `拾贴_x.x.x_x64-setup.exe`
2. 运行安装,SmartScreen 警告点「仍要运行」

## 🛠️ 技术栈

| 层 | 技术 |
|----|------|
| 桌面壳 + 后端 | Tauri 2(Rust 单进程,无 sidecar / 无 HTTP) |
| 前端 | React 18 + TypeScript + Vite |
| UI | Tailwind CSS + shadcn/ui + lucide-react |
| 剪贴板 | clipboard-rs(多格式读写 + 监听) |
| 存储 | rusqlite(bundled SQLite + FTS5 全文搜索) |
| 状态 | zustand |

**架构亮点**:纯 Tauri 单进程——Rust 既是壳也是后端,前端通过 invoke 直接通信。轻量、启动快、零网络开销。

## 🔧 开发

### 环境要求

- Node.js 18+
- Rust(toolchain via [rustup](https://rustup.rs))
- [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)

### 本地运行

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发模式(热更新 + Tauri 窗口)
```

### 测试 + 构建

```bash
npm test             # 前端测试(vitest)
npm run build        # 前端构建
cd src-tauri && cargo test  # Rust 测试
npm run tauri build  # 打包桌面应用(.app / .dmg / .exe / .msi)
```

## 📁 工程结构

```
├── src/                     React 前端
│   ├── components/          SpotlightPanel + SettingsPanel + ClipboardList
│   ├── core/                api / constants / theme / settings
│   └── hooks/               useClipboardItems / useStorageStats
├── src-tauri/               Tauri 2 Rust 层
│   └── src/
│       ├── commands/        clipboard / settings / storage 后端命令
│       ├── clipboard/       监听 + 捕获 + 去重 + 敏感过滤
│       └── storage/         SQLite + FTS5 + 图片存储 + 自动清理
├── .github/workflows/       CI 跨平台打包(macOS / Windows)
└── wiki/                    项目知识库(架构 / 约束 / 决策)
```

## 🗺️ 开发路线

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **M0** | 架构清理 + Spotlight UI 骨架 | ✅ |
| **M1** | 剪贴板监听 + 存储 + 历史列表 + 回填 | ✅ |
| **M2** | 搜索(FTS5)+ 富文本/图片/文件多格式 | ✅ |
| **M3** | 常驻体验(快捷键 + 托盘 + 自启 + 设置 + 自动清理) | ✅ |
| **M4** | 打包 CI + 去重 + 敏感过滤 + UI 改造 + 多主题 | ✅ |

## License

MIT
