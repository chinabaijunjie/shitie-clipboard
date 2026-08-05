---
title: M0 规划 — 架构清理 + Spotlight UI 骨架
tags: [M0, 规划, 架构清理, Spotlight, Tailwind, shadcn, sidecar-removal]
created: 2026-08-05
updated: 2026-08-05
status: active
supersedes: ./archive/001-old-m0-sidecar-removal.md
---

# M0 规划:架构清理 + Spotlight UI 骨架

> **本文档替代旧的 `idle-task-context.md`(已归档至 `archive/001-old-m0-sidecar-removal.md`)。**
>
> **方向变更**:从「保留 wiki 大量 sidecar 方法论的温和迁移」改为
> 「激进精简成轻量常驻剪贴板工具」。变更原因见归档文档顶部说明。

---

## 一、产品定位(为什么这么改)

**Clipboard Manager = 轻量常驻的剪贴板历史管理工具**,不是带后端服务的应用。

核心交互三要素:
1. **被动监听**:系统复制时原生事件捕获 → 存记录
   (macOS 用 NSPasteboard.changeCount,Windows 用 AddClipboardFormatListener)
2. **常驻 + 呼出**:后台运行,全局快捷键呼出 Spotlight 风格窗口
3. **历史回填**:上下键/点击选中 → 写入当前剪贴板 → Ctrl+V 粘贴选中的内容

**架构含义**:Rust 既是壳也是后端,无 Python、无 HTTP、无独立后端服务。
前端用 Tailwind+shadcn(轻、键盘交互友好)。窗口是 Spotlight 形态
(小、无边框、失焦隐藏)。

---

## 二、M0 范围边界(已与用户确认)

**M0 只做:架构清理 + UI 骨架重置。不含任何剪贴板功能(M1+)。**

判据:本次规划做完后,应用能启动 → 显示一个空的 Spotlight 风格占位窗口 →
ping command 可调通 → 全部测试/构建/检查通过。

---

## 三、技术栈定型(已与用户确认)

| 层 | 选型 | 理由 |
|----|------|------|
| 桌面壳 | Tauri 2(Rust) | 保留 |
| Rust 剪贴板(M1+) | **平台原生 API**(macOS NSPasteboard / Windows Win32) | 性能/体验最优;M0 不实现,Rust 侧目录结构预留 |
| Rust 监听(M1+) | **原生事件**(changeCount / AddClipboardFormatListener) | 零延迟;M0 不实现 |
| 存储(M1+) | **rusqlite + refinery** | 轻量内嵌;M0 不实现 |
| 前端构建 | **Vite**(保留) | 现状成熟 |
| 前端 UI | **Tailwind + shadcn/ui**(换掉 AntD) | 轻、键盘导航原生支持 |
| 前端状态 | zustand(保留) | 全局状态用得上 |
| 前端通信 | `@tauri-apps/api` invoke(换掉 axios) | 无 HTTP |
| 前端路由 | **删除 react-router-dom** | 单窗口不需要 |

---

## 四、资产处置清单(逐项明确)

### ✂️ 删除(不贴合)

| 资产 | 处置 |
|------|------|
| `backend/` 整个目录 | 删(27 个 Python 文件) |
| `src-tauri/src/sidecar.rs`(32KB 三重兜底) | 删 |
| `scripts/build-sidecar.sh` | 删 |
| `src-tauri/src/lib.rs` 里的 sidecar 相关全部 | 删(spawn/check_health/cleanup/三个 command 注册/`mod sidecar`) |
| `reqwest` / `dirs` / tokio `process`+`fs`+`io-util` features | 删(Cargo.toml) |
| `axios`(package.json) | 删 |
| `antd` + `@ant-design/icons`(package.json) | 删 |
| `react-router-dom`(package.json) | 删 |
| `src/components/business/SplashScreen.tsx` | 删(启动太快不需要) |
| `src/components/business/SetupGate.tsx` | 删(Rust 即时启动,不需要守卫) |
| `src/components/ui/notify.ts`(AntD message 封装) | 删 |
| `src/core/api/client.ts`(axios 封装) | 删 |
| `src/core/api/types.ts`(ApiResponse/ApiErrorCode) | 删 |
| `src/core/api/useSidecarReady.ts` + `.test.ts` | 删 |
| `src/core/events/index.ts` 的 SIDECAR_* 事件常量 | 删(THEME_CHANGED 保留) |
| `src/core/types/models.ts` 的 Item 类型 | 删(剪贴板条目类型 M1 定义) |
| `src/pages/HomePage.tsx`(items CRUD 演示) | 删 |
| `.env.example` 里 `SIDECAR_DEV_PYTHON` 等 | 删 |
| `.gitignore` 里 sidecar.pid / venv / PyInstaller 产物规则 | 删 |
| `tauri.conf.json` 的 `bundle.resources ../resources/bin/` 映射 | 删 |
| `src/core/constants.ts` 的 SIDECAR_PORT/SIDECAR_BASE_URL | 删 |

