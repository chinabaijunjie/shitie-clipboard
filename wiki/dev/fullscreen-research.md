---
title: macOS 全屏悬浮窗口调研
tags: [调研, macOS, NSPanel, 全屏, tauri-nspanel]
created: 2026-08-10
updated: 2026-08-10
status: active
---

# macOS 全屏应用悬浮窗口调研

> **背景**：拾贴（Tauri 2 剪贴板工具）的全局快捷键在 macOS 全屏应用（Safari/Chrome native fullscreen）
> 上无法呼出窗口。上一会话尝试 3 种方案均失败。本次会话（2026-08-10）做系统调研。
>
> **调研方法**：3 个并行 subagent 分头深挖 ① macOS 官方机制 ② Tauri/tao 框架层 ③ 同类工具参考实现。
> 三路结论高度一致、互相印证。

---

## TL;DR

- **根因**：普通 `NSWindow` 无法全屏悬浮，是**两个独立 macOS 机制叠加**导致的：
  1. Space 隔离 —— 全屏应用占独立 Space，普通窗口进不去（需 `collectionBehavior` 突破）
  2. app 激活导致 Space 切换 —— 普通 NSWindow 的 `show`/`set_focus` 底层调
     `activateIgnoringOtherApps:YES`，强制切回 app 的 Space → 全屏应用被挤走
- **上次失败的根因诊断完全正确**：`NSWindowStyleMaskNonactivatingPanel` 只对 `NSPanel` 子类生效
  （依赖 NSPanel 初始化时设的 WindowServer 内部标签，普通 NSWindow 加 mask 不改变行为）
- **Tauri 现状**：底层 tao 库硬编码用 `NSWindow`（`TaoWindow` 类），无 NSPanel 支持，
  官方标记为 "not planned"（#11488 / #5793 / #11791 全 closed）
- **唯一成熟方案**：`tauri-nspanel` 插件（isa-swizzle，412 star，awesome-tauri 收录），
  EcoPaste（7283 star 同类剪贴板工具）已验证可行
- **已选方案**：用 tauri-nspanel 插件（用户拍板 2026-08-10）

---

## §1 macOS 官方窗口机制

### 1.1 NSPanel vs NSWindow 的本质区别

`NSPanel` 是 `NSWindow` 的子类，专为辅助性窗口（浮动工具面板、Spotlight、查找框）设计：

| 特性 | NSWindow | NSPanel |
|------|----------|---------|
| 显示时激活 app | 是（强制） | 可配置为不激活（`nonactivatingPanel`）|
| 成为 key window 的条件 | 显示即成为 key | 可延迟到「真正需要」（`becomesKeyOnlyIfNeeded`）|
| 模态窗口弹出时可用 | 否 | 可（`worksWhenModal`）|
| app 失活时自动隐藏 | 否 | 可配置（`hidesOnDeactivate`）|
| 浮动层级 | 需手动设 | 内建 `isFloatingPanel` 属性 |

