<div align="center">

# 拾贴

**拾起每一次复制，粘贴你想要的过去。**

轻量常驻的剪贴板历史管理桌面应用，支持 macOS & Windows。

[下载安装](#-安装) · [功能特性](#-功能特性) · [使用指南](#-使用指南)

</div>

---

## 🌟 这是什么

你复制过的每一段文字、每一张图片、每一个文件链接——**拾贴**都帮你记着。

它安静地常驻在你的系统托盘里，你随时可以通过快捷键呼出它，像 Spotlight 一样搜索、浏览、一键回填你复制过的任何内容。不需要再翻聊天记录找之前复制的链接，不需要担心覆盖了重要内容。

## ✨ 功能特性

### 📋 全格式捕获
复制的文字、富文本、图片、文件、链接、Markdown——自动收录，一个都不漏。

### 🔍 秒级搜索
基于 SQLite FTS5 全文检索引擎，支持中英文混合搜索。上千条历史记录也能瞬间找到。

### ⚡ 唤起即用
全局快捷键（默认 `Ctrl/Cmd + Shift + V`，支持自定义）随时呼出 Spotlight 风格窗口。上下键选中，回车或双击即可回填到剪贴板。

### 🏷️ 类型筛选
文字、图片、文件、链接、Markdown——按类型一键过滤，快速定位你要的内容。

### 🗄️ 存储管理
按类型统计存储占用，一键清除历史。支持设置保留天数、容量上限、条数上限，自动清理过期内容。

### 🔒 敏感信息过滤
自动识别并跳过部分已知格式的云服务密钥（AWS / GitHub / GitLab / Slack / Stripe / Google）和 PEM 格式私钥（可在设置中开关）。

> ⚠️ **此过滤范围有限，仅覆盖特定前缀的密钥格式。** 它拦不住密码、Bearer Token、数据库连接串、`.env` 变量、图片/截图中的敏感信息，也无法覆盖所有云厂商的密钥。
>
> **请不要用拾贴存储密码或任何高敏感信息。** 密码请使用 [1Password](https://1password.com/)、[Bitwarden](https://bitwarden.com/) 等专业密码管理器。

### 🎨 精致外观
三套主题（深色 / 浅色 / 折中），毛玻璃质感界面，窗口透明度可调。精心设计的图标和交互动效。

### 🚀 无感常驻
系统托盘常驻 + 开机自启，不占桌面空间，不抢注意力。需要时一秒呼出，用完自动隐藏。

## 📦 安装

### macOS

1. 从 [Releases](../../releases) 下载 `.dmg` 文件
   - Apple Silicon（M1/M2/M3/M4）：下载 `aarch64.dmg`
   - Intel Mac：下载 `x64.dmg`
2. 打开 DMG，将「拾贴」拖入 Applications 文件夹
3. 首次打开时右键选择「打开」（未签名应用需要手动放行）

<details>
<summary><b>遇到「应用已损坏」提示？</b></summary>

由于应用没有 Apple 开发者签名，macOS 可能会提示「应用已损坏，应该移到废纸篓」。这不是真的损坏，解决方法：

**方法一：终端命令（推荐）**

打开「终端」，输入以下命令，然后**把「拾贴」图标拖到终端窗口里**（会自动填入路径），按回车：

```bash
sudo xattr -rd com.apple.quarantine
```

输入你的 Mac 登录密码（输入时不会显示字符，正常输入即可），回车后重新打开应用即可。

**方法二：关闭 Gatekeeper（更宽松）**

```bash
sudo spctl --master-disable
```

这会允许打开任何来源的应用。使用后建议重新开启：`sudo spctl --master-enable`。

</details>

### Windows

1. 从 [Releases](../../releases) 下载 `x64-setup.exe`
2. 运行安装程序（SmartScreen 警告点击「仍要运行」）

## 📖 使用指南

### 基本操作

| 操作 | 快捷键 |
|------|--------|
| 呼出 / 隐藏窗口 | `Ctrl/Cmd + Shift + V`（可自定义） |
| 上下浏览 | `↑` `↓` |
| 回填到剪贴板 | `Enter` 或 `双击` |
| 删除某条记录 | `Backspace` |
| 搜索 | `/` 聚焦搜索框，或直接输入 |

### 快速上手

1. **安装后启动**——拾贴会常驻在系统托盘
2. **正常使用复制**——复制任何内容都会自动收录
3. **按快捷键呼出**——浏览历史，搜索需要的内容
4. **回车回填**——选中后回车，内容回到剪贴板，直接 `Ctrl/Cmd + V` 粘贴

### 自定义设置

点击窗口右上角的 ⚙️ 图标进入设置：

- **快捷键**——自定义全局唤起快捷键
- **外观**——深色 / 浅色 / 折中三套主题
- **窗口透明度**——15% ~ 100% 可调
- **开机自启**——开关系统启动时自动运行
- **存储管理**——设置保留天数、容量上限、条数上限
- **敏感过滤**——开关 API Key / Token 自动过滤

## 📸 预览

<!-- 在这里添加截图 -->
<!-- ![主界面](screenshots/main.png) -->
<!-- ![设置面板](screenshots/settings.png) -->

*截图待补充*

## 💻 支持平台

| 平台 | 状态 |
|------|------|
| macOS（Apple Silicon） | ✅ |
| macOS（Intel） | ✅ |
| Windows 10/11 | ✅ |

## 🔧 技术栈

拾贴基于 [Tauri 2](https://v2.tauri.app/) 构建——Rust 后端 + React 前端，纯单进程架构，轻量、启动快、零网络开销。

<details>
<summary>技术细节</summary>

- **桌面框架**：Tauri 2（Rust）
- **前端**：React 18 + TypeScript + Vite
- **UI**：Tailwind CSS + shadcn/ui + lucide-react
- **剪贴板**：clipboard-rs（跨平台多格式读写 + 监听）
- **存储**：rusqlite + SQLite FTS5（全文搜索）
- **状态管理**：zustand

**内存占用**：运行时约 150~200MB，主要来自系统 WebView 引擎（WebKit）和 JavaScript 运行时，应用本体仅 5MB 左右。这是基于 Web 技术栈的跨平台桌面应用的固有开销（同等功能的 Electron 应用通常 300MB+）。

</details>

详细的开发文档、架构说明和工程规范请参考 [DEVELOPMENT.md](DEVELOPMENT.md)。

## 📝 License

MIT

## 应用图片

<img width="2596" height="2090" alt="图片1" src="https://github.com/user-attachments/assets/4e9544e4-2daa-4625-aa2b-a0dd2190d97d" />

<img width="1416" height="1382" alt="图片2" src="https://github.com/user-attachments/assets/5f60ba20-ff7d-4032-9957-741e50dfbca2" />

<img width="1568" height="1532" alt="图片3" src="https://github.com/user-attachments/assets/80d7d89b-7a65-4c28-935a-7c96a6e9c51b" />

<img width="1542" height="1442" alt="图片4" src="https://github.com/user-attachments/assets/30c152c7-9749-4457-98cb-31cf16210115" />

<img width="1620" height="1474" alt="图片5" src="https://github.com/user-attachments/assets/c2a69e58-a215-419c-91be-9e23d5d205db" />

<img width="1490" height="1432" alt="图片6" src="https://github.com/user-attachments/assets/95e2cca0-706b-440f-abd3-6323e34f92d2" />

<img width="1456" height="1492" alt="图片7" src="https://github.com/user-attachments/assets/a8a9a4b8-8270-4afe-924c-4ecd76f5bf2c" />

