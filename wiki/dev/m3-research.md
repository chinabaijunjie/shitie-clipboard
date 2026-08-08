---
title: M3 技术调研 — 常驻体验(全局快捷键/托盘/自启/设置)
tags: [M3, 调研, global-shortcut, tray, autostart, store]
created: 2026-08-08
updated: 2026-08-08
status: done
---

# M3 技术调研:常驻体验

> 三块并行调研 + 官方文档/docs.rs 源码确认,所有 API 均验证真实可用。

---

## 一、tauri-plugin-global-shortcut(全局快捷键)

### 1.1 依赖

```toml
# Cargo.toml
tauri-plugin-global-shortcut = "2"
```
```json
// package.json
"@tauri-apps/plugin-global-shortcut": "^2"
```

### 1.2 Tauri 2 关键变化(与 v1 最大区别)

`register()` **不再接收 handler 参数**(plugins-workspace issue #965)。handler 必须在插件初始化时通过 `Builder::new().with_handler(...)` 传入,setup 里只做 `register(快捷键)`。

### 1.3 Rust 端骨架

```rust
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// setup 内:
let toggle = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);

app.handle().plugin(
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if shortcut == &toggle && event.state() == ShortcutState::Pressed {
                toggle_window(app);
            }
        })
        .build(),
)?;
app.global_shortcut().register(toggle)?;
```

- `Modifiers` 是 bitflags:`CONTROL` / `SHIFT` / `ALT` / `SUPER`(macOS ⌘)
- `Code` 枚举:`KeyA`..`KeyZ`、`Digit0`..`Digit9`、`F1`..`F12`
- `ShortcutState`:`Pressed` / `Released`(项目只用 Pressed)

### 1.4 JS 端:动态注册/注销(用户改快捷键时)

```ts
import { register, unregister, isRegistered } from '@tauri-apps/plugin-global-shortcut'
await unregister('Cmd+Shift+V')
await register('Cmd+Shift+Q', (event) => { /* ... */ })
```

快捷键字符串格式:`modifier+modifier+Key`,`+` 分隔。合法 modifier:`CommandOrControl` / `Cmd` / `Control` / `Shift` / `Alt` / `Super`。

### 1.5 capabilities 追加

```json
"global-shortcut:allow-register",
"global-shortcut:allow-unregister",
"global-shortcut:allow-is-registered"
```

### 1.6 项目约束

- 全局快捷键只支持 keydown 不支持 keyup → 不做「松手回填」(progress.md M1 已记)
- 快捷键绑定放 Rust setup(随开机注册最可靠);用户改键走 JS register/unregister

---

## 二、tauri-plugin-autostart(开机自启)

### 2.1 依赖

```toml
tauri-plugin-autostart = "2"
```
```json
"@tauri-apps/plugin-autostart": "^2"
```

### 2.2 macOS 实现机制

`MacosLauncher` 枚举两个变体,`LaunchAgent` 是 `#[default]`:
- **LaunchAgent** — 写 `~/Library/LaunchAgents/<bundle-id>.plist`,无权限弹窗,推荐
- **AppleScript** — 走 System Events,会弹自动化权限,不推荐

### 2.3 Rust 端

```rust
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

// setup 内:
app.handle().plugin(tauri_plugin_autostart::init(
    MacosLauncher::LaunchAgent,
    Some(vec!["--hidden"]),  // 自启时隐藏窗口
))?;
// 不在 setup 里强制 enable —— 自启由用户设置开关驱动
```

`app.autolaunch()` manager 有 `enable()/disable()/is_enabled()`。

### 2.4 JS 端

```ts
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
await enable()
await disable()
const on = await isEnabled()  // Promise<boolean>
```

### 2.5 capabilities 追加

```json
"autostart:allow-enable",
"autostart:allow-disable",
"autostart:allow-is-enabled"
```

---

## 三、tray-icon(Tauri 2 内置托盘)

### 3.1 feature 启用

```toml
tauri = { version = "2.11.2", features = ["macos-private-api", "protocol-asset", "tray-icon"] }
```

### 3.2 Rust 端(Tauri 2 API,与 v1 完全不同)

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

// setup 内:
let show_i = MenuItem::with_id(app, "show", "显示剪贴板", true, None::<&str>)?;
let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
let menu  = Menu::with_items(app, &[&show_i, &quit_i])?;

let _tray = TrayIconBuilder::with_id("main-tray")
    .icon(app.default_window_icon().unwrap().clone())
    .tooltip("Clipboard Manager")
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(|app, event| match event.id.as_ref() {
        "show" => show_main_window(app),
        "quit" => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up, ..
        } = event
        {
            toggle_window(tray.app_handle());
        }
    })
    .build(app)?;
```

- 纯 Rust,不需要额外 capability
- 左键 toggle 窗口(`show_menu_on_left_click(false)`),右键出菜单(系统默认)

---

## 四、tauri-plugin-store(配置持久化)

### 4.1 选型决策

| 方案 | 契合度 | 理由 |
|------|--------|------|
| **tauri-plugin-store** ✅ | 高 | 轻量 key-value 偏好,JSON 文件放 app_data_dir,Rust+JS 双端 |
| 自写 JSON | 中 | 可控但要自己管路径+原子写 |
| rusqlite | 低 | 配置项少(几个 bool/string),开表成本不划算 |

历史数据继续走 rusqlite,M3 配置走 settings.json(Store 插件)。职责分离。

### 4.2 依赖

```toml
tauri-plugin-store = "2"
```
```json
"@tauri-apps/plugin-store": "^2"
```

### 4.3 Rust 端

```rust
use tauri_plugin_store::StoreExt;

