---
title: macOS 全屏悬浮窗口实施方案
tags: [方案, macOS, NSPanel, tauri-nspanel, 全屏]
created: 2026-08-10
updated: 2026-08-10
status: active
---

# macOS 全屏悬浮窗口实施方案

> **前置**：根因调研见 [[fullscreen-research]]。已选定方案：tauri-nspanel 插件（isa-swizzle）。
> **参考**：EcoPaste（7283 star 同类剪贴板）的 `src-tauri/src/window/macos.rs` 已验证可行。
> 本方案精确到 API 调用 + 文件:行号改动，经源码核实（tauri-nspanel v2.1 + EcoPaste master）。

---

## §1 核心改造思路

主窗口在 setup 阶段从 `NSWindow`（tao 默认）转换成 `NSPanel` 子类（通过 `tauri-nspanel` 的
isa-swizzle），获得 `nonactivatingPanel` + `canBecomeKeyWindow` 能力，从而能悬浮在全屏应用之上。

**三关键点**（缺一不可，都在调研中确认）：
1. **转 NSPanel**：`WebviewWindow.to_panel::<MainPanel>()` — 让 `nonactivatingPanel` mask 生效
2. **不激活 app 的显示**：`panel.show_and_make_key()`（内部 `orderFrontRegardless`，不调 `activateIgnoringOtherApps`）— 替代当前 `set_focus()`（元凶）
3. **动态切 collectionBehavior**：show 时 `can_join_all_spaces`、hide 时 `move_to_active_space` — 让 panel 跟随当前 Space

---

## §2 文件改动清单

| 文件 | 改动 | 说明 |
|------|------|------|
| `src-tauri/Cargo.toml` | 删 `objc = "0.2"`，加 `tauri-nspanel` git 依赖 | objc 和 objc2 不兼容 |
| `src-tauri/src/macos_panel.rs` | **新建** | NSPanel 集成模块（高内聚）|
| `src-tauri/src/lib.rs` | 注册 plugin + 调 setup_panel + mod 声明 | 时序红线：plugin 必须在 to_panel 前 |
| `src-tauri/src/window_control.rs` | show/hide 走 panel 路径 + 删旧 setup + 改失焦分支 | 跨平台 #cfg 分支 |

---

## §3 详细改动

### 3.1 Cargo.toml

```toml
# 删除 line 48-50 的：
# # macOS 全屏 Space 支持:...
# [target.'cfg(target_os = "macos")'.dependencies]
# objc = "0.2"

# 替换为：
# macOS 全屏悬浮支持：tauri-nspanel 用 isa-swizzle 把 NSWindow 转成 NSPanel 子类，
# 获得 nonactivatingPanel + canBecomeKeyWindow 能力。
# 选型调研见 wiki/dev/fullscreen-research.md。只发 git 不发 crates.io。
# 必须放 macOS target 段（依赖 objc2-app-kit，Windows 编译失败）。
[target.'cfg(target_os = "macos")'.dependencies]
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2.1" }
```

⚠️ **objc crate 必须删**：`objc = "0.2"`（老 API `objc::runtime::Object`）和 tauri-nspanel 用的
`objc2`（`objc2::runtime::AnyClass`）是两套不兼容的绑定，不能共存。删除后 `window_control.rs`
里的旧 `setup_macos_fullscreen_window` 也要同步删（它用 objc crate）。

### 3.2 新建 `src-tauri/src/macos_panel.rs`

> 单一职责：NSPanel 转换 + 配置 + show/hide。window_control.rs 只管可见性决策，不管 NSPanel 细节。
> 符合工程纪律「模块边界清晰」（§1.21）。

