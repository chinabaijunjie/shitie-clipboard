---
title: 架构概述
tags: [架构, 概述]
created: 2026-06-29
updated: 2026-08-05
status: active
---

# 架构概述

## 技术栈

- **桌面壳 + 后端**:Tauri 2(Rust)— 单进程,既是壳也是后端
- **前端**:React 18 + TypeScript + Vite + Tailwind CSS + shadcn/ui
- **状态**:zustand
- **剪贴板访问(M1+)**:平台原生 API(macOS NSPasteboard / Windows Win32)
- **存储(M1+)**:rusqlite + refinery(SQLite 内嵌)

## 进程模型

```
┌─────────────────────────────────────────────┐
│  Tauri 主进程(Rust)                         │
│  ┌─────────────┐    ┌─────────────────────┐ │
│  │ WebView     │    │ commands/           │ │
│  │ (React 前端)│◄──►│ ping / clipboard* / │ │
│  │ Tailwind    │    │ storage*            │ │
│  │ Spotlight   │    │ AppState(M1+ 监听)  │ │
│  └─────────────┘    └─────────────────────┘ │
│         invoke(IPC,无 HTTP)                  │
└─────────────────────────────────────────────┘
```

> *标记的 command 在 M1+ 实现。M0 阶段仅 ping。

**关键**:无 sidecar / 无端口 / 无 HTTP。前端与 Rust 通过 Tauri invoke 直接 IPC 通信,
零网络开销。这是 M0 架构清理(sidecar→纯 Rust)后的形态。

## 分层

### Rust 侧(src-tauri/src/)

```
commands/    Tauri command(暴露给前端 invoke),按业务领域拆子模块
error.rs     AppError 错误契约({code, message} 序列化给前端)
lib.rs       应用入口 + AppState 注册 + plugin-log
main.rs      二进制入口(只调 lib::run)
```

### 前端(src/)

```
components/  SpotlightPanel(主面板)+ ui(shadcn 组件,M1+ 按需加)
core/        api(invoke 通信层)/ constants / storage / events / types / utils
lib/         shadcn cn() helper
```

## 关键资产

| 资产 | 文件 | 评级 | 说明 |
|------|------|------|------|
| Tauri invoke 通信层 | `src/core/api/invoke.ts` | A | 无 HTTP,invoke + InvokeError 错误码镜像 |
| AppError 错误契约 | `src-tauri/src/error.rs` | A | `{code, message}` 序列化,两端错误码镜像 |
| Spotlight 窗口配置 | `src-tauri/tauri.conf.json` | A | 无边框/透明/置顶/失焦隐藏,macOSPrivateApi |
| 启动探活模式 | `src/components/SpotlightPanel.tsx` | A | invoke ping 探活,避开 setup emit 时序坑 |
| 20 条通用工程纪律 | `wiki/architecture/engineering-principles.md` | S | 任何软件工程适用 |

详见 [[constraints]](技术红线)+ [[engineering-principles]](工程纪律)。

---

## 历史架构(sidecar 时代,M0 已清理)

> 本工程从模板 `tauri-sidecar-template` 复制创建,原架构是
> 「Tauri + Python FastAPI sidecar 双进程」。M0 已完成清理,
> sidecar 时代的方法论(三重兜底进程管理 / frozen 路径解析 / Fernet 加密)
> 已归档至 `wiki/dev/archive/sidecar-methodology-extract.md`(模板母体仍用)。
