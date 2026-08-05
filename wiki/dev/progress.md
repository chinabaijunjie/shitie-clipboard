---
title: 开发进度（状态唯一真相源）
tags: [进度, 状态]
created: 2026-06-29
updated: 2026-07-03
status: active
---

# 开发进度

> **📌 当前状态（唯一真相源）**：版本号 / 测试数 / lint 只在本段维护，别处放指针。

## 当前状态

- **模板版本**：v1.2.0（2026-07-03 新增版本 bump 规则 + 同步锚点机制）
- **后端测试**：18 passed（unit 4 + integration 6 + api 8)
- **前端**：tsc 通过 + vite build 通过
- **Rust**：cargo check 通过 + cargo test 10 passed
- **打包**：PyInstaller onefile binary 31M，health 200 + items CRUD 通

## 历史版本

### v1.2.0 — 新增版本 bump 规则 + 同步锚点机制（2026-07-03）

完善回流机制：明确「谁 bump / 何时 bump / bump 同步哪些位置」+「版本号作为模板与子工程的同步锚点」。
- `template-evolution-guide.md` 版本约定节扩充（bump 触发表 + 同步位置清单 + 同步锚点说明）

### v1.1.0 — 回流 pixel-matrix Stage 5A（2026-07-03）

子工程 pixel-matrix 剥离 Python sidecar 改 Rust 原生后端，提炼迁移方法论回流模板：
- 新增 `wiki/process/sidecar-to-rust-migration-guide.md`（sidecar→Rust 迁移指南）
- `constraints.md` 新增「Tauri v2 setup 时序坑」节（setup 内 emit 会被丢弃）
- 索引同步（index.md 速查表 + 分类导航）

**回流 commit**：`3659169`。详见 CHANGELOG.md [1.1.0]。

### v1.0.0 — 模板初始落地（2026-06-29）

从 ai-audiobook 项目沉淀的通用工程模板，10 个 commit：
1. Tauri 2 + React + TS 工程骨架
2. Python sidecar 进程管理（三重兜底，S 级）
3. FastAPI sidecar 骨架（health/status + core 分层 + alembic）
4. 示例 CRUD（items）+ 第一个 migration
5. 前端 core 骨架（api client + 启动守卫 SetupGate）
6. 示例 HomePage（调 /api/items 全链路）+ UI 组件
7. 工程质量基线（ruff/mypy/pytest + conftest + 示例测试）
8. PyInstaller sidecar 打包脚本（onefile）
9. 知识库骨架 + 工程方法论套件 + 进化机制
10. README 进化模式说明 + CHANGELOG

**验证**：tauri dev 起空窗 + sidecar health 200 + items CRUD 全链路 + 打包 binary 可运行。
