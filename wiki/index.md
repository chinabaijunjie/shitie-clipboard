---
title: 知识库总索引（门面）
tags: [索引, 门面, 导航]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# 知识库总索引

> 本文件是知识库"门面"。新文档/新目录/新模块 → **立刻更新本文件**。
> 状态性内容（进度/Tier）→ 会话结束统一更新。

## 场景驱动速查

| 你想做什么 | 用什么文档 |
|-----------|-----------|
| 查工程纪律（修 bug / 重构 / 新功能） | [[architecture/engineering-principles]]（20 条）|
| 查技术红线（端口对齐 / 错误码镜像 / frozen 路径）| [[architecture/constraints]] |
| 查架构（进程模型 / 分层 / 关键资产）| [[architecture/overview]] |
| 查 AI 协作风格（"你定吧" / 自动决策）| [[process/ai-collaboration-style]] |
| commit 前 / Stage 自检 | [[process/code-review-checklist]] |
| 会话收尾 | [[dev/session-checklist]]（8 步）|
| 改 AGENTS.md / CLAUDE.md | [[process/dual-tool-sync]] |
| 写/改 wiki 内容（防膨胀）| [[process/knowledge-base-guide]] |
| 🔴 剥离 sidecar 改 Rust 原生后端 | [[process/sidecar-to-rust-migration-guide]]（含 Tauri v2 setup 时序坑）|
| 🔴 模板回流（新工程成果回流模板）| [[process/template-evolution-guide]] |
| 🔴 开发中怎么发现回流候选（AI 主动识别）| [[process/reflux-signals]] |
| 查进度 / 下次目标 | [[dev/progress]] + [[dev/TODO]] |
| 跨会话恢复 | [[dev/new-session-starter]] |
| 查历史决策案例 | [[architecture/decision-cases]]（生长型）|

## 分类导航

### architecture/（架构 + 约束 + 决策原则）
- [[engineering-principles]] — ⭐ 20 条通用工程纪律（核心）
- [[constraints]] — 技术硬约束（红线）
- [[overview]] — 架构概述（进程模型 / 分层 / 关键资产）
- [[decision-cases]] — 历史决策案例（生长型）

### process/（工程流程 + 协作规范）
- [[code-review-checklist]] — Code Review 三套清单
- [[session-checklist]] — 会话收尾 8 步清单
- [[knowledge-base-guide]] — 知识库内容规范（防膨胀）
- [[dual-tool-sync]] — 双工具同步契约
- [[ai-collaboration-style]] — AI 协作风格（13 条 + 分块逐项决策）
- [[sidecar-to-rust-migration-guide]] — 🔴 sidecar→Rust 原生迁移指南（回流自 pixel-matrix）
- [[template-evolution-guide]] — 🔴 模板回流机制（母体进化核心）
- [[reflux-signals]] — 🔴 回流信号清单（AI 主动识别用）

### dev/（进度 + TODO + 启动包）
- [[progress]] — 开发进度（状态唯一真相源）
- [[TODO]] — 全局 TODO
- [[new-session-starter]] — 新窗口启动包

## 关键资产优先级

```
sidecar.rs（三重兜底进程管理）   > 前后端错误码契约
> paths.py（frozen 路径解析）    > engineering-principles（20 条工程纪律）
> database.py（SQLite 加固）     > SetupGate（启动守卫）
> 会话收尾 8 步清单              > 双工具同步 pre-commit hook
```
