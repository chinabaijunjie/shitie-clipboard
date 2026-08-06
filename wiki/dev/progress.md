---
title: 开发进度（状态唯一真相源）
tags: [进度, 状态]
created: 2026-06-29
updated: 2026-08-06
status: active
---

# 开发进度

> **📌 当前状态（唯一真相源）**：版本号 / 测试数 / lint 只在本段维护，别处放指针。

## 当前状态

- **项目**：Clipboard Manager（轻量常驻的剪贴板历史管理桌面应用）
- **出身**：2026-08-05 从模板 `tauri-sidecar-template` 复制创建（commit `ac02fed`）
- **当前里程碑**：M1 macOS 剪贴板监听 MVP — **✅ 完成**(2026-08-06,端到端实测 6 项全通过)
- **架构现状**：纯 Tauri 2 单进程(Rust 既是壳也是后端),无 sidecar / 无 HTTP
- **技术栈**：Tauri 2(Rust) + React 18 + Tailwind + shadcn/ui + zustand
  + clipboard-rs 0.3(剪贴板读写/监听)+ rusqlite 0.31(存储,bundled)
- **验证基线**：cargo test 27 passed + npm test 20 passed + npm build 43 modules,全绿零 warning
- **下一步**：M2 搜索 + 富文本/图片支持(SQLite FTS5 + 多格式);M3 常驻体验(快捷键唤起/托盘/自启/Windows)

## M1 调研 + Spike 记录(2026-08-05)

- **技术调研**(见 [[m1-research]]):三块并行调研
  - 监听机制:macOS 无原生通知,只能轮询 changeCount(500ms 共识值)
  - crate 选型:**clipboard-rs**(用户决策:功能全 + 后期可 fork;放弃 arboard 因能力受限)
  - 多格式存储:items + formats 一对多两表(照搬 Maccy/Ditto),图片混合存储(Raycast 方案)
- **Spike 验证**(见 [[m1-research]] 第四节):
  - 🔴 核心验证:**clipboard-rs watcher 在 Tauri 后台线程不触发 NSPasteboard 崩溃**(5 次实测通过)
  - → M1 监听架构定:后台线程 + clipboard-rs watcher,无需 dispatch 主线程
  - 实测发现:`get_text()` 无文本格式时报错(文件场景),正式实现需 `available_formats()` 容错
- **M1 规划落盘**(见 [[m1-plan]]):5 阶段(存储/监听/命令/UI/实测)
- **Spike 代码**:`spike_clipboard.rs` + `SpikeClipboardProbe.tsx` 临时验证,阶段 B 演化删除

## M1 实施完成记录(2026-08-06)

5 阶段施工,全链路打通:复制文字 → 监听捕获 → 存 DB → 列表展示 → 上下键 → 回车回填。

- **阶段 A**(`1c41df7`):rusqlite 存储层(items/formats 两表 + CRUD + 15 单测)
  - 决策调整(动态校准):migration 用手写 `CREATE TABLE IF NOT EXISTS`,不引 refinery(过重)
  - 踩坑:`execute_batch` 的反斜杠续行让 SQL `--` 注释吞掉建表语句,改 raw string 修复
- **阶段 B**(`08e230b`):剪贴板监听层(clipboard-rs watcher 后台线程 → DB → emit 前端)
  - 模块化:`clipboard/mod.rs`(生命周期)+ `handler.rs`(捕获逻辑),平台无关
  - watcher 线程内开独立 DB 连接(Connection 非 Send;WAL 模式并发安全)
  - 删 spike 临时代码,演化成正式模块
- **随写随测纪律落地**(`63ac4ea`):用户强调最高优先级工程纪律,补 handler 7 单测 + 写进核心原则 8 / §1.22
- **阶段 C**(`2c40f58`):command 接口(list_items / copy_back / delete_item)+ 错误码两端同步
  - 两层结构:核心逻辑函数(接 &Connection,可测)+ command 包装(接 State)
- **阶段 D**(`edbe2b0`):前端历史列表 UI(types/hook/ClipboardList/SpotlightPanel)
  - 键盘交互:↑↓ 循环导航 / ⏎ 回填 / ⌫ 删除
  - 踩坑:vite/tsconfig/vitest 三处别名要同步加 @hooks(vitest 有独立 config 不读 vite)