### 🔧 改造

| 资产 | 改造为 |
|------|--------|
| `src-tauri/src/lib.rs` | 精简:setup 只 manage AppState + 注册 ping command;删全部 sidecar 痕迹 |
| `src-tauri/Cargo.toml` | 删 reqwest/dirs/thiserror,tokio 收紧到 `["sync","macros","rt-multi-thread","time"]`;description 改成剪贴板应用 |
| `src-tauri/tauri.conf.json` | 窗口改 Spotlight 形态:~600×480、`decorations:false`、`transparent:true`、`visible:false`(快捷键唤起,M1 加)、`alwaysOnTop:true`、`resizable:false`;删 titleBarStyle Overlay |
| `src-tauri/capabilities/default.json` | 保留 core:default + window 相关;M1 加 global-shortcut 权限 |
| `src/main.tsx` | 删 AntD App/ConfigProvider 包裹,改 Tailwind 入口 |
| `src/App.tsx` | 删 SetupGate,直接渲染主面板 |
| `src/index.css`(新增) | 引入 Tailwind 指令 + shadcn CSS 变量 |
| `src/core/api/` | 新建 `invoke.ts`(基于 `@tauri-apps/api/core` 的 call 封装 + InvokeError)+ `index.ts` 出口 |
| `src/core/constants.ts` | 精简:删 sidecar 常量,保留 APP_NAME/VERSION/STORAGE 前缀 |
| `src/core/storage/index.ts` | 保留(设置/主题用得上),但 STORAGE_KEYS.SIDECAR_PID 删 |

### ✅ 保留

- `src-tauri/src/main.rs`(入口,只调 lib::run)
- `src-tauri/build.rs`、icons、Cargo.lock
- `src/core/utils/`、`src/core/storage/`、`src/core/events/`(精简后)
- `vitest`/`vite`/`tsconfig`/`vitest.config.ts`/`tsconfig.test.json` 测试基建
- `tauri-plugin-log`(改 Warn 级,常驻工具日志有用但不要吵)

### 📦 新增

| 新增 | 用途 |
|------|------|
| `tailwind.config.js` + `postcss.config.js` | Tailwind 配置 |
| `src/index.css` | Tailwind 指令 + shadcn 变量 |
| `src/lib/utils.ts`(shadcn 标准 `cn` helper) | className 合并 |
| `src/components/ui/`(shadcn 组件目录) | M0 先放占位,具体组件 M1 按需加 |
| `src/components/SpotlightPanel.tsx` | Spotlight 风格占位面板(ping 调通 + 「等待 M1」) |
| `src-tauri/src/error.rs` | AppError 错误契约(M1 业务会用,M0 先建骨架) |
| `src-tauri/src/commands/mod.rs` + `commands/ping.rs` | command 分目录(M0 只有 ping) |

### 📝 文档同步

- `AGENTS.md`:技术栈行 / 目录结构 / 三处对齐段(删 9880) / 改动生效验证段(删重启 sidecar)
- `CLAUDE.md`:同步(除 §8 受保护内容)
- `README.md`:删 curl/PyInstaller 段,改产品定位为剪贴板工具
- `wiki/architecture/constraints.md`:删 9880 三处对齐 / 环境变量契约 / PyInstaller 三节(失效);
  保留 Tauri v2 setup 时序坑(仍然适用)
