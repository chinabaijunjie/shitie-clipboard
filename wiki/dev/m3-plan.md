---
title: M3 规划 — 常驻体验(全局快捷键/托盘/自启/设置/清理)
tags: [M3, 规划, global-shortcut, tray, autostart, settings, cleanup]
created: 2026-08-08
updated: 2026-08-08
status: done
tech_base: ./m3-research.md
---

# M3 规划:常驻体验

> 技术依据见 [[m3-research]](四块调研 + 官方文档确认,均已完成)。
> 本文档是 M3 施工蓝图,实现阶段严格按此执行(§核心原则 3)。
>
> **用户决策**(2026-08-08):
> - 配置 7 项:自启 + 快捷键 + 启动时打开窗口 + 历史上限 + 保留天数 + 容量上限 + 失焦隐藏
> - 交互:回车回填后隐藏窗口 + 移到最前 / Esc 隐藏 / 失焦隐藏(可配)/ 关窗拦截转隐藏
> - 配置持久化:tauri-plugin-store(settings.json),与 rusqlite(历史数据)职责分离

---

## 一、M3 范围边界

**M3 目标闭环**:
1. 全局快捷键唤起/隐藏窗口(任何 app 前台时)→ ↑↓ 导航 → Enter 回填 → 窗口自动隐藏 + 选中项移到最前
2. 系统托盘常驻 + 开机自启(可选)
3. 设置面板:7 项配置可视化修改,即时生效
4. 自动清理:保留天数 + 容量上限,过限自动删旧

**M3 做**:
- **全局快捷键**:tauri-plugin-global-shortcut(toggle show/hide,用户可配)
- **窗口控制**:close 拦截转 hide / 失焦隐藏(可配)/ Esc 隐藏 / 回填后隐藏
- **进程常驻**:RunEvent::ExitRequested prevent_exit + 托盘退出
- **托盘**:tray-icon feature + TrayIconBuilder(左键 toggle / 右键菜单)
- **开机自启**:tauri-plugin-autostart(LaunchAgent,--hidden)
- **设置持久化**:tauri-plugin-store(settings.json)
- **回填增强**:copy_back 更新 last_copied_at → 列表前置 + 隐藏窗口
- **自动清理**:保留天数 + 容量上限(cleanup 模块)
- **设置面板 UI**:7 项配置 + 视图切换

**M3 不做**(留给后续里程碑):
- ❌ Windows 平台补全(M3 聚焦 macOS,M4+ 再扩)
- ❌ 去重优化 / 敏感过滤(M4)
- ❌ 多设备同步(远期)

**判据**:
1. 在 Safari/Word 等任意 app 前台时按 ⌘⇧V → 窗口唤起 → ↑↓ 导航 → Enter → 回填成功 + 窗口隐藏 + 该条移到最前
2. 失焦自动隐藏(可配开关)
3. 托盘左键 toggle / 右键菜单(显示/退出)
4. 设置面板改快捷键 → 新快捷键即时生效
5. 开关自启 → macOS 系统设置登录项出现/消失
6. 保留天数 7 天 → 造 8 天前数据 → 触发清理 → 自动删除 + 图片文件清理
7. 容量上限 50MB → 超 50MB → 从最旧开始删到达标

---

## 二、架构设计

### M3 在 M2 架构上的增量

