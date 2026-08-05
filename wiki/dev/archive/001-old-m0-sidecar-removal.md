---
title: 闲时任务交接文档 — M0 架构清理（已归档·被新规划替代）
tags: [闲时任务, 交接, M0, 架构清理, sidecar-removal, 归档]
created: 2026-08-05
updated: 2026-08-05
status: archived-superseded
superseded_by: ../m0-plan.md
---

# 闲时任务交接文档 — M0 架构清理

> ⚠️ **本文档已于 2026-08-05 归档,被 [[../m0-plan]] 替代。**
>
> **归档原因**:用户重新明确了产品定位(轻量常驻的剪贴板历史工具),
> 决定采用更激进的精简策略(换 UI 栈、删启动守卫、Spotlight 窗口形态等),
> 超出本文档「保留 wiki sidecar 方法论的温和迁移」范围。新规划见 `../m0-plan.md`。
>
> **本文档保留作为历史轨迹**(遵循 engineering-principles §1.4 不删原文原则)。
> 下方内容是归档时的原始版本,不再代表当前任务。

> **本文档是闲时任务的入口**。如果你是被闲时任务唤起的 AI,请**先完整读完本文件**,
> 再按「必读文档清单」读相关 wiki,然后开始执行「M0 任务」。
>
> **来源**:本任务由模板工程(`tauri-sidecar-template`)的会话规划,
> 在新工程创建后交接给闲时任务执行。

---

## 一、项目背景(你是谁,在干什么)

**本项目是什么**:Clipboard Manager —— 一个剪贴板历史管理桌面应用。

**技术定位**:纯 Tauri 2 + React 18 + Rust 原生后端(无 Python sidecar)。

**当前状态**:本工程刚从母体模板 `tauri-sidecar-template`(路径:`/Users/baijunjie/Documents/infra/tauri-sidecar-template`)
通过 `scripts/init-project.sh` 复制创建。**当前仍是模板的 sidecar 架构**(Tauri + Python FastAPI sidecar 双进程),
尚未改造。你的任务就是执行 M0:把架构从 sidecar 改造成纯 Rust。

**为什么不用 Python sidecar**:剪贴板应用是轻量常驻工具,不需要 Python 生态的重量级库。
单进程(Tauri + Rust)架构更简洁:无端口、无 spawn、无 PyInstaller 打包链、启动更快。

---

## 二、M0 任务:架构清理(sidecar → 纯 Rust)

### 2.1 任务目标

把本工程从「Tauri + Python sidecar 双进程」改造成「纯 Tauri + Rust 单进程」架构。
**M0 只做架构清理,不含任何剪贴板功能**(剪贴板监听是 M1+ 的事)。

### 2.2 完成判据(M0 做完了的标志)

全部满足才算 M0 完成:

- [ ] **`backend/` 整个目录已删除**
- [ ] **`src-tauri/src/sidecar.rs` 已删除**
- [ ] **`scripts/build-sidecar.sh` 已删除**
- [ ] **`resources/bin/sidecar` 二进制产物已清理**(保留 `.gitkeep`)
- [ ] **前端通信层已从 axios HTTP 改为 Tauri invoke**(`src/core/api/client.ts` → 基于 invoke 的封装)
- [ ] **启动守卫已改造**:`useSidecarReady`(HTTP 轮询)→ `useBackendReady`(invoke `ping` 探活)
- [ ] **存在一个可工作的 `ping` command**:Rust 侧有 `#[tauri::command] ping()`,前端能 invoke 并收到响应
- [ ] **应用能启动**:`npm run tauri dev` 不报错,前端能调通 `ping` 并显示「后端就绪」
- [ ] **端口 9880 相关代码全部清理**(三处真理源 + 文档引用)
- [ ] **`npm run build`(前端)能通过**
- [ ] **`cargo check`(src-tauri)能通过**
- [ ] **前端测试能通过**:`npm test`(改 invoke 后的通信层测试要同步更新)

### 2.3 必读文档(开工前必读,按顺序)

| 顺序 | 文档 | 为什么要读 |
|------|------|----------|
| 1 | `AGENTS.md` | 本工程的 AI 规则手册,特别是 §8 通用工程纪律 |
| 2 | `wiki/architecture/engineering-principles.md` | 20 条通用工程纪律(不可变性/错误处理/文件组织等) |
| 3 | `wiki/architecture/constraints.md` | 技术红线,**特别是「Tauri v2 setup 时序坑」节**(M0 必踩) |
| 4 | **`wiki/process/sidecar-to-rust-migration-guide.md`** | ⭐ **M0 的施工蓝图**。迁移触点清单表 + ping 探活改造方案 + tokio features 收紧 + 删 sidecar 留档要求 |
| 5 | `wiki/architecture/overview.md` | 当前架构理解(改造前的状态) |