- **阶段 E**:端到端实测 6 项全通过(复制实时更新 / 导航循环 / 回车回填 / 去重 ×N / 删除 / 中文代码)

**M1 期间确立的两条核心工程纪律**(用户强调,已进跨会话必读):
- 核心原则 7 / §1.21:模块化·高内聚低耦合(大模型开发复杂度控制根基)
- 核心原则 8 / §1.22:随写随测(写完+测完+通过才算闭环)

**M1 遗留 → M3 决策**(用户确认):
- 窗口常驻问题(visible:true)→ M3 做全局快捷键唤起/隐藏
- 交互方案:唤起 + 回车回填(主流方案,Maccy/Flycut 同款)
- ⚠️ 「松手回填」做不到:Tauri 全局快捷键只支持 keydown 不支持 keyup([文档](https://v2.tauri.app/reference/javascript/global-shortcut/)),要松手检测需 OS hook(rdev/Carbon)+ 触发辅助功能权限,工程量大,M3 不做

## 本工程开发历史

### M0 架构清理 + Spotlight UI 骨架（2026-08-05）

8 阶段施工,从「Tauri + Python sidecar 双进程模板」改造为「纯 Rust 单进程剪贴板工具骨架」:

- **阶段 A**(`06f1892`):归档旧 M0 规划 + 落盘新 M0 规划 + sidecar 方法论删前留档
- **阶段 B**(`93da773`):删除 backend/ + sidecar.rs + build-sidecar.sh(3409 行)
- **阶段 C**(`3d30b48`):Rust 侧改造(error.rs + commands/ping + Spotlight 窗口配置)
  - 踩坑:transparent:true 必须配合 Cargo.toml tauri features=["macos-private-api"]
  - 踩坑:generate_handler! 宏对模块路径解析有限制,改 use 别名裸名注册
- **阶段 D+E**(`4a03c33`):前端换栈 Tailwind+shadcn,删 axios/AntD/路由,重建 invoke 通信层
  - 踩坑:CSS 文件 JSDoc 风格多行注释(含 `bg-*`)让 PostCSS parser 报错,精简单行注释
- **阶段 F**(`ff3b92d`):清理配置/脚本/环境变量(.env.example + init-project.sh + .gitignore)
- **阶段 G**(`29af37f`):文档同步(AGENTS/CLAUDE/README + wiki 架构文档,定位改为剪贴板应用)
- **阶段 H**(`7a3bb7a`):启动实测验证,修复探活竞态 bug
  - 实测发现:静态检查全过但 GUI 实测暴露真实 bug(§1.5 价值)
  - 根因:React StrictMode 双渲染 + 成功路径漏 clearTimeout(deadline),ping 成功后超时 setTimeout 覆盖了 ready
  - 修复:ping 成功后立即 clearTimeout(deadline)
  - 验证:UI 正确显示「● 后端就绪」,Console 零错误

**关键技术决策**(产品定位明确后重新规划):
- UI:Tailwind + shadcn/ui(换掉 AntD,轻量 + 键盘导航友好)
- 窗口:Spotlight 风格(无边框/透明/置顶/失焦隐藏,600×480)
- 剪贴板(M1+):平台原生 API(macOS NSPasteboard / Windows Win32)
- 监听(M1+):原生事件(changeCount / AddClipboardFormatListener)
- 存储(M1+):rusqlite + refinery
- 前端通信:Tauri invoke(换掉 axios,无 HTTP)
- 路由:删除 react-router-dom(单窗口不需要)

### 工程初始化（2026-08-05）

- 从模板 `tauri-sidecar-template` 通过 `scripts/init-project.sh` 复制创建
- 占位符替换：tauri-sidecar-template → clipboard-manager（19 个文件）
- git init + 首次 commit `ac02fed`
- 规划 M0 架构清理任务

---

## 以下为模板母体历史（本工程出身参考，不再维护）

> 本工程从模板复制，模板的开发历史保留如下供追溯。本工程自己的进度在上方维护。

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
