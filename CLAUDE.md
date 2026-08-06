# Clipboard Manager — 剪贴板历史管理工具

> 本文件是 **规则手册 + 知识库入口**。
>
> 具体开发细节（架构 / 约束 / 决策原则 / 工程流程）全部在 `wiki/` 下。
> AI 想了解任何内容，**先查 `wiki/index.md` 总索引**。

## 项目简介

**Clipboard Manager** 是轻量常驻的剪贴板历史管理桌面应用。

**技术栈**：Tauri 2.x(Rust) + React 18 + TypeScript + Tailwind CSS + shadcn/ui

**架构**：纯 Tauri 单进程(Rust 既是壳也是后端),无 Python sidecar / 无 HTTP / 无独立后端服务。
前端与 Rust 通过 Tauri invoke 直接通信。

**出身**：2026-08-05 从模板 `tauri-sidecar-template` 复制创建,M0 已完成架构清理(sidecar→纯 Rust)。

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
├── README.md                ← 产品说明 + 安装运行
├── src/                     ← React 前端（components/core/lib + SpotlightPanel）
├── src-tauri/               ← Tauri 2 Rust 层（commands/ + error.rs + AppState）
├── scripts/                 ← 工具脚本（hooks/pre-commit 双工具同步检查）
└── wiki/                    ← 项目知识库（架构 / 决策 / 流程）
```

详细架构 / 关键资产 → `wiki/architecture/overview.md`。

---

## 开发环境

- **Node + Rust**：`npm install` 装前端依赖;Rust 用系统 toolchain(无需 conda/Python 环境)
- **无端口**：纯 Tauri 单进程,前端与 Rust 通过 invoke 直接通信(无 HTTP / 无 sidecar 端口)

技术红线（Tauri v2 setup 时序坑 / SQLite PRAGMA）详见
`wiki/architecture/constraints.md`——**开发前必读**。

---

## 核心原则（启动时必须知道，不需要查文档）

1. **后端三原则**：稳定 > 架构 > 维护性 > 短期成本（所有后端决策依据）
2. **调查先行**：开工前必有规划文档，调研结果必须立刻持久化到 wiki/
3. **严格按规划 + 动态校准**：执行中不调整，每个切换节点重新调研
4. **主动暴露错误**：发现自己写错立刻修正，加「⚠️ 重要更正」段，不删原文
5. **实测发现为准**：决策前重新跑测试 / 类型检查 / lint，不靠内存中的旧结论
6. **关键决策 spot check**：5 类决策（架构变更 / 跨模块影响 / 不可逆操作 / 新依赖引入 / 安全相关）即使已授权也必须 spot check
7. **🔒 模块化 · 高内聚低耦合(最高优先级工程纪律)**：所有代码必须按职责拆成
   独立模块,模块边界清晰、单向依赖、互不渗透。**这是大模型开发时代的复杂度控制
   根基**——AI 写代码快,但速度快会放大耦合风险;模块化让每个单元独立可理解、可排查、
   可单独修改,是保证 AI 长期可控的关键。具体落地见 §8「文件组织」+ 工程原则完整版。

8. **🔒 随写随测(最高优先级工程纪律)**：完成一块代码,对应的单元测试**必须同步编写**,
   写完 + 测完 + 通过 才是一个完整闭环。**绝不积压到全部写完才测**——大模型一次写
   太多再测,出错时来不及定位。每完成一个模块/函数,立即补测试并跑绿,再进下一块。
   不好测的系统交互(系统剪贴板/网络)也要把可测的决策逻辑抽成纯函数覆盖。详见 §1.22。

完整 22 条通用原则 → `wiki/architecture/engineering-principles.md`。

---

## 知识查询决策树

| 你想做什么 | 用什么工具 |
|-----------|-----------|
| 查工程纪律：修 bug / 重构 / 新功能 | `wiki/architecture/engineering-principles.md`（20 条）|
| 查约束：技术红线（Tauri setup 时序坑 / SQLite）| `wiki/architecture/constraints.md` |
| 查架构：进程模型 / 分层 / 关键资产 | `wiki/architecture/overview.md` |
| 查进度：当前 TODO / 历史 | `wiki/dev/TODO.md` + `wiki/dev/progress.md` |
| 跨会话恢复 | `wiki/dev/new-session-starter.md` |
| 查当前任务规划（M0/M1+）| `wiki/dev/m0-plan.md` |

---

## 工作规则

- **每次完成阶段性工作时 commit**——commit 前用 `wiki/process/code-review-checklist.md` 自检
- **开发新模块前**先读 `wiki/index.md` 找对应设计文档
- **写/改 wiki 内容前**先读 `wiki/process/knowledge-base-guide.md`（防膨胀）
- **改 AGENTS.md 后必须同步 CLAUDE.md**（除 §8 受保护内容，详见 `wiki/process/dual-tool-sync.md`）
- **进度追踪**：每完成一个任务项立即更新 `wiki/dev/progress.md` + `wiki/dev/TODO.md`
- **改动生效验证**（改完代码主动让服务加载新代码，不等用户提醒）：改前端 `.ts/.tsx` → 提示用户刷新浏览器；改 `src-tauri/` → 告知需重跑 tauri dev
- **会话结束时**（识别到结束信号即启动）：
  - ⚠️ **第一件事：读 `wiki/dev/session-checklist.md` 按清单收尾**
  - 触发词：「存档」「下次继续」「换个新窗口」「今天先到这」「收尾」「结束」等
  - 收尾完成后**主动向用户报告核查结果**

---

_最后更新：2026-08-06（新增核心原则 7 模块化 + 8 随写随测 + §1.21/§1.22 完整纪律）_