**核心属性**：
- `NSWindowStyleMaskNonactivatingPanel`（1<<7）：panel 被点击/显示时**不激活所属 app**。
  底层原理：NSPanel 初始化时调内部方法设 `kCGSPreventsActivationTagBit` 窗口标签，普通 NSWindow 不做这步。
  → **给普通 NSWindow 的 styleMask 加这个 bit，底层标签不会更新，行为不变**。这是上次失败的根因。
  来源：[philz.blog 深度分析](https://philz.blog/nspanel-nonactivating-style-mask-flag/)
- `becomesKeyOnlyIfNeeded`：panel 仅在被点击视图的 `needsPanelToBecomeKey` 返回 true 时才成 key。
  实现「不激活 app 但能接收键盘输入」的关键。

### 1.2 全屏（native fullscreen）的 Space 机制

native fullscreen 时 macOS 创建**专用独立 Space**，完全隔离。关键 `collectionBehavior` flags：

| Flag | 值 | 作用 |
|------|-----|------|
| `CanJoinAllSpaces` | 1<<0 (1) | 跨所有虚拟桌面可见 |
| `FullScreenAuxiliary` | 1<<8 (256) | 可作为辅助层出现在全屏 Space 上（**全屏悬浮关键**）|
| `FullScreenPrimary` | 1<<7 (128) | 窗口自己进全屏 |
| `CanJoinAllApplications` | 1<<18 (262144) | 可加入**其他 app** 的全屏 Space（Spotlight 类关键）|
| `Stationary` | 1<<4 (16) | Space 切换时不动画 |
| `MoveToActiveSpace` | — | 移到当前活跃 Space |

**关键区分**：
- `canJoinAllSpaces`：跨**虚拟桌面**
- `fullScreenAuxiliary`：能出现在**全屏 Space**
- `canJoinAllApplications`：能出现在**其他 app** 的全屏 Space（全屏悬浮的三件套之一）

来源：[Apple collectionBehavior 文档](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct)

### 1.3 Window Level 数值表

| Level | 数值 | 用途 |
|-------|------|------|
| Normal | 0 | 标准应用窗口（Safari 等）|
| Floating | 3 | 浮动面板 |
| ModalPanel | 8 | 模态对话框 |
| MainMenu | 24 | 菜单栏 |
| Status | 25 | 状态栏项目 |
| PopUpMenu | 101 | 弹出菜单 |
| ScreenSaver | 1000 | 屏保（最高标准级别）|

**核心事实**：全屏应用窗口的 level 本身不高。全屏隔离的本质是 **Space 隔离**，不是 level 提高。
窗口 level 只控制**同一 Space 内**的堆叠顺序。要盖住全屏窗口需要：**足够高的 level** +
**正确的 collectionBehavior**（让它加入全屏 Space）。

Maccy 用 `.screenSaver`(1000)，EcoPaste 用 `PanelLevel::Dock`，Tauri issue #11488 用户方案用
`NSMainMenuWindowLevel + 1`(25)。

来源：[Noticky 窗口层级详解](https://www.noticky.app/en/blog/macos-window-levels-explained)

### 1.4 Activation Policy

| 策略 | Dock | 菜单栏 | 对全屏悬浮的影响 |
|------|------|--------|-----------------|
| `.regular` | 显示 | 有 | show 会激活 app 切 Space，**全屏应用被挤走** |
| `.accessory` | 不显示 | 无 | app 不抢焦点，窗口可浮动显示，**不挤走全屏应用** |
| `.prohibited` | 不显示 | 无 | 不能创建窗口，完全无法显示 UI |

Spotlight / Alfred / Raycast 用 `.accessory`（配合 `LSUIElement = true`）。

来源：[Multi.app 博客](https://multi.app/blog/nailing-the-activation-behavior-of-a-spotlight-raycast-like-command-palette)、
[Art Lasovsky](https://artlasovsky.com/fine-tuning-macos-app-activation-behavior)

### 1.5 完整「全屏悬浮配方」

```swift
class FloatingPanel: NSPanel {
    init(contentRect: NSRect) {
        super.init(contentRect:, styleMask: [.nonactivatingPanel, ...], ...)
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = true
        isFloatingPanel = true
        level = .statusBar  // 或更高（实践中常用 1000+）
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary,
                              .canJoinAllApplications, .stationary]
    }
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}
// App: setActivationPolicy(.accessory) + LSUIElement = true
// 显示: orderFrontRegardless()  // 不触发激活
```

---

## §2 Tauri / tao 框架层

### 2.1 tao 硬编码用 NSWindow

tao（Tauri 底层窗口库）在 macOS 用自定义类 `TaoWindow`，**继承 NSWindow**（不是 NSPanel）：

```rust
// tao/src/platform_impl/macos/window.rs:406-425
let window_superclass = class!(NSWindow);  // ← 父类是 NSWindow
let mut decl = ClassDecl::new("TaoWindow", window_superclass).unwrap();
```

全文 0 处提及 NSPanel。`WindowBuilderExtMacOS` trait 全是样式配置，**没有任何 hook 注入自定义窗口类**。

### 2.2 set_focus 是元凶（源码确认）

tao 的 `set_focus` 底层调用链（已从源码确认）：

```rust
// tao/src/platform_impl/macos/util/async.rs:237-244
pub unsafe fn set_focus(ns_window: &NSWindow) {
    run_on_main(move || {
        ns_window.makeKeyAndOrderFront(None);           // ← 让窗口成 key 并前置
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let () = msg_send![app, activateIgnoringOtherApps: YES];  // ← ★强制激活 app
    });
}
```

`makeKeyAndOrderFront + activateIgnoringOtherApps:YES` = 激活 app = 切到 app 的 Space = 全屏应用被挤走。
**这就是上次 PR review 发现的 `show_main_window` 的 `set_focus()` 矛盾根因**。

替代方法：NSPanel 用 `orderFrontRegardless()`（不触发激活）。

### 2.3 Tauri 官方明确不修

相关 issue 全部 closed as not planned：
- [#11488](https://github.com/tauri-apps/tauri/issues/11488) — visibleOnAllWorkspaces 全屏无效（closed）
- [#5793](https://github.com/tauri-apps/tauri/issues/5793) — show window on top of fullscreen（closed）
- [#11791](https://github.com/tauri-apps/tauri/issues/11791) — transparent app above fullscreen（closed）
- [tao#414](https://github.com/tauri-apps/tao/issues/414) — NSPanel behavior needed（open，无维护者回应）

### 2.4 四条技术路线评估

| 路线 | 可行性 | 工程量 | 评估 |
|------|--------|--------|------|
| **A. isa-swizzle**（object_setClass 替换窗口类）| **高** | 小-中 | ✅ tauri-nspanel 插件已实现，EcoPaste 验证 |
| B. 自建 NSPanel + reparent webview | 中 | 中-大 | wry 有 reparent API 但 Tauri 不暴露，事件路由复杂 |
| C. hook tao 窗口创建 | 低 | 大 | tao 无 hook 点，需 fork |
| D. 完全自建 NSPanel + wry | 低 | 极大 | 丢失 Tauri 生态（IPC/插件/事件循环）|

**isa-swizzle 原理**：用 ObjC runtime 的 `object_setClass()` 把已创建的 `TaoWindow` 实例的 isa 指针
替换为继承 NSPanel 的自定义子类。`object_setClass` 不能添加 ivar（新类不能比原类多实例变量），
所以只做方法覆盖（`canBecomeKey` / `canBecomeMainWindow` 等）。

**已知风险**：
- macOS Big Sur+ 兼容性：Electron 说 swizzle 原生 NSWindow 在 Big Sur+ 不行，但 tao 的 TaoWindow
  本身是自定义子类，tauri-nspanel 仍可用（2026-08 仍活跃维护）
- 转成 NSPanel 后 `is_maximised` / maximize 会 panic（plugins-workspace#1546）—— 本项目 `resizable:false` 不受影响

---

## §3 tauri-nspanel 插件（选定的方案）

### 3.1 基本信息

- 仓库：[github.com/ahkohd/tauri-nspanel](https://github.com/ahkohd/tauri-nspanel)（412 star）
- 分支：`v2.1`（Tauri v2 兼容）
- 技术：isa-swizzle（objc2 `define_class!` + `object_setClass`）
- 收录：awesome-tauri 官方插件列表
- 维护：2026-08-05 仍有更新；作者 ahkohd 也是 tauri#9556 的 assignee

### 3.2 已验证的同类项目

插件 README Showcase 列出的使用项目（节选）：
- **EcoPaste**（7283 star）— **与本工程最对标的剪贴板管理器** ⭐ 最佳参考模板
- Cap（屏幕录制）/ Screenpipe（AI 录制）/ Verve（启动器）/ Overlayed（屏幕覆盖层）
- BongoCat（桌面宠物）/ Coco（AI 搜索）

### 3.3 EcoPaste 的验证配方（源码直接提取）

```rust
// Cargo.toml
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2.1" }

// 1. 定义 Panel 子类（宏）
tauri_panel! {
    panel!(MainPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: false
        }
    })
}

// 2. setup 阶段转 NSPanel + 配置
let panel = clipboard_window.to_panel::<MainPanel>()?;
panel.set_corner_radius(16.0);
panel.set_level(PanelLevel::Dock.value());
panel.set_style_mask(StyleMask::empty().resizable().nonactivating_panel().into());
panel.set_collection_behavior(
    CollectionBehavior::new().stationary().move_to_active_space().full_screen_auxiliary().into(),
);

// 3. show 时切 collectionBehavior（跟随当前 Space）
fn show_clipboard_panel(...) {
    panel.show_and_make_key();  // ← 用 orderFront 而非 makeKeyAndOrderFront + activate
    panel.set_collection_behavior(
        CollectionBehavior::new().stationary().can_join_all_spaces().full_screen_auxiliary().into(),
    );
}

// 4. hide 后切回 move_to_active_space（下次 show 重新落位）
fn hide_clipboard_panel(...) {
    panel.hide();
    panel.set_collection_behavior(
        CollectionBehavior::new().stationary().move_to_active_space().full_screen_auxiliary().into(),
    );
}
```

**EcoPaste 的巧妙设计**：show/hide 动态切换 `canJoinAllSpaces ↔ moveToActiveSpace`，
实现「弹出时跟随当前 Space，隐藏后下次弹出到新的当前 Space」。

### 3.4 Tauri issue #11488 的用户验证代码

```rust
appHandle.set_activation_policy(ActivationPolicy::Accessory);
let second_window = appHandle.get_webview_window("second").unwrap();
let panel = second_window.to_panel().unwrap();
panel.set_collection_behaviour(
    NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorStationary
        | NSWindowCollectionBehaviorFullScreenAuxiliary,
);
panel.set_level(NSMainMenuWindowLevel + 1);
panel.show();
appHandle.set_activation_policy(ActivationPolicy::Regular);
// 用户反馈：「dock 图标在，主窗口正常，popup panel 盖在所有窗口上，包括全屏的」
```

---

## §4 同类工具参考

### Maccy（开源 macOS 原生剪贴板，Swift）

`Maccy/FloatingPanel.swift` — `class FloatingPanel: NSPanel, NSWindowDelegate`：

```swift
styleMask = [.nonactivatingPanel, .resizable, .closable, .fullSizeContentView]
isFloatingPanel = true
level = .screenSaver  // 1000，注释：Chrome autofill 在 999，刚好盖过
collectionBehavior = [.auxiliary, .stationary, .moveToActiveSpace, .fullScreenAuxiliary]
hidesOnDeactivate = false
canBecomeKey = true  // 允许文本输入获取焦点
```

### Multi.app 博客（Spotlight/Raycast 风格命令面板，最完整配方）

```swift
// 启动时不抢焦点
func applicationWillFinishLaunching(_) { NSApp.setActivationPolicy(.prohibited) }
func applicationDidFinishLaunching(_) { NSApp.setActivationPolicy(.accessory) }

// NSPanel 子类 + nonactivatingPanel
class MultiAppWindow: NSPanel {
    init() { super.init(styleMask: [.closable, .titled, .nonactivatingPanel], ...); level = .modalPanel }
}
override func resignKey() { close() }  // 失焦自动关闭
override func becomeKey() { NSApp.activate(); super.becomeKey() }  // 必要时强制激活
```

### Electron（对比）

Electron 同样用 NSWindow，**无官方 NSPanel 支持**。`visibleOnFullScreen` + `setAlwaysOnTop`
是部分解法但不可靠（#3888 / #2097），社区需写 native addon swizzle。长期请求 #4939（2016）至今未实现。

---

## §5 全部参考来源

### Apple 官方文档
- [NSWindowStyleMaskNonactivatingPanel](https://developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/nonactivatingpanel)
- [NSPanel 类文档](https://developer.apple.com/documentation/appkit/nspanel)
- [becomesKeyOnlyIfNeeded](https://developer.apple.com/documentation/appkit/nspanel/1528836-becomeskeyonlyifneeded)
- [collectionBehavior](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct)
- [fullScreenAuxiliary](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/fullscreenauxiliary)
- [activate(ignoringOtherApps:)](https://developer.apple.com/documentation/appkit/nsapplication/activate(ignoringotherapps:))

### 深度技术博客
- ⭐ [philz.blog — NSPanel nonactivating mask 为什么对 NSWindow 无效](https://philz.blog/nspanel-nonactivating-style-mask-flag/)（上次失败根因的权威解释）
- ⭐ [Multi.app — Spotlight/Raycast 风格命令面板的 activation 行为](https://multi.app/blog/nailing-the-activation-behavior-of-a-spotlight-raycast-like-command-palette)
- ⭐ [Cindori — SwiftUI floating panel 完整代码](https://cindori.com/developer/floating-panel)
- [Noticky — macOS Window Levels 详解](https://www.noticky.app/en/blog/macos-window-levels-explained)
- [Art Lasovsky — macOS App Activation Behavior](https://artlasovsky.com/fine-tuning-macos-app-activation-behavior)

### Tauri/tao/wry 源码 + issue
- tao 窗口类（TaoWindow 继承 NSWindow）：`tao/src/platform_impl/macos/window.rs`
- tao set_focus（元凶，activateIgnoringOtherApps:YES）：`tao/src/platform_impl/macos/util/async.rs:237`
- [tauri#11488](https://github.com/tauri-apps/tauri/issues/11488)（closed，用户验证代码）
- [tauri#13034](https://github.com/tauri-apps/tauri/issues/13034)（NSPanel 功能请求，open）
- [tao#414](https://github.com/tauri-apps/tao/issues/414)（NSPanel 需求，open）
- [plugins-workspace#1546](https://github.com/tauri-apps/plugins-workspace/issues/1546)（NSPanel 后 maximize panic）

### 社区方案
- ⭐ [tauri-nspanel 插件](https://github.com/ahkohd/tauri-nspanel)（412 star）
- ⭐ [EcoPaste 剪贴板管理器](https://github.com/EcoPasteHub/EcoPaste)（7283 star，最佳参考，`src-tauri/src/window/macos.rs`）
- [tauri-macos-spotlight-example](https://github.com/ahkohd/tauri-macos-spotlight-example)
- [Maccy](https://github.com/p0deje/Maccy)（`Maccy/FloatingPanel.swift`）

### Stack Overflow
- [Allow NSWindow/NSPanel to float above full screen apps](https://stackoverflow.com/questions/36205834)
- [Why does my NSWindow not float above full screen apps](https://stackoverflow.com/questions/58934673)
- [How to force NSWindow in front of fullscreen apps](https://stackoverflow.com/questions/10905045)

---

## §6 下一步

用户已拍板用 tauri-nspanel 插件。具体集成方案（精确到 API 调用 + 文件改动）见
`wiki/dev/fullscreen-plan.md`（待编写，依赖后台 agent 调研 tauri-nspanel 完整 API + EcoPaste 源码映射）。
