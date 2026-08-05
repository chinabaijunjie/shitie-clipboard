---
title: 知识库总索引（门面）
tags: [索引, 门面, 导航]
created: 2026-06-29
updated: 2026-08-05
status: active
---

# 知识库总索引

> 本文件是知识库"门面"。新文档/新目录/新模块 → **立刻更新本文件**。
> 状态性内容（进度/Tier）→ 会话结束统一更新。

---

## 🚨 当前状态(M0 完成,下一步 M1)

> **M0 已完成**(2026-08-05):架构清理 + Spotlight UI 骨架,8 阶段 7 commit。
> 详见 [[dev/m0-plan]](规划)+ [[dev/progress]](完成记录)。
>
> **下一步:M1 macOS 剪贴板监听 MVP**(NSPasteboard.changeCount + rusqlite + 历史列表 + 回填)。
> M1 启动前需先做技术 spike + 规划文档(§1.16)。详见 [[dev/TODO]]。
>
> **历史轨迹**:旧版温和迁移规划已归档至 [[dev/archive/001-old-m0-sidecar-removal]],
> sidecar 方法论删前留档见 [[dev/archive/sidecar-methodology-extract]]。

---

## 场景驱动速查

| 你想做什么 | 用什么文档 |
|-----------|-----------|
| 查工程纪律（修 bug / 重构 / 新功能） | [[architecture/engineering-principles]]（20 条）|
| 查技术红线（Tauri setup 时序坑 / SQLite / tokio）| [[architecture/constraints]] |
| 查架构（进程模型 / 分层 / 关键资产）| [[architecture/overview]] |
| 查 AI 协作风格（"你定吧" / 自动决策）| [[process/ai-collaboration-style]] |
| commit 前 / Stage 自检 | [[process/code-review-checklist]] |
| 会话收尾 | [[dev/session-checklist]]（8 步）|
| 改 AGENTS.md / CLAUDE.md | [[process/dual-tool-sync]] |
| 写/改 wiki 内容（防膨胀）| [[process/knowledge-base-guide]] |
| 查当前任务规划（M0/M1+）| [[dev/m0-plan]] |
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
- [[sidecar-to-rust-migration-guide]] — sidecar→Rust 原生迁移指南(参考,M0 已执行)
- [[archive]] — 已归档流程文档(回流机制等模板母体专用)

### dev/（进度 + TODO + 启动包）
- [[dev/m0-plan]] — 🚨 **当前任务规划**(M0 架构清理 + Spotlight UI 骨架)
- [[dev/progress]] — 开发进度（状态唯一真相源）
- [[dev/TODO]] — 全局 TODO
- [[dev/new-session-starter]] — 新窗口启动包
- [[dev/archive]] — 历史归档(旧规划 / 删前留档)

## 关键资产优先级

```
engineering-principles（20 条工程纪律）> 会话收尾 8 步清单
> Tauri v2 setup 时序坑（constraints）> 双工具同步 pre-commit hook
> m0-plan（当前任务规划）
```

> ⚠️ sidecar.rs / paths.py / database.py 等 sidecar 时代资产在 M0 后已删,
> 方法论摘要见 [[dev/archive/sidecar-methodology-extract]](模板母体仍用)。