```rust
//! macOS NSPanel 集成 —— 全屏 Space 悬浮支持。
//!
//! 主窗口在 setup 阶段从 NSWindow（tao 默认）转成 NSPanel 子类，
//! 获得 nonactivatingPanel + canBecomeKeyWindow 能力，能悬浮在全屏应用之上。
//!
//! 选型调研见 wiki/dev/fullscreen-research.md。
//! 参考实现：EcoPaste（7283 star 同类剪贴板）src-tauri/src/window/macos.rs。
//!
//! 模块化：本模块只管 NSPanel 转换/配置/show/hide，
//! 不管可见性决策（window_control.rs）/ 不管事件绑定（lib.rs）。

#![cfg(target_os = "macos")]

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

use crate::window_control::MAIN_WINDOW;

/// show 前延迟（等窗口几何应用完，避免闪烁。EcoPaste 经验值）。
const PANEL_SHOW_DELAY: Duration = Duration::from_millis(16);

/// 定义 NSPanel 子类 + 失焦 delegate。
///
/// - `can_become_key_window: true` — NSPanel 默认返回 NO，必须 override 成 true 才能接收键盘
///   （搜索框/列表导航需要）。这是 NSPanel 子类化的核心价值。
/// - `can_become_main_window: false` — 不成为 main window，app 激活状态不转移，
///   前台 app（全屏的那个）保持激活。
/// - `is_floating_panel: true` — 浮在普通窗口之上。
///
/// ⚠️ `tauri_panel!` 宏自动注入 NSNotification/NSWindow 等类型，块内直接用裸名即可。
tauri_panel! {
    panel!(MainPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })

    panel_event!(MainPanelEventHandler {
        // 失焦回调：panel 失去 key 状态时触发（用户点别的窗口/切 app）。
        // 替代 Tauri 的 Focused(false) 事件（NSPanel 下不可靠）。
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

/// setup 最早阶段：注册 plugin（⚠️ 必须在 to_panel 之前）。
///
/// `init()` 注册 WebviewPanelManager（全局面板表）。若晚于 to_panel，
/// `to_panel` 内部 `state::<WebviewPanelManager>()` 会 panic。
pub fn register_plugin<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.plugin(tauri_nspanel::init()) {
        log::error!("[panel] 注册 tauri-nspanel plugin 失败: {e}");
    }
}

/// setup 末尾：WebviewWindow → NSPanel + 配置 + 绑失焦回调。
pub fn setup_panel<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window(MAIN_WINDOW) else {
        log::warn!("[panel] 找不到主窗口，跳过 NSPanel 转换");
        return;
    };

    // ⭐ 核心步骤：isa-swizzle，把 TaoWindow(NSWindow) 转成 MainPanel(NSPanel)
    let panel = match win.to_panel::<MainPanel>() {
        Ok(p) => p,
        Err(e) => {
            log::error!("[panel] to_panel 失败: {e:?}");
            return;
        }
    };

    panel.set_level(PanelLevel::Dock.value()); // 20，高于普通窗口但不盖菜单栏
    panel.set_style_mask(
        // nonactivating_panel 是关键：不抢 app 焦点（仅对 NSPanel 子类生效）
        StyleMask::empty().resizable().nonactivating_panel().into(),
    );
    panel.set_collection_behavior(
        // 初始 hide 态：move_to_active_space（下次 show 跟到当前 space）
        CollectionBehavior::new()
            .stationary()
            .move_to_active_space()
            .full_screen_auxiliary() // ⭐ 允许在全屏 Space 显示
            .into(),
    );

    // 绑失焦回调（delegate 模式，替代 Tauri Focused(false)）
    let handler = MainPanelEventHandler::new();
    let resign_handle = app.clone();
    handler.window_did_resign_key(move |_notification| {
        // 复用 window_control 的统一 hide 入口（guard 一致管理）
        if crate::window_control::should_hide_on_blur(
            crate::window_control::read_hide_on_blur_pub(&resign_handle),
        ) {
            crate::window_control::hide_main_window(&resign_handle);
        }
    });
    panel.set_event_handler(Some(handler.as_ref()));

    log::info!("[panel] NSPanel 转换 + 配置完成（level=Dock, nonactivating, fullScreenAuxiliary）");
}

/// show panel（主线程 + 16ms 延迟 + 切 collectionBehavior 跟随当前 Space）。
///
/// ⚠️ 用 `show_and_make_key()`（内部 orderFrontRegardless，不激活 app），
/// 绝不能用原 `set_focus()`（底层 makeKeyAndOrderFront + activateIgnoringOtherApps:YES，
/// 会激活 app → 切到 app 的 Space → 全屏应用被挤走）。
pub fn show_panel<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    // 快捷键回调可能在非主线程；panel 操作必须在主线程
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PANEL_SHOW_DELAY).await;
        let panel_handle = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if let Ok(panel) = panel_handle.get_webview_panel(MAIN_WINDOW) {
                panel.show_and_make_key();
                // show 时切到 can_join_all_spaces：显示瞬间出现在用户当前 Space
                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .stationary()
                        .can_join_all_spaces()
                        .full_screen_auxiliary()
                        .into(),
                );
            } else {
                log::warn!("[panel] show: 找不到 panel（未 setup？）");
            }
        });
    });
}

/// hide panel（主线程 + 切回 move_to_active_space）。
pub fn hide_panel<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(MAIN_WINDOW) {
            panel.hide();
            // hide 后切回 move_to_active_space：下次 show 按当前 Space 落位
            panel.set_collection_behavior(
                CollectionBehavior::new()
                    .stationary()
                    .move_to_active_space()
                    .full_screen_auxiliary()
                    .into(),
            );
        }
    });
}
```

