---
title: 回流候选池
tags: [回流, 候选池, 子工程]
created: 2026-06-29
updated: 2026-08-09
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

### [敏感信息正则过滤(内容级)] — 2026-08-09
- **内容**:M4 实现基于正则的敏感信息检测(API key/token/私钥),从 gitleaks 精选高置信度模式(固定前缀)。三层防御的第 3 层(前两层:UTI 标记/应用黑名单)。调研结论:业界无纯文本启发式(高熵检测误报被弃),gitleaks 正则是共识方案。
- **为什么像通用**:对应 reflux-signals「通用方案」—— 任何处理用户输入的桌面应用都可能需要敏感信息过滤,gitleaks 正则模式库 + OnceLock 懒初始化模式可复用
- **状态**:待评估

### [Tauri 2 跨平台 CI 打包(tauri-action)] — 2026-08-09
- **内容**:M4 写了 GitHub Actions workflow,用 tauri-apps/tauri-action 实现三平台打包(macos arm64/x86_64 + windows)。macos-latest 已是 ARM runner,双架构靠一个 runner 交叉编译。push tag v* 触发 + 草稿 release。
- **为什么像通用**:对应 reflux-signals「通用方案」—— 所有 Tauri 桌面应用都需要 CI 打包发布,这份 workflow 可直接复制(改 productName/identifier 即可)
- **状态**:待评估

---

## 已处理（追溯用）

<!-- 处理完的条目从「待评估」移到这里，标记最终去向 -->

### [FTS5 中文 bigram + migration 版本号 + bundled 含 FTS5 发现] — 2026-08-07
- **内容**:M2 实现搜索时发现三件可能通用的事:
  1. rusqlite `bundled` feature 已无条件编译 FTS5(libsqlite3-sys build.rs),无需加任何 feature 即可用 `CREATE VIRTUAL TABLE USING fts5`
  2. FTS5 中文搜索方案:unicode61 + 应用层 bigram 预处理(30 行纯函数)+ LIKE 单字兜底,零依赖零 unsafe
  3. SQLite migration 手写 `PRAGMA user_version`(ALTER TABLE 不幂等需版本号守护,且整段要包事务)
- **为什么像通用**:对应 reflux-signals「通用技术坑/方案」—— 任何需要本地搜索的 Tauri/Rust 桌面应用都会遇到 FTS5 中文 + migration,模板母体若加这些约束可帮到其他子工程
- **状态**:丢弃(母工程评估后放弃,剪贴板专用性太强)

### [Tauri 2 常驻应用三件套:窗口隐藏 guard + tray + 进程常驻] — 2026-08-08
- **内容**:M3 实现常驻体验时踩的三个 Tauri 2 坑:
  1. **hide() 触发 Focused(false) 连锁**:Tauri 2 已知行为,程序主动 hide() 会派发失焦事件,若失焦 handler 也 hide() 会连锁抖动。解法:全局 AtomicBool guard(hide 前置位,Focused(false) 检查跳过)。三条 hide 路径(快捷键/close 拦截/失焦)必须统一走一个函数管 guard
  2. **末窗口关闭 → 进程退出**:Tauri 默认最后窗口销毁就退出。常驻应用必须在 `.run()` 回调里拦 `RunEvent::ExitRequested { api, .. } => api.prevent_exit()`。退出只能走 `app.exit(0)`(绕过拦截)
  3. **close 拦截转 hide**:`CloseRequested { api, .. } => api.prevent_close()` + 转 hide,否则点关闭按钮直接退出
- **为什么像通用**:对应 reflux-signals「通用技术坑」—— 任何 Tauri 2 常驻后台应用(托盘类)都会遇到完全相同的三个问题,模板母体若把这些写进 constraints.md 可帮到所有子工程
- **状态**:已回流(母工程已采纳)

### [tauri-plugin-store 做配置持久化] — 2026-08-08
- **内容**:M3 用 tauri-plugin-store(settings.json in app_data_dir)做用户配置持久化,与 rusqlite(业务数据)职责分离。Store 插件 Rust+JS 双端,自动管路径,比自写 JSON 文件省心,比 DB 开表轻量。配合 serde `#[serde(default)]` 做向后兼容(老 settings.json 缺字段用默认值)
- **为什么像通用**:对应 reflux-signals「通用方案」—— 所有 Tauri 桌面应用都有「用户偏好设置」需求,Store 插件是官方推荐方案
- **状态**:已回流(母工程已采纳)

### [快捷键录入用 e.code 物理键位 而非 e.key 字符] — 2026-08-08
- **内容**:M3 设置面板快捷键录入器,最初用 KeyboardEvent.key(字符),中文输入法下 e.key 变特殊字符(如 √),Tauri global-shortcut 底层 global-hotkey crate 不认识报错。改用 e.code(物理键位 KeyV)解决。需做 codeToTauriKey 映射(字母 KeyA-KeyZ / 数字 Digit0-Digit9 / 标点 Slash 等)
- **为什么像通用**:对应 reflux-signals「通用技术坑」—— 任何涉及全局快捷键配置的 Tauri 应用,只要用户可能开输入法就会遇到
- **状态**:丢弃(母工程评估后放弃)
