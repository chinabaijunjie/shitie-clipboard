---
title: 架构概述
tags: [架构, 概述]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# 架构概述

## 技术栈

- **前端**：React 18 + Ant Design 5 + TypeScript + Vite
- **桌面壳**：Tauri 2（Rust）
- **后端 sidecar**：Python 3.10 + FastAPI + SQLAlchemy 2.0 async + SQLite

## 进程模型

```
┌─────────────────────────────────────────────┐
│  Tauri 主进程（Rust）                         │
│  ┌─────────────┐    ┌─────────────────────┐ │
│  │ WebView     │    │ sidecar.rs          │ │
│  │ (React 前端)│◄──►│ 进程管理（三重兜底） │ │
│  │  :5173 dev  │    │  spawn → :9880      │ │
│  └─────────────┘    └─────────┬───────────┘ │
└──────────────────────────────┬──────────────┘
                               │ spawn + env 注入
                               ▼
                    ┌─────────────────────────┐
                    │ Python sidecar (FastAPI)│
                    │  :9880                  │
                    │  /api/health + 业务路由  │
                    │  SQLite + alembic       │
                    └─────────────────────────┘
```

## 分层（后端）

```
api/        路由层（请求解析 + 调 service + 包装响应）
services/   业务逻辑层（封装业务 + 抛 AppError）
core/       通用层（config/paths/database/errors/logger/events/models）
engines/    引擎层（业务工程起步时填充，如模型推理/外部服务）
```

## 关键资产（S/A 级，跨产品复用）

| 资产 | 文件 | 评级 | 说明 |
|------|------|------|------|
| sidecar 三重兜底进程管理 | `src-tauri/src/sidecar.rs` | S | 任何 Tauri+sidecar 必用 |
| frozen/开发双模式路径解析 | `backend/core/paths.py` | S | 任何 PyInstaller 打包必用 |
| 前后端错误码镜像契约 | `errors.py` ↔ `types.ts` | S | 任何前后端分离必用 |
| SQLite 加固 | `backend/core/database.py` | A | 用 SQLite 的应用必用 |
| 启动守卫模式 | `SetupGate.tsx` | A | sidecar 冷启动慢的应用必用 |

详见 [[constraints]]（技术红线）+ [[engineering-principles]]（工程纪律）。