### 3.3 lib.rs 改动

**① mod 声明**（顶部其他 mod 附近）：
```rust
#[cfg(target_os = "macos")]
mod macos_panel;
```

**② setup 闭包最开头**（在托盘 / autostart / DB 初始化之前，紧跟在读完 user_settings 之后）：
```rust
// ⚠️ 时序红线：tauri-nspanel plugin 必须在 to_panel 之前注册。
// 放 setup 最开头确保优先级。调研见 fullscreen-research.md §3.2。
#[cfg(target_os = "macos")]
macos_panel::register_plugin(app.handle());
```

**③ 替换原 line 250-253**（setup 末尾，show_main_window 之前）：
```rust
// 删除原：
// #[cfg(target_os = "macos")]
// window_control::setup_macos_fullscreen_window(app.handle());

// 替换为：
// macOS: 主窗口转 NSPanel 子类（全屏悬浮支持）
#[cfg(target_os = "macos")]
macos_panel::setup_panel(app.handle());
```

### 3.4 window_control.rs 改动

**① 删除** `setup_macos_fullscreen_window`（line 100-171，整个函数）—— 被 `macos_panel::setup_panel` 取代。

**② `show_main_window`（line 87-98）改跨平台分支**：
```rust
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    {
        // macOS: 走 NSPanel 路径（show_and_make_key 不激活 app，全屏悬浮）
        crate::macos_panel::show_panel(app);
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
            if let Err(e) = win.show() {
                log::warn!("[window] show 失败: {e}");
            }
            if let Err(e) = win.set_focus() {
                log::warn!("[window] set_focus 失败: {e}");
            }
        }
    }
    // 清 guard：窗口已显示，后续失焦是真实用户操作
    SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
}
```

**③ `hide_main_window`（line 76-84）改跨平台分支**：
```rust
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    SUPPRESS_BLUR_HIDE.store(true, Ordering::SeqCst);
    #[cfg(target_os = "macos")]
    {
        crate::macos_panel::hide_panel(app);
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
            if let Err(e) = win.hide() {
                log::warn!("[window] hide 失败: {e}");
            }
        }
    }
}
```

**④ `handle_window_event` 的 Focused(false) 分支**（line 188-212）：
macOS 下 NSPanel 失焦走 delegate `window_did_resign_key`，Focused(false) 不再需要。
但 Windows 仍需要（拖动假失焦去抖）。

```rust
WindowEvent::Focused(false) => {
    #[cfg(not(target_os = "macos"))]
    {
        if !should_hide_on_blur(read_hide_on_blur(app)) {
            return;
        }
        // 原有 50ms 去抖逻辑（Windows 拖动假失焦过滤）...保持不变
        let app_handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if !should_hide_on_blur(read_hide_on_blur(&app_handle)) { return; }
            if let Some(win) = app_handle.get_webview_window(MAIN_WINDOW) {
                if !win.is_visible().unwrap_or(false) { return; }
                if win.is_focused().unwrap_or(false) { return; }
            }
            hide_main_window(&app_handle);
        });
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: NSPanel 走 delegate window_did_resign_key，这里不处理
        let _ = app;
    }
    #[allow(unused_variables)]
    {
        let _ = app;
    }
}
```

**⑤ 暴露 `read_hide_on_blur` 给 macos_panel 模块用**：
当前 `read_hide_on_blur` 是私有 `fn`，macos_panel 的 delegate 回调要调。
加一个 pub 别名（或直接把原函数改 pub）：