// setup 内:
app.handle().plugin(tauri_plugin_store::Builder::default().build())?;
let store = app.store("settings.json")?;
let shortcut: String = store.get("shortcut")
    .map(|v| serde_json::from_value(v).unwrap_or_default())
    .unwrap_or_else(|| "Cmd+Shift+V".into());
store.set("shortcut", serde_json::json!(shortcut));
// set 不会自动落盘,需 store.save()
```

### 4.4 JS 端

```ts
import { load } from '@tauri-apps/plugin-store'
const store = await load('settings.json', { autoSave: 100 })
await store.set('shortcut', 'Cmd+Shift+Q')
```

### 4.5 capabilities 追加

```json
"store:default"
```

### 4.6 约束冲突检查

constraints.md「文件 IO 用 spawn_blocking」针对 command handler 内同步 IO。Store 插件内部自管 IO,setup 闭包同步读配置是可接受的(毫秒级,与 init_db 同模式)。**不违反约束**。

---

## 五、窗口控制 API

### 5.1 Rust 端

```rust
use tauri::Manager;

fn toggle_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}
```

### 5.2 失焦隐藏 + close 拦截

```rust
.on_window_event(|window, event| {
    match event {
        WindowEvent::Focused(false) => {
            // 需 guard 防 hide→Focused(false) 连锁(见下)
            let _ = window.hide();
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        _ => {}
    }
})
```

### 5.3 防止「最后一个窗口关掉 → 进程退出」

```rust
.run(|_app_handle, event| match event {
    tauri::RunEvent::ExitRequested { api, .. } => {
        api.prevent_exit();  // 常驻
    }
    _ => {}
});
```

### 5.4 🔴 关键陷阱:hide() 触发 Focused(false) → 连锁抖动

tauri #13511 官方明确。三条 hide 路径(快捷键/close 拦截/失焦)互相干扰。

**解决方案(AtomicBool guard)**:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
static SUPPRESS_BLUR_HIDE: AtomicBool = AtomicBool::new(false);

// Focused(false):
if !SUPPRESS_BLUR_HIDE.load(Ordering::SeqCst) {
    let _ = window.hide();
}

// 程序主动 hide 前:
SUPPRESS_BLUR_HIDE.store(true, Ordering::SeqCst);
let _ = window.hide();

// show 时复位:
SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
```

与项目现有 `self_write_flag`(防 copy_back 反馈循环)同一套原子标志套路。三条 hide 路径统一走 `hide_main_window()` 函数。

---

## 六、JS 端窗口控制

```ts
import { getCurrentWindow } from '@tauri-apps/api/window'
const win = getCurrentWindow()
await win.hide()
await win.show()
await win.setFocus()
await win.isVisible()
```

窗口 hide/show/set-focus 权限已在 capabilities/default.json 中(M1 加的),无需追加。

---

## 七、现有代码基础(已调查确认)

| 能力 | 现状 | M3 接入点 |
|------|------|-----------|
| 窗口隐藏/显示 | ❌ 无,visible:true 常驻 | on_window_event + JS getCurrentWindow |
| 全局快捷键 | ❌ 只有窗口内 DOM keydown | global-shortcut 插件 + register |
| 失焦隐藏 | ❌ 无 on_window_event | Focused(false) + guard |
| 托盘 | ❌ 零实现,feature 未启用 | tray-icon feature + TrayIconBuilder |
| 开机自启 | ❌ 零实现 | autostart 插件 |
| 回填后隐藏 | ❌ handleCopyBack 成功后什么都不做 | await copy_back → hideWindow |
| 回填后移到最前 | ❌ copy_back 不更新 last_copied_at | UPDATE last_copied_at + 前端前置 |
| 应用退出 vs 隐藏 | ❌ 关窗=退出 | close_requested prevent_close→hide |
| 进程常驻 | ❌ 末窗口关→退出 | RunEvent::ExitRequested prevent_exit |
| 设置持久化 | ❌ 无 | Store 插件 settings.json |

预留挂钩点:`lib.rs:135-138` 的 `.run()` 回调空实现,注释已标注「未来若加托盘/全局快捷键,在此处理 RunEvent::Exit」。

---

## 八、参考来源

- [Global Shortcut — Tauri v2 官方指南](https://v2.tauri.app/plugin/global-shortcut/)
- [Autostart — Tauri v2 官方指南](https://v2.tauri.app/plugin/autostart/)
- [System Tray — Tauri v2 官方指南](https://v2.tauri.app/learn/system-tray/)
- [Store — Tauri v2 官方指南](https://v2.tauri.app/plugin/store/)
- [tauri #13511:prevent_close + hide 的 Focused(false) 副作用](https://github.com/tauri-apps/tauri/issues/13511)
- [plugins-workspace #965:v2 register 不再接 handler](https://github.com/tauri-apps/plugins-workspace/issues/965)
