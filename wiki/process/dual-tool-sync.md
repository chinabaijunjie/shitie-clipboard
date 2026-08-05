---
title: 双工具同步契约（CLAUDE.md ↔ AGENTS.md）
tags: [工具, 同步, 契约]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# 双工具同步契约

> **触发场景**：用 Claude Code + ZCode 双工具并行开发时，改动 `CLAUDE.md` / `AGENTS.md` 的工作说明。
>
> 单工具项目可忽略本文件。

## 背景

Claude Code 读 `CLAUDE.md`，ZCode 读 `AGENTS.md`。两份文件内容应保持同步
（都是"AI 工作说明"），但允许少量工具专属差异。

## 双方共同的责任

- 改了主文件（你用的那份）→ 同步到对方文件
- 接受对方文件的反向同步（对方改了，你同步过来）

## 永不同步（AGENTS.md 专属内容）

以下内容**只在 AGENTS.md 保留，不同步到 CLAUDE.md**：

- **§8 通用工程纪律整段**：ZCode 不支持用户级全局规则（与 Claude Code 的 `~/.claude/rules/` 不同），
  这些通用工程纪律只能写进每个项目的 AGENTS.md 才能对 ZCode 生效。Claude Code 已通过全局 rules 覆盖，同步会造成重复。
- **Code Review 的工具专属补充项**：同上原因，只在 AGENTS.md 保留。

> 这是工具能力差异导致的必然结果，不是疏漏。

## 同步检查

- 两文件归一化后 diff 应仅限上述受保护内容
- 自动化兜底：`scripts/hooks/pre-commit` hook 检查双文件同步状态
  （归一化后 diff 为空才放行 commit，否则阻塞）

## 单工具项目

若项目只用一个工具（如只用 Claude Code），可只保留对应的一份文件，
删除另一份 + 删除 pre-commit hook。