- `wiki/architecture/overview.md`:重写进程模型(单进程)/ 技术栈 / 关键资产
- `wiki/index.md`:更新当前任务指针指向本文档
- `wiki/dev/progress.md` + `wiki/dev/TODO.md`:重写,反映新 M0/M1 路线

---

## 五、施工阶段(每阶段一个 commit,完成后向用户汇报确认)

### 阶段 A:删前留档 + 旧规划归档(低风险)
1. ✅ 写 `wiki/dev/archive/sidecar-methodology-extract.md`(三重兜底/frozen/Fernet 摘要)
2. ✅ 移 `idle-task-context.md` → `archive/001-old-m0-sidecar-removal.md`
3. ✅ 写新 `wiki/dev/m0-plan.md`(本文档)
4. ⬜ 更新 `wiki/index.md` 指针
5. commit: `docs: 归档旧 M0 规划 + 落盘新 M0 规划(sidecar 方法论留档)`

### 阶段 B:删除 sidecar/Python/打包链(低风险,纯删)
6. 删 `backend/`
7. 删 `src-tauri/src/sidecar.rs`
8. 删 `scripts/build-sidecar.sh`
9. 删 `src-tauri/src/lib.rs` 的 sidecar 痕迹(此阶段 lib.rs 暂编译不过,阶段 C 修复)
10. commit: `chore: 删除 Python sidecar / 进程管理 / 打包链`

### 阶段 C:Rust 侧改造到能编译(中风险)
11. 改 `lib.rs`:删 sidecar,新增 `app.manage(AppState)` + ping command + 删 RunEvent cleanup
12. 新增 `src-tauri/src/error.rs`(AppError 骨架)
13. 新增 `src-tauri/src/commands/mod.rs` + `commands/ping.rs`
14. 改 `Cargo.toml`:删 reqwest/dirs/thiserror,tokio 收紧,description 改名
15. 改 `tauri.conf.json`:窗口改 Spotlight 形态
16. 改 `capabilities/default.json`:精简
17. **验证**:`cd src-tauri && cargo check` 通过
18. commit: `refactor(tauri): Rust 侧改造成剪贴板工具骨架(Spotlight 窗口 + ping)`

### 阶段 D:前端依赖换栈 + Tailwind 接入(中风险,与 E 合并 commit)
19. `npm uninstall axios antd @ant-design/icons react-router-dom`
20. `npm install -D tailwindcss postcss autoprefixer`;`npx tailwindcss init -p`
21. 装 shadcn 依赖:`clsx tailwind-merge class-variance-authority @radix-ui/react-slot`
22. 配 `tailwind.config.js` + 新建 `src/index.css`
23. 改 `src/main.tsx`:删 AntD 包裹,import index.css
24. 改 `vite.config.ts`:补 `/@lib` 路径别名(若缺)

### 阶段 E:前端源码清理 + 通信层重建(中风险)
25. 删 `HomePage.tsx` / `SetupGate.tsx` / `SplashScreen.tsx` / `ui/notify.ts` / `ui/index.ts`
26. 删 `api/client.ts` / `types.ts` / `useSidecarReady.ts` / `.test.ts`
27. 新建 `api/invoke.ts`(call 封装 + InvokeError)+ `api/index.ts`
28. 新建 `src/lib/utils.ts`(shadcn cn helper)
29. 改 `constants.ts`(删 sidecar)/ `events/index.ts`(删 SIDECAR_*)/ `types/models.ts`(删 Item)
30. 改 `App.tsx`:删 SetupGate,渲染 SpotlightPanel
31. 新建 `SpotlightPanel.tsx`:ping 调通 + 「等待 M1」占位
32. 新建 `components/ui/index.ts`:空出口占位
33. 新建 `invoke.test.ts`
34. **验证**:`npm run build` + `npm test` + `npm run test:typecheck` 全过
35. commit(与 D 合并): `refactor(frontend): 换栈 Tailwind+shadcn,删 axios/AntD/路由,重建 invoke 通信层`

### 阶段 F:配置/脚本/环境清理(低风险)
36. `.env.example`:删 sidecar 变量
37. `.gitignore`:删 sidecar.pid / venv / PyInstaller / backend/build 规则
38. `scripts/init-project.sh`:删 sidecar exclude + Python 环境引导段
39. `scripts/README.md`:删 build-sidecar 说明
40. commit: `chore: 清理 sidecar 相关配置/脚本/环境变量`