```rust
/// 从 AppState 读 hide_on_blur（pub 版本，供 macos_panel delegate 回调用）。
pub fn read_hide_on_blur_pub<R: Runtime>(app: &AppHandle<R>) -> bool {
    read_hide_on_blur(app)
}
```

> `should_hide_on_blur` 已经是 `pub`，不用改。

---

## §4 风险点 + 应对

| 风险 | 影响 | 应对 |
|------|------|------|
| **plugin 注册时序**（最高风险）| `to_panel` panic | `register_plugin` 放 setup 最开头，`setup_panel` 放末尾 |
| **主线程约束** | panel 操作必须在主线程 | show/hide 用 `run_on_main_thread` 包裹（已设计）|
| **objc/objc2 冲突** | 编译失败 | 删 `objc = "0.2"`，旧 setup 函数同步删 |
| **Focused + delegate 双重触发** | hide 两次 | delegate 回调走 `hide_main_window` 统一入口，guard 一致；macOS 的 Focused 分支置空 |
| **panel 不跟随 Space** | 全屏 app 切换后找不到 | show/hide 动态切 collectionBehavior（已设计）|
| **copy_back 回填** | panel 是 key 时粘贴可能被吞 | 当前 copy_back 用 clipboard-rs 写剪贴板，不模拟 Cmd+V，**不需要 resign_key**；若未来加「自动粘贴」再预留 |
| **set_dock_visibility** | EcoPaste 隐藏 Dock | 本项目暂不做（托盘常驻但 Dock 保留，用户可在设置里选——当前无此设置项，后续可选加）|

---

## §5 验证计划

### 5.1 编译验证
- `cargo check` macOS target 通过（objc 删除后无残留引用）
- `cargo check` 不引 tauri-nspanel 的 target（CI Windows）仍通过（#cfg 包裹正确）

### 5.2 单测
- `window_control.rs` 现有 5 个纯函数测试（toggle_action / should_hide_on_blur）保持通过
- macos_panel.rs 全是 unsafe ObjC 交互，不写单测（系统交互不可测，§1.22 例外）

### 5.3 实测（核心 — 用户配合）
1. **普通桌面**：快捷键呼出/隐藏 → 正常
2. **多桌面 Space**：切到不同桌面，快捷键呼出 → 窗口跟到当前桌面
3. **🔴 全屏应用**（核心目标）：Safari/Chrome 全屏 → 快捷键呼出 → **窗口出现在全屏应用上面**
4. **全屏 + 失焦隐藏**：全屏呼出后点回全屏应用 → 窗口自动隐藏
5. **托盘左键 toggle**：全屏应用上托盘左键 → 窗口出现/隐藏
6. **搜索 + 键盘导航**：全屏呼出后能输入搜索、↑↓ 导航、回车回填（验证 canBecomeKeyWindow）
7. **copy_back 回填**：全屏呼出 → 选中条目 → 回车 → 粘贴到全屏应用（验证回填不中断）

### 5.4 回归
- Windows 路径不受影响（#cfg 包裹，show/hide/event 走原逻辑）

---

## §6 实施顺序

1. **Cargo.toml**：删 objc + 加 tauri-nspanel git 依赖
2. **macos_panel.rs**：新建模块（register_plugin / setup_panel / show_panel / hide_panel）
3. **lib.rs**：mod 声明 + setup 里 register_plugin（开头）+ setup_panel（末尾替换旧调用）
4. **window_control.rs**：删旧 setup 函数 + show/hide 跨平台分支 + Focused 分支 + read_hide_on_blur_pub
5. **`cargo check`**（macOS）→ 修编译错误
6. **`cargo test`** → 确认现有测试不回归
7. **`tauri dev` 实测**（§5.3 七项）→ 用户确认全屏悬浮成功
8. commit + 更新 progress.md / TODO.md

---

## §7 后续可选（不在本次范围）

- **隐藏 Dock 图标**：`app.set_dock_visibility(false)`（EcoPaste 做法）。当前保留 Dock，如用户
  想纯托盘常驻，加设置项 + 条件调用。
- **copy_back + 自动粘贴**：若未来加「回填后自动 Cmd+V 粘贴」，需 `resign_panel_key` /
  `make_panel_key` 配对（照搬 EcoPaste macos.rs:160-185），否则 Cmd+V 被 panel 吞。
