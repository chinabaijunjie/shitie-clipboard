# Changelog

> 本文件分两部分:
> - **上方**:本工程(clipboard-manager)自己的变更记录
> - **下方**(分割线后):模板母体 `tauri-sidecar-template` 的历史版本追溯(本工程出身参考,不再维护)

格式参考 [Keep a Changelog](https://keepachangelog.com/)，版本号遵循 [语义化版本](https://semver.org/)。

## [Unreleased] - 2026-08-05

### M0 架构清理 + Spotlight UI 骨架

本工程从模板 `tauri-sidecar-template` 复制创建后,执行的首次架构改造:
从「Tauri + Python sidecar 双进程模板」改造为「纯 Tauri 单进程剪贴板工具骨架」。

**删除**(sidecar 架构产物):
- `backend/` 整个目录(35 个 Python 文件:FastAPI + SQLAlchemy + alembic + tests)
- `src-tauri/src/sidecar.rs`(817 行三重兜底进程管理,方法论已归档)
- `scripts/build-sidecar.sh` + `scripts/init-project.sh`(模板母体专用脚本)
- 前端 axios / antd / @ant-design/icons / react-router-dom 依赖

**新增**(剪贴板工具骨架):
- `src-tauri/src/error.rs` — AppError 错误契约(`{code, message}` 序列化)
- `src-tauri/src/commands/` — ping 探活 command(M1+ 加 clipboard/storage)
- `src/components/SpotlightPanel.tsx` — Spotlight 风格主面板(invoke ping 探活)
- `src/core/api/invoke.ts` — Tauri invoke 通信层封装(call + InvokeError)
- `tailwind.config.js` + `src/index.css` — Tailwind + shadcn 主题变量
- `src/lib/utils.ts` — shadcn cn() helper

**改造**:
- Rust 侧:tokio features 收紧(删 process/fs/io-util)+ Spotlight 窗口配置(无边框/透明/置顶)
- 前端:换栈 Tailwind + shadcn/ui,删 SetupGate 启动守卫,删 styles 旧样式
- 文档:定位从「模板母体」改为「剪贴板应用」,回流机制归档

**验证**:cargo check 零 warning + cargo test ping 通过 + npm build 41 modules + npm test 7 passed

详见 `wiki/dev/m0-plan.md` + `wiki/dev/progress.md`。

---

## 以下为模板母体历史(本工程出身参考,不再维护)

> 本工程 2026-08-05 从模板 `tauri-sidecar-template` 复制创建。
> 下方是模板母体的版本变更记录,保留作为出身参考。本工程自己的变更记录在上方。

## [1.2.0] - 2026-07-03

### 新增版本 bump 规则 + 模板↔子工程同步锚点机制

完善回流机制的执行细节，让版本号真正可操作：

**新增规则**（`wiki/process/template-evolution-guide.md` 版本约定节扩充）
- bump 责任分工：MAJOR 用户触发 / MINOR+PATCH AI 主动 bump
- bump 触发表：回流新增→MINOR / 修正优化→PATCH / 大工程完成→MAJOR
- bump 必须同步 3 处：progress.md（版本号真相源）+ CHANGELOG.md（详情）+ log.md（结构变更）
- 版本号 = 同步锚点：子工程记录「基于模板 vX.Y.Z 起步」，对照模板当前版本判断该同步哪些回流条目

## [1.1.0] - 2026-07-03

### 回流 pixel-matrix Stage 5A（剥离 sidecar 改 Rust 原生后端）

来源：[pixel-matrix](https://github.com/) 子工程 Stage 5A（剥离 Python sidecar 改 Rust 原生后端）。
回流 commit：`3659169`。

**新增方法论**
- ⭐ `wiki/process/sidecar-to-rust-migration-guide.md`（236 行）— sidecar → Rust 原生后端迁移指南
  - 何时该迁移 / 不该迁移的决策原则
  - 迁移触点清单（Python/Rust/前端/打包链/错误契约/Cargo features/数据目录）
  - 启动守卫改造（Tauri v2 setup 时序坑，指针 constraints.md）
  - Tauri Command 错误契约（AppError 序列化）
  - 其他注意点（tokio features 收紧 / 文件 IO 用 spawn_blocking / 删 sidecar 留档）
- `wiki/architecture/constraints.md` 新增「Tauri v2 setup 时序坑」节
  - setup 内 emit 事件会被丢弃的根因（setup 早于 WebView JS）+ 修正方案（invoke 探活）+ 反模式
  - 官方文档 + 社区确认链接

**索引同步**
- `wiki/index.md` 速查表 + 分类导航新增入口

### 通用化处理

- 内容已通用化：剥离 pixel-matrix 业务（图片/Provider/Skill 等），只保留迁移方法论
- 时序坑段标注「当前模板用 HTTP 轮询不受影响，子工程剥离 sidecar 时必踩」
- 去重：migration-guide §三改为指针，权威定义集中在 constraints.md（避免双处维护）

## [1.0.0] - 2026-06-29

### 初始落地

从 [ai-audiobook](https://github.com/) 项目沉淀的通用工程模板，10 个 commit：

**代码骨架**
- Tauri 2 + React 18 + TS 工程骨架
- Python sidecar 进程管理（三重兜底 + 健康检查，⭐ S 级资产）
- FastAPI sidecar 骨架（health/status + core 分层 + alembic）
- 示例 CRUD（items）全链路 + 第一个 migration
- 前端 core 骨架（api client + 启动守卫 SetupGate）
- 示例 HomePage（调 /api/items 全链路）+ UI 组件
- 工程质量基线（ruff/mypy/pytest + conftest + 18 tests）
- PyInstaller sidecar 打包脚本（onefile，31M binary）

**方法论套件**
- ⭐ engineering-principles 20 条通用工程纪律（含 v4.0 新增 §1.17-§1.20 + 扩展 §1.5/§1.7/§1.10）
- ai-collaboration-style 协作风格 13 条 + 分块逐项决策
- session-checklist 会话收尾 8 步清单
- code-review-checklist 三套 review 清单
- knowledge-base-guide 知识库防膨胀规范
- dual-tool-sync 双工具同步契约 + pre-commit hook

**进化基础设施**
- template-evolution-guide 回流机制（母体进化核心）
- README 进化模式说明
- CHANGELOG 版本追溯

**验证**：tauri dev 起空窗 + sidecar health 200 + items CRUD 全链路 + 打包 binary 可运行。

### 通用化处理（从 ai-audiobook 迁移时的裁剪）

- 剔除所有有声书业务（TTS/LLM引擎/角色/音色/卡片/工作流/MPS约束/进程级拆分）
- sidecar.rs：CONDA_PYTHON 硬编码 → SIDECAR_DEV_PYTHON 环境变量；去 identifier 改名迁移 / MPS 注入
- errors.py：去业务错误码（NARRATOR/CHARACTER/VOICE 等 9 个），留通用错误码
- paths.py：去 CosyVoice/models/voices 业务路径，留 frozen 双模式核心
- design-tokens.css：紫罗兰/琥珀品牌色 → 通用中性色 + 蓝主色
- engineering-principles：去 AI 推理专属案例（§1.14 质量优先体验的有声书案例），换通用表述
- 保留最小 items CRUD 示例让模板跑得起来