```
┌──────────────────────────────────────────────────────────────┐
│  Tauri 主进程(Rust)                                         │
│                                                              │
│  ┌──────────────────┐    ┌──────────────────────────────┐  │
│  │ clipboard watcher│    │ AppState                     │  │
│  │ (500ms 轮询)     │    │  - db: Mutex<Conn>           │  │
│  │                  │    │  - self_write_flag           │  │
│  │ on_clipboard_    │    │  - watcher                   │  │
│  │   change         │    │  - images_dir                │  │
│  │  ④ emit 前端     │    │  ┌────────────────────────┐  │  │
│  │  ⑤ cleanup       │    │  │ settings: Mutex<Settings> │  │  ← M3 新增
│  │   (保留/容量)    │    │  └────────────────────────┘  │  │
│  └──────────────────┘    └──────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────┐              │
│  │ window_control(新)                       │              │
│  │  toggle_window / hide_main / show_main   │              │
│  │  SUPPRESS_BLUR_HIDE guard(AtomicBool)    │              │
│  └──────────────────────────────────────────┘              │
│                                                              │
│  ┌────────────────────┐  ┌────────────────────────────┐    │
│  │ tray-icon(新)     │  │ global-shortcut(新)        │    │
│  │ 左键 toggle        │  │ ⌘⇧V → toggle_window       │    │
│  │ 右键 显示/退出     │  │ handler 在 plugin Builder  │    │
│  └────────────────────┘  └────────────────────────────┘    │
│                                                              │
│  ┌──────────────────────────────────────────┐              │
│  │ commands(新增 get/update_settings)       │              │
│  │  list / copy_back / delete / search      │              │
│  │  get_settings(新) / update_settings(新)  │              │
│  └──────────────────────────────────────────┘              │
│                                                              │
│  on_window_event: Focused(false)→hide + CloseRequested→hide │
│  .run(): ExitRequested→prevent_exit                         │
│                                                              │
│  tauri-plugin-store(settings.json in app_data_dir)          │
│  tauri-plugin-autostart(LaunchAgent, --hidden)              │
└──────────────────────────────────────────────────────────────┘
              │                           │
              ▼                           ▼
┌──────────────────────────┐  ┌─────────────────────────────┐
│  WebView(React)         │  │  settings.json              │
│                          │  │  (app_data_dir)             │
│  App.tsx                 │  │                             │
│   view: main|settings ←  │  │  shortcut: "Cmd+Shift+V"   │
│                          │  │  launchVisible: true        │
│  SpotlightPanel(主)      │  │  maxItems: 50               │
│   齿轮按钮 → settings    │  │  retentionDays: 0           │
│   Enter → copyBack +     │  │  maxStorageMb: 0            │
│          前置 + hide     │  │  hideOnBlur: true           │
│   Esc → hide / 清空搜索  │  │  autostart: false (插件管)  │
│                          │  └─────────────────────────────┘
│  SettingsPanel(新)       │
│   7 项配置 + 即时保存    │
└──────────────────────────┘
```

### 关键架构决策(均来自 m3-research)

1. **配置存 settings.json**:tauri-plugin-store,与 rusqlite(历史数据)职责分离。setup 早期读取(与 init_db 同模式,毫秒级)
2. **窗口隐藏 guard**:`SUPPRESS_BLUR_HIDE: AtomicBool`,程序主动 hide 前置位,Focused(false) 检查跳过。与 `self_write_flag` 同套路
3. **三条 hide 路径统一走 `hide_main_window()`**:快捷键 toggle / close 拦截 / 失焦,内部统一管 guard
4. **快捷键 handler 在 plugin Builder 注册**:Tauri 2 的 register() 不再接 handler(与 v1 最大区别)
5. **托盘左键 toggle / 右键菜单**:`show_menu_on_left_click(false)`
6. **自启用 LaunchAgent**:无权限弹窗,系统设置登录项可见
7. **回填后移到最前**:copy_back 里 UPDATE last_copied_at,前端手动前置(不依赖 watcher 事件)
8. **自动清理在 watcher on_clipboard_change 后触发**:每次新内容入库后检查是否超限

### 数据流(M3 增量)

**唤起/隐藏流**:
1. 全局快捷键 ⌘⇧V → handler → `toggle_window`(is_visible ? hide : show+focus)

**回填流(增强)**:
1. Enter → `invoke('copy_back', {id})` → mark_self_write + set 剪贴板 + **UPDATE last_copied_at**(M3 新)
2. 前端成功后:手动前置该 item + hideWindow()

**设置流**:
1. 用户在 SettingsPanel 改配置 → `invoke('update_settings', {patch})`
2. 后端:Store 写 settings.json + AppState 更新 + 联动(改快捷键→unregister旧/register新,改自启→enable/disable)

**自动清理流**:
1. watcher on_clipboard_change → 写 DB 后 → 检查 retention_days / max_storage_mb → 超限删旧 + 图片引用计数

---

## 三、数据模型

