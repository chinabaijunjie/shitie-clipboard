# Clipboard Manager — 通用工程模板

> 本文件是 **规则手册 + 知识库入口**。
>
> 具体开发细节（架构 / 约束 / 决策原则 / 工程流程）全部在 `wiki/` 下。
> AI 想了解任何内容，**先查 `wiki/index.md` 总索引**。

## 项目简介

**Clipboard Manager** 是 Tauri 2 + React 18 + Python FastAPI sidecar 的通用工程模板。

**技术栈**：Tauri 2.x + React 18 + Ant Design 5 + Python 3.10 (FastAPI sidecar)

**定位**：持续进化的母体——新工程从模板复制起步，开发成果提炼后回流模板，让模板不断进化。
详见 `README.md`「进化模式」段 + `wiki/process/template-evolution-guide.md`。

---

## 🚀 AI 启动 3 步（每次新会话必做）

1. **读 `wiki/dev/new-session-starter.md`** — 跨会话恢复入口
2. **读 `wiki/index.md`** — 知识库总索引（场景驱动速查）
3. **读本文件下方「核心原则 + 知识查询决策树」** — 不查文档也能记住的最低基线

> §8 通用工程纪律（不可变性 / 文件组织 / 错误处理 / 输入校验 / Git 格式）由 Claude Code
> 全局 rules（`~/.claude/rules/`）覆盖，不在此重复。ZCode 用户的等价内容在 `AGENTS.md` §8。

---

## 工程结构

```
clipboard-manager/
├── CLAUDE.md / AGENTS.md    ← 工程说明（双工具同步，见 wiki/process/dual-tool-sync.md）
├── README.md                ← 模板使用说明（含进化模式）
├── CHANGELOG.md             ← 版本追溯（母体进化记录）
├── src/                     ← React 前端（pages/modules/components/core）
├── src-tauri/               ← Tauri 2 Rust 层（sidecar 进程管理）
├── backend/                 ← Python FastAPI sidecar（api/services/core 四层 + tests）
├── scripts/                 ← 工具脚本（build-sidecar.sh / hooks/）
└── wiki/                    ← 项目知识库（架构 / 决策 / 流程）
```

详细架构 / 关键资产 → `wiki/architecture/overview.md`。

---

## 开发环境

- **conda 环境**：复用任意含 FastAPI/uvicorn/sqlalchemy/alembic 的 Python 环境
  （开发期通过 `SIDECAR_DEV_PYTHON` 环境变量指定解释器，见 `.env.example`）
- **端口**：sidecar 9880（三处对齐：Rust sidecar.rs + config.py + constants.ts）

技术红线（端口对齐 / 错误码镜像 / frozen 路径 / SQLite PRAGMA / alembic 适配）详见
`wiki/architecture/constraints.md`——**开发前必读**。

---

## 核心原则（启动时必须知道，不需要查文档）

1. **后端三原则**：稳定 > 架构 > 维护性 > 短期成本（所有后端决策依据）
2. **调查先行**：开工前必有规划文档，调研结果必须立刻持久化到 wiki/
3. **严格按规划 + 动态校准**：执行中不调整，每个切换节点重新调研
4. **主动暴露错误**：发现自己写错立刻修正，加「⚠️ 重要更正」段，不删原文
5. **实测发现为准**：决策前重新跑测试 / 类型检查 / lint，不靠内存中的旧结论
6. **关键决策 spot check**：5 类决策（架构变更 / 跨模块影响 / 不可逆操作 / 新依赖引入 / 安全相关）即使已授权也必须 spot check

完整 20 条通用原则 → `wiki/architecture/engineering-principles.md`。

---

## 知识查询决策树

| 你想做什么 | 用什么工具 |
|-----------|-----------|
| 查工程纪律：修 bug / 重构 / 新功能 | `wiki/architecture/engineering-principles.md`（20 条）|
| 查约束：技术红线（端口 / 错误码 / frozen / SQLite）| `wiki/architecture/constraints.md` |
| 查架构：进程模型 / 分层 / 关键资产 | `wiki/architecture/overview.md` |
| 查进度：当前 TODO / 历史 | `wiki/dev/TODO.md` + `wiki/dev/progress.md` |
| 跨会话恢复 | `wiki/dev/new-session-starter.md` |
| 🔴 模板回流（成果回流模板）| `wiki/process/template-evolution-guide.md` |

---

## 工作规则

- **每次完成阶段性工作时 commit**——commit 前用 `wiki/process/code-review-checklist.md` 自检
- **开发新模块前**先读 `wiki/index.md` 找对应设计文档
- **写/改 wiki 内容前**先读 `wiki/process/knowledge-base-guide.md`（防膨胀）
- **改 AGENTS.md 后必须同步 CLAUDE.md**（除 §8 受保护内容，详见 `wiki/process/dual-tool-sync.md`）
- **进度追踪**：每完成一个任务项立即更新 `wiki/dev/progress.md` + `wiki/dev/TODO.md`
- **改动生效验证**（改完代码主动让服务加载新代码，不等用户提醒）：改后端 `.py` → 主动重启 sidecar；改前端 `.ts/.tsx` → 提示用户刷新浏览器；改 `src-tauri/` → 告知需重跑 tauri dev
- **会话结束时**（识别到结束信号即启动）：
  - ⚠️ **第一件事：读 `wiki/dev/session-checklist.md` 按清单收尾**
  - 触发词：「存档」「下次继续」「换个新窗口」「今天先到这」「收尾」「结束」等
  - 收尾完成后**主动向用户报告核查结果**
- **🔴 子工程回流识别**（本工程是模板子工程时）：每次干活多扫一眼当前任务涉及的代码/设计，
  对照 `wiki/process/reflux-signals.md` 信号清单判断"这块通用吗"。命中信号 → **主动提示用户**
  并记入 `wiki/dev/REFLUX-CANDIDATES.md`。用户专注业务，AI 负责"盯回流候选"。

---

_最后更新：2026-06-29（模板 v1.0.0 初始落地）_