### 阶段 G:文档同步(低风险,但量大)
41. `AGENTS.md` / `CLAUDE.md`(除 §8) / `README.md`
42. `wiki/architecture/constraints.md`(删失效三节,保留 Tauri setup 时序坑)
43. `wiki/architecture/overview.md`(重写进程模型)
44. `wiki/dev/progress.md` / `wiki/dev/TODO.md`
45. commit: `docs: 同步 M0 架构清理至全部文档`

### 阶段 H:启动实测验证(关键,§1.5 实测为准)
46. `npm run tauri dev` 启动,观察:无报错 / Spotlight 形态 / invoke ping 成功 / 显示「后端就绪」
47. visible:false 导致看不到窗口时,临时改 visible:true 验证后改回
48. commit(如有调整): `fix: M0 启动验证修正`

---

## 六、必踩坑提醒(M0 执行红线)

1. **Tauri v2 setup 时序坑**(constraints.md 已沉淀):setup 里**不 emit** 任何事件;
   M0 直接在 SpotlightPanel mount 时 invoke ping 验证。
2. **tokio features**:删 `process`/`fs`/`io-util`,保留 `sync`/`macros`/`rt-multi-thread`/`time`。
   **禁止引 fs**(文件 IO 走 spawn_blocking)。
3. **不可变性**(AGENTS §8):前端 state 展开运算符更新;Rust ownership 天然符合。
4. **错误处理**(AGENTS §8):无空 catch、无裸 unwrap();Rust 用 `Result<T, AppError>`。
5. **Spotlight 窗口 visible:false**:M0 还没实现全局快捷键唤起,验证时需临时 visible:true。
   **M0 不实现快捷键**(M3),但窗口配置先按 Spotlight 形态定型。
6. **CLAUDE.md 同步**:AGENTS.md 改动除 §8 外必须同步(见 dual-tool-sync)。

---

## 七、红线:什么不做

- ❌ 不动模板母体(`/Users/baijunjie/Documents/infra/tauri-sidecar-template`)
- ❌ M0 不加任何剪贴板功能(监听/存储/回填都是 M1+)
- ❌ M0 不实现全局快捷键(M3)和托盘(M3)
- ❌ 不保留任何 sidecar 残留(端口/HTTP/Python 引用彻底清)
- ❌ 不跳过删前留档(母体仍需 sidecar 方法论,见 `archive/sidecar-methodology-extract.md`)

---

## 八、后续路线图(M0 完成后)

| 里程碑 | 目标 | 备注 |
|--------|------|------|
| **M0** | 架构清理 + Spotlight UI 骨架 | ← 本规划 |
| M1 | macOS 剪贴板监听 MVP | NSPasteboard.changeCount 事件 + rusqlite 存储 + 历史列表 + 回填。
 **先做 macOS,Windows 在 M1 后期或 M2** |
| M2 | 搜索 + 富文本/图片支持 | SQLite FTS5 + 多格式 |
| M3 | 常驻体验 | 全局快捷键唤起 + 托盘 + 自启 + Windows 平台补全 |
| M4 | 生产化 | 打包 dmg/exe,去重/容量上限/敏感过滤 |

---

## 九、验证流程(自检,做完必跑)

```bash
cd /Users/baijunjie/Documents/infra/clipboard-manager

# 1. Rust 检查(阶段 C 末尾)
cd src-tauri && cargo check && cd ..

# 2. 前端构建 + 类型检查 + 测试(阶段 E 末尾)
npm run build
npm run test:typecheck
npm test

# 3. 启动实测(阶段 H,关键)
npm run tauri dev
# DevTools Console: await window.__TAURI__.core.invoke('ping') → 应返回 "ok"
```

---

## 十、执行模式(已与用户确认)

- 每阶段完成后 **commit + 汇报**,等用户确认再进下一阶段
- 阶段内自主执行,不再逐文件确认
- 遇到未预见的决策点(如 shadcn 主题色、Spotlight 具体尺寸)停下问用户

---

_本文档 M0 完成后标记 `status: done`,并归档至 `archive/`。_
