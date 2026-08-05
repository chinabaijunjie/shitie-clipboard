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
3. **读本文件下方「核心原则 + 知识查询决策树 + §8 通用工程纪律」** — 不查文档也能记住的最低基线

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

完整 20 条通用原则 → `wiki/architecture/engineering-principles.md`。

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

### 8. 通用工程纪律（每次写代码都必须遵守）

> **🔒 ZCode 专属内容 · 受保护 · 禁止改动**
>
> **为什么需要保护**：ZCode 不支持用户级全局规则（与 Claude Code 的 `~/.claude/rules/` 不同），
> 这些通用工程纪律只能写进每个项目的 AGENTS.md 才能对 ZCode 生效。删除或弱化本节 = ZCode
> 在本项目中失去基础工程纪律保障。
>
> **受保护范围**：本节「### 8」标题起，到本节末尾，以及 Code Review 通用检查清单中标注
> 「ZCode 专属补充」的项。
>
> **修改规则**：
> - ❌ Claude Code 不得删除、弱化、合并或移动本节
> - ❌ 不得把本节内容同步到 CLAUDE.md（Claude Code 已通过全局 rules 覆盖，同步会造成重复）
> - ✅ 可以补充新的工程纪律条目到本节内
> - ✅ 可以调整具体规则内容（如 800 行上限改成 600 行），但必须保留规则本身

**不可变性（最高优先级）**
- 永远创建新对象，**绝不原地修改**。错误：`obj.field = x`；正确：`{...obj, field: x}`
- React state 尤其如此：用展开运算符 / 不可变更新，不要 mutate 后 setState

**文件组织**
- 多个小文件 > 少数大文件（高内聚低耦合）
- 单文件典型 200-400 行，**最大 800 行**；超出先考虑拆分
- 按功能/领域组织，不按类型组织

**错误处理**
- 每一层显式处理错误，**绝不静默吞错**（不要空 `except:` / 空 `catch`）
- UI 层给用户友好提示，后端记录完整上下文（logger，不是 print）

**输入校验**
- 所有系统边界（用户输入、API 响应、文件内容）**进入即校验**
- 优先用 schema 校验（Python: Pydantic；TS: Zod），快速失败

**代码质量自检（标记完成前）**
- [ ] 可读、命名清晰
- [ ] 函数 < 50 行
- [ ] 文件 < 800 行
- [ ] 嵌套不超过 4 层
- [ ] 错误已处理（无静默吞错）
- [ ] 无硬编码值（用常量/配置）
- [ ] 无原地修改

**文件复制安全（cp 命令）**
- 复制到可能已有文件的位置时，**必须加日期时间后缀**避免覆盖：`cp config.json "config_$(date +%Y%m%d-%H%M%S).json"`
- 例外：全新目录、用户明确要求覆盖、临时/构建产物

**Git 提交格式**
```
<type>: <description>

<可选 body>
```
类型：feat, fix, refactor, docs, test, chore, perf, ci, build

### Code Review ZCode 专属补充

> Claude Code 已通过 `~/.claude/rules/` 全局规则覆盖，**这些项只在 AGENTS.md 中保留**，永不同步到 CLAUDE.md。

- [ ] **不可变性**：没有原地修改对象/state
- [ ] **无硬编码密钥**：API key / token / 密码走环境变量或 config
- [ ] **文件规模**：无超过 800 行的文件，函数不超过 50 行
- [ ] **错误已处理**：无空 except/catch、无静默吞错

---

_最后更新：2026-08-05（M0 架构清理完成,定位改为剪贴板应用）_
