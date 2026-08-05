---
title: 目录规范
tags: [规范, 目录]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# 目录规范

> 详细的目录职责 + 生命周期规则见 [[process/knowledge-base-guide]]。

## wiki/ 目录结构

```
wiki/
├── index.md              门面（总索引，场景驱动速查）
├── schema.md             本文件（目录规范）
├── log.md                变更日志
├── architecture/         架构 / 约束 / 决策原则
├── process/              工程流程清单 / 协作规范
└── dev/                  进度 / TODO / 启动包
```

业务工程起步时，按需追加：
- `design/` — UI/UX 设计稿
- `modules/` — 业务模块文档
- `research/` — 调研报告 / 踩坑经验
- 业务专属目录

## 命名规范

- 文档用 kebab-case（`engineering-principles.md`）
- frontmatter 必含 title + tags + created + updated + status
- status 取值：`active` / `active-growable` / `ready-to-use` / `archived`
