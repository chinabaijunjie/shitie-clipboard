---
title: 回流候选池
tags: [回流, 候选池, 子工程]
created: 2026-06-29
updated: 2026-08-07
status: active
---

# 回流候选池

> **子工程专属**：本文件随模板复制到子工程后，记录开发中发现的"可能回流模板"的候选。
>
> **怎么用**：
> - AI 主动识别到回流候选 → 记一条到下面「待评估」
> - 你/我开发中灵光一现 → 也记一条
> - **不靠脑子记**——记下来就忘，到收尾时 `session-checklist` 会强制过一遍
>
> 收尾处理流程见 `wiki/process/reflux-signals.md`。

---

## 待评估

<!-- 格式：
### [简述] — YYYY-MM-DD
- **内容**：做了什么
- **为什么像通用**：对应哪个信号
- **状态**：待评估 / 已回流 / 留子工程 / 丢弃
-->

### [FTS5 中文 bigram + migration 版本号 + bundled 含 FTS5 发现] — 2026-08-07
- **内容**:M2 实现搜索时发现三件可能通用的事:
  1. rusqlite `bundled` feature 已无条件编译 FTS5(libsqlite3-sys build.rs),无需加任何 feature 即可用 `CREATE VIRTUAL TABLE USING fts5`
  2. FTS5 中文搜索方案:unicode61 + 应用层 bigram 预处理(30 行纯函数)+ LIKE 单字兜底,零依赖零 unsafe
  3. SQLite migration 手写 `PRAGMA user_version`(ALTER TABLE 不幂等需版本号守护,且整段要包事务)
- **为什么像通用**:对应 reflux-signals「通用技术坑/方案」—— 任何需要本地搜索的 Tauri/Rust 桌面应用都会遇到 FTS5 中文 + migration,模板母体若加这些约束可帮到其他子工程
- **状态**:待评估(收尾时提示用户;剪贴板专用性强,但 FTS5 中文 + migration 模式可能值得回流 constraints.md)
- **状态**：待评估 / 已回流 / 留子工程 / 丢弃
-->

（空——子工程开发中由 AI/你随时记录）

---

## 已处理（追溯用）

<!-- 处理完的条目从「待评估」移到这里，标记最终去向 -->