### Settings struct(Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub shortcut: String,           // "Cmd+Shift+V"
    pub launch_visible: bool,       // true
    pub max_items: i64,             // 50
    pub retention_days: i64,        // 0 = 不限
    pub max_storage_mb: i64,        // 0 = 不限
    pub hide_on_blur: bool,         // true
}
```

> autostart 状态由 `tauri-plugin-autostart` 插件管理(写 LaunchAgent plist),不进 settings.json。
> SettingsPanel 读取 autostart 状态时直接调 `isEnabled()`,不走 get_settings。

### 前端类型(models.ts)

```ts
export interface Settings {
  readonly shortcut: string
  readonly launchVisible: boolean
  readonly maxItems: number
  readonly retentionDays: number
  readonly maxStorageMb: number
  readonly hideOnBlur: boolean
}
```

### 数据库变更

**无 schema 变更**。copy_back 更新 `last_copied_at` 用现有列(已有 idx_items_last_copied 索引)。清理用现有 `clipboard_items` + `clipboard_formats` + 图片引用计数。

---

## 四、Command 接口(M3 增量)

| command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `get_settings` | — | `Settings` | **M3 新增**:读当前设置 |
| `update_settings` | `Settings`(完整) | `Settings` | **M3 新增**:写设置 + 联动(快捷键 unregister/register,自启 enable/disable) |
| `copy_back` | `id` | `String` | M2 既有,**增加 UPDATE last_copied_at** |

**错误码**(两端镜像):
- `SETTINGS_LOAD_FAILED`(M3 新增)
- `SETTINGS_UPDATE_FAILED`(M3 新增)
- `SHORTCUT_REGISTER_FAILED`(M3 新增)

---

## 五、交互逻辑(M3 增量)

| 触发 | 行为 |
|------|------|
| 全局快捷键 ⌘⇧V | toggle 窗口(show+focus / hide) |
| Enter(回填) | copy_back → 更新 last_copied_at → 列表前置 → **隐藏窗口** |
| Esc | 有 searchText → 清空搜索 / 无 searchText → 隐藏窗口 |
| 失焦 | 隐藏窗口(hide_on_blur=true 时;可配) |
| 关窗按钮 | 拦截 close → 转 hide(不退出进程) |
| 托盘左键 | toggle 窗口 |
| 托盘右键菜单 | 「显示」/「退出」 |

---

## 六、施工阶段(每阶段一个 commit,完成后汇报确认)

### 阶段 A:设置持久化层(后端 + 前端类型)

**目标**:settings.json 读写基础设施,不接 UI。

1. `Cargo.toml`:加 3 个 plugin 依赖 + tauri features 加 `tray-icon`
2. `package.json`:加 3 个前端 plugin 依赖
3. `capabilities/default.json`:追加 global-shortcut / autostart / store 权限
4. `src-tauri/src/settings/mod.rs`(新):Settings struct + 默认值 + load/save(Store 插件)
5. `src-tauri/src/commands/settings.rs`(新):get_settings / update_settings command
6. `error.rs` + 前端 `invoke.ts`:加 SETTINGS_LOAD_FAILED / SETTINGS_UPDATE_FAILED / SHORTCUT_REGISTER_FAILED
7. `lib.rs`:setup 加载 store 插件 + 读初始设置 → AppState 加 `settings: Mutex<Settings>`
8. `src/core/settings/models.ts`(新):SettingsSchema(Zod)+ TypeScript 接口
9. **验证**:`cargo test`(Settings 默认值 + 序列化 + 部分更新)+ `npm run test:typecheck`
10. **commit**:`feat(settings): 设置持久化层(Settings 模块 + command + 类型)`

### 阶段 B:全局快捷键 + 窗口控制

**目标**:⌘⇧V 唤起/隐藏,close 拦截,失焦隐藏。

1. `src-tauri/src/window_control.rs`(新):toggle_window / hide_main_window / show_main_window + SUPPRESS_BLUR_HIDE guard
2. `lib.rs` setup:注册 global-shortcut 插件 + with_handler(toggle)+ register(快捷键)
3. `lib.rs` `.on_window_event`:Focused(false)→ hide(带 guard + 检查 hide_on_blur)+ CloseRequested → prevent_close + hide
4. `lib.rs` `.run()`:ExitRequested → prevent_exit
5. `tauri.conf.json`:`visible: false`(默认隐藏,由快捷键/设置驱动)
6. **验证**:`cargo test`(toggle 逻辑纯函数)+ GUI 实测(⌘⇧V 唤起/隐藏 / 关窗拦截 / 失焦隐藏)
7. **commit**:`feat(window): 全局快捷键 + 窗口控制(toggle/hide/失焦隐藏)`

### 阶段 C:托盘 + 开机自启

**目标**:系统托盘常驻 + 开机自启。

1. `lib.rs` setup:TrayIconBuilder + 菜单(显示/退出)+ 左键 toggle / 右键菜单
2. `lib.rs` setup:autostart 插件 init(LaunchAgent, --hidden)
3. `lib.rs`:启动时检查 args,--hidden 则不 show 窗口
4. update_settings 联动:自启开关 → enable/disable;快捷键 → unregister/register
5. **验证**:GUI 实测(托盘左键 toggle / 右键菜单 / 自启开关联动系统设置)
6. **commit**:`feat(tray): 系统托盘 + 开机自启`

### 阶段 D:回填增强(移到最前 + 隐藏窗口)

**目标**:Enter 回填后 → 移到最前 → 隐藏窗口。

1. `commands/clipboard.rs`:copy_back_with_conn 加 UPDATE last_copied_at
2. `src/core/window.ts`(新):hideWindow() 封装
3. `SpotlightPanel.tsx`:handleCopyBack 成功后 → 前置 item + hideWindow()
4. `SpotlightPanel.tsx`:Esc 逻辑(有搜索→清空 / 无搜索→hide)
5. `hooks/useClipboardItems.ts`:加 moveToFront action(回填后前置,不可变更新)
6. **验证**:`cargo test`(copy_back 更新 last_copied_at)+ `npm test`(moveToFront 纯函数)+ GUI 实测
7. **commit**:`feat(clipboard): 回填后移到最前 + 隐藏窗口`

### 阶段 E:设置面板 UI

**目标**:7 项配置可视化。

1. `src/components/SettingsPanel.tsx`(新):7 项配置表单 + 即时保存
2. `src/hooks/useSettings.ts`(新):get_settings 加载 + update_settings 更新
3. `App.tsx`:view state('main' | 'settings')+ 视图切换
4. `SpotlightPanel.tsx`:顶栏加齿轮按钮 → 切设置页
5. 快捷键录入器:监听 keydown 录入 → 格式化 → 调 update_settings
6. **验证**:`npm test`(表单校验纯函数)+ GUI 实测(改各项 → 即时生效)
7. **commit**:`feat(ui): 设置面板(7 项配置 + 视图切换)`

### 阶段 F:自动清理 + 端到端实测

**目标**:保留天数 + 容量上限 + 全链路 GUI 实测 + PR review。

1. `src-tauri/src/storage/cleanup.rs`(新):cleanup_by_retention / cleanup_by_capacity / total_storage_size
2. `clipboard/handler.rs`:on_clipboard_change 后触发 cleanup
3. cleanup 联动图片引用计数删除(复用 image_store)
4. **验证**:`cargo test`(cleanup 纯函数)+ GUI 实测全链路(见判据 7 项)
5. `zcode-pr-review` Skill 跑深度 review
6. **commit**:`feat(cleanup): 保留天数 + 容量上限自动清理`

---

## 七、必踩坑提醒(M3 执行红线)

1. **hide() 触发 Focused(false) → 连锁抖动**(tauri #13511)→ 三条 hide 路径统一走 hide_main_window(),SUPPRESS_BLUR_HIDE guard
2. **Tauri 2 register() 不接 handler** → handler 在 plugin Builder 的 with_handler 传入(与 v1 最大区别)
3. **末窗口关闭→进程退出** → RunEvent::ExitRequested prevent_exit(常驻必备)
4. **自启不要 setup 无条件 enable** → 由用户设置面板驱动
5. **快捷键改键先 unregister 旧再 register 新** → 避免冲突
6. **copy_back 更新 last_copied_at 不加 copy_count** → copy_count 反映「被复制次数」,回填不是新复制
7. **mark_self_write 时序**:在 set 之前置位,hide 不影响 watcher 独立线程
8. **Settings 不可变更新**:Rust 侧 clone + 替换字段,前端展开运算符

---

## 八、红线:什么不做

- ❌ Windows 平台补全(M4+)
- ❌ 去重优化 / 敏感过滤(M4)
- ❌ 多设备同步(远期)
- ❌ UI 苹果审美改造(独立任务,见 [[ui-redesign-plan]])

---

## 九、验证流程

```bash
cd /Users/baijunjie/Documents/infra/clipboard-manager

# 每阶段末
cd src-tauri && cargo test && cd ..
npm run build && npm run test:typecheck && npm test

# 阶段 B/C/D/E/F 末(GUI 实测)
npm run tauri dev
```

---

## 十、执行模式

- 每阶段完成后 **commit + 汇报**,等用户确认再进下一阶段
- 阶段内自主执行,不再逐文件确认
- 遇到未预见的决策点停下问用户
- GUI 实测阶段请用户配合(按快捷键/改设置等)

---

_本文档 M3 完成后标记 `status: done`,关键决策沉淀到 constraints.md / decision-cases.md。_