### 2.4 施工步骤(基于迁移蓝图)

> 详细的「动哪些文件」见 `sidecar-to-rust-migration-guide.md` §二迁移触点清单表。
> 这里给出执行顺序和关键注意点。

**阶段 A:删除(先删后改,避免编译错误干扰)**

1. 删除 `backend/` 整个目录
2. 删除 `src-tauri/src/sidecar.rs`
3. 删除 `scripts/build-sidecar.sh`
4. 清理 `resources/bin/sidecar` 二进制(保留 `.gitkeep` 和目录)

**阶段 B:Rust 侧改造**

5. 改 `src-tauri/src/lib.rs`:
   - 删 `mod sidecar;` 及全部 `use sidecar::{...}`
   - 删 setup 里的 `spawn_sidecar` / `check_health`
   - 删三个 sidecar command 注册(`start_sidecar`/`stop_sidecar`/`check_sidecar_health`)
   - 删 `RunEvent::Exit` 里的 `cleanup_sidecar`
   - **新增** `ping` command(返回简单标识,供前端探活)
6. 改 `src-tauri/Cargo.toml`:
   - 删 `reqwest`(sidecar 健康检查专用)
   - 删 `dirs`(sidecar 数据目录用,改用 Tauri `app_data_dir()`)
   - tokio 删 `process` feature,**保留** `rt-multi-thread`/`macros`/`sync`/`time`
   - 视情况删 `thiserror`(如果新 error.rs 不用)
7. 新增 `src-tauri/src/error.rs`(AppError 错误契约,见蓝图 §四)

**阶段 C:前端通信层改造**

8. 改 `src/core/constants.ts`:删 `SIDECAR_PORT` / `SIDECAR_BASE_URL`
9. 改写 `src/core/api/client.ts` → 基于 `@tauri-apps/api/core` invoke 的封装(见蓝图 §四 `invoke.ts`)
10. 改 `src/core/api/useSidecarReady.ts` → `useBackendReady`(invoke `ping` 探活,带 100ms 重试 + 5s 超时)
11. 同步更新对应测试文件(`*.test.ts`)
12. 改 `src/App.tsx` / `SetupGate.tsx`:启动守卫从 HTTP 轮询改为 invoke 探活
13. 改 `src/pages/HomePage.tsx`:items 演示调用改为 invoke(或先清空成 placeholder,M1 再填剪贴板业务)

**阶段 D:配置/脚本清理**

14. 改 `src-tauri/tauri.conf.json`:删 `bundle.resources` 里的 `../resources/bin/` 映射
15. 改 `scripts/init-project.sh`:删 L134 的 sidecar exclude、L242-253 的 Python 环境引导段
16. 改 `scripts/README.md`:删 build-sidecar 相关说明
17. 改 `.env.example`:删 `SIDECAR_DEV_PYTHON` 等 sidecar 相关变量
18. 改 `.gitignore`:删 sidecar.pid / venv / backend/build / PyInstaller 产物规则

**阶段 E:文档同步**

19. 改 `AGENTS.md` L35-37(目录结构)、L49(三处对齐段落)、L89(改动生效验证里的「重启 sidecar」)、技术栈行
20. **同步 `CLAUDE.md`**(AGENTS.md 改动除 §8 外必须同步,见 `wiki/process/dual-tool-sync.md`)
21. 改 `README.md`:删 curl 示例(L73-77)+ 安装/运行段
22. 改 `wiki/architecture/constraints.md`:9880 三处对齐段(整个端口契约失效)
23. 改 `wiki/architecture/overview.md`:架构图含 :9880 的部分

**阶段 F:删前留档(回流素材,蓝图 §5.4 要求)**

24. 在 `wiki/dev/` 新建 `001-sidecar-removal-plan.md`,记录:
    - 被删的 sidecar.rs / paths.py / crypto.py 的方法论摘要(三重兜底进程清理 / frozen 路径 / Fernet 加密)
    - 这些方法论对本工程已失效,但**模板母体仍在用**,留档作为回流模板的素材

### 2.5 ⚠️ 必踩坑提醒(蓝图反复强调)

1. **Tauri v2 setup 时序坑**(蓝图 §三 + constraints.md):
   - ❌ **不要**在 setup 里 `emit("backend://ready")`——这永远收不到(setup 早于 WebView JS 执行,emit 即发即弃无缓冲)
   - ✅ setup 里只 `app.manage(AppState)`,**不 emit**
   - ✅ 前端用 invoke `ping` 探活(本身就是就绪信号)

2. **文件 IO 用 spawn_blocking**(蓝图 §5.2):
   - ❌ handler 内不要直接同步文件 IO(卡主线程)
   - ✅ `tokio::task::spawn_blocking(move || std::fs::write(...)).await??`

3. **tokio features**(蓝图 §5.1):
   - 删 `process`,保留 `rt-multi-thread`/`macros`/`sync`/`time`
   - ❌ 不要引入 `fs`(constraints 禁止,文件 IO 走 spawn_blocking)

4. **不可变性**(AGENTS.md §8 最高优先级):
   - Rust 侧 ownership/move 天然符合;前端 React state 必须展开运算符更新,绝不 mutate

5. **错误处理**(AGENTS.md §8):
   - 绝不静默吞错(无空 `except:` / 空 `catch` / 无 `unwrap()` 裸用)
   - Rust 侧用 `Result<T, AppError>`,前端镜像错误码

### 2.6 验证流程(自检,做完必跑)

```bash
cd /Users/baijunjie/Documents/infra/clipboard-manager

# 1. 前端构建
npm install   # 如果还没装依赖
npm run build

# 2. Rust 检查
cd src-tauri && cargo check && cd ..

# 3. 前端测试
npm test

# 4. 启动验证(关键)
npm run tauri dev
# 观察日志:应用能启动,前端能调通 ping 并显示「后端就绪」
# 手动验证:DevTools Console 里 invoke('ping') 应返回成功
```

**如果 cargo check 报 reqwest/dirs 找不到的残留引用** → 说明没删干净,grep 残留引用并清理。
**如果前端报 SIDECAR_BASE_URL 未定义** → 说明 constants.ts / client.ts 改造不完整。

### 2.7 红线:什么不要做

- ❌ **不要动模板母体**(`/Users/baijunjie/Documents/infra/tauri-sidecar-template`)。本工程是独立子工程。
- ❌ **不要在 M0 加剪贴板功能**(监听/历史/搜索都是 M1+,见 §三路线图)
- ❌ **不要保留任何 sidecar 残留**(端口、HTTP、Python 引用必须彻底清)
- ❌ **不要跳过「删前留档」**(蓝图 §5.4 要求,是回流模板的素材)

---

## 三、后续路线图(供参考,M0 不用做)

| 里程碑 | 目标 | 完成判据 | 备注 |
|------|------|--------|------|
| **M0** | 架构清理(sidecar→Rust) | 见本文档 §2.2 | ← **你现在做这个** |
| M1 | 剪贴板监听 MVP | 复制文字→历史列表出现新条目 | 涉及跨平台 API(arboard) |
| M2 | 历史 UI + 检索 | 搜索/点击回填 | SQLite FTS5 |
| M3 | 常驻体验 | 托盘+自启+全局快捷键 | 平台差异,可能需人工介入 |
| M4 | 生产化 | 打包 dmg/exe | 去重/容量上限/敏感过滤 |

---

## 四、给闲时任务 AI 的指令

如果你是被闲时任务唤起的 AI:

1. **先读**「§2.3 必读文档」列出的 5 份 wiki(按顺序)
2. **按**「§2.4 施工步骤」执行 M0
3. **注意**「§2.5 必踩坑提醒」
4. **做完跑**「§2.6 验证流程」自检
5. **每完成一个阶段 commit**(遵循 AGENTS.md「每次完成阶段性工作时 commit」)
6. **遵守**「§2.7 红线」
7. **完成后更新** `wiki/dev/progress.md`(记录 M0 完成状态)和 `wiki/dev/TODO.md`(标记 M0 完成,下一个目标 M1)

### 关于进度追踪

- 当前进度/状态 → `wiki/dev/progress.md`
- TODO 清单 → `wiki/dev/TODO.md`
- 完成后务必更新这两个文件,让下次会话能接续

### 关于回流(AGENTS.md 要求)

本工程是模板母体的子工程。开发过程中如果发现「这块通用,模板母体也用得上」,
记入 `wiki/dev/REFLUX-CANDIDATES.md`,由用户决定是否回流模板。

---

_本文档在 M0 完成后可归档(移到 `wiki/dev/archive/` 或标记 status: done)。_
