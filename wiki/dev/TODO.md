---
title: 全局 TODO
tags: [TODO, 任务]
created: 2026-06-29
updated: 2026-08-10
status: active
---

# 全局 TODO

> 本工程是拾贴(轻量常驻的剪贴板历史管理桌面应用),
> 2026-08-05 从模板 `tauri-sidecar-template` 复制起步,M0/M1/M2/M3/M4(A+B+C+UI)+ 多主题切换
> + UI 增强迭代 + Windows 跨平台适配(v1.0.0~v1.0.8) + v2.0.0 准备 + macOS 全屏悬浮修复 已完成。

## 🎯 下次目标

> macOS 全屏悬浮修复已完成(2026-08-10,commit `12b1a72`),详见 [[fullscreen-research]] + [[fullscreen-plan]]。
> 当前无阻断性待办。可选后续见下方「想法池」。



## 路线图

- [x] **M0**:架构清理 + Spotlight UI 骨架(sidecar → 纯 Rust)— ✅ 完成(2026-08-05)
- [x] **M1**:macOS 剪贴板监听 MVP(clipboard-rs watcher + rusqlite + 历史列表 + 回填)— ✅ 完成(2026-08-06)
- [x] **M2**:搜索 + 富文本/图片支持(SQLite FTS5 + 多格式)— ✅ 完成(2026-08-07)
- [x] **M3**:常驻体验(全局快捷键 + 托盘 + 自启 + 设置面板 + 自动清理)— ✅ 完成(2026-08-08)
- [x] **M4 A+B+C**:打包 CI + 去重修复 + 敏感过滤 — ✅ 完成(2026-08-09)
- [x] **M4 UI 苹果审美改造**:深色毛玻璃主题 + 三组件重做 — ✅ 完成(2026-08-09)
- [x] **多主题切换**:深色/浅色/折中三套主题 + 分段按钮选择器 — ✅ 完成(2026-08-09)
- [x] **UI 增强迭代**:透明度+存储统计+内容识别+类型筛选+双击回填+类型图标 — ✅ 完成(2026-08-09)
- [x] **Windows 跨平台适配 + CI 打包**(v1.0.0~v1.0.8):格式兼容 + 单实例锁 + 窗口拖动 — ✅ 完成(2026-08-09)
- [x] **v2.0.0 准备**:版本号 + 底栏显示 + README 重写 + macOS 全屏配置(代码已写,全屏未生效)— ✅ 代码完成(2026-08-09)
- [x] **macOS 全屏应用呼出窗口修复**:tauri-nspanel isa-swizzle(NSPanel)— ✅ 完成(2026-08-10)

## 待办

### M2 搜索 + 富文本/图片支持(✅ 完成,见 [[m2-plan]])

- [x] 技术 spike:FTS5 + bigram 中文搜索(✅ 8 测试全绿,见 [[m2-research]])
- [x] M2 规划文档:落盘 `wiki/dev/m2-plan.md`
- [x] **阶段 A**:schema migration(手写版本号)+ 多格式存储基础(search_text 列 + FTS 表)
- [x] **阶段 B**:多格式捕获(白/黑名单)+ 多格式回填(富文本/文件)
- [x] **阶段 C**:图片存储 + 缩略图渲染 + 引用计数删除
- [x] **阶段 D**:搜索功能(FTS5 + bigram + 搜索框 UI)
- [x] **阶段 E**:端到端实测 + PR review(4 Critical + 5 Important 全修)

### M2 后改进(✅ 全部完成,2026-08-08)

> 来源:M2 PR review(5 agent 并行,见 progress M2 记录)。这些是 code quality 改进,
> 非阻断性 bug(GUI 已验证功能正常)。2026-08-08 独立改进会话清完。
- [x] newtype 固化类型(WhitelistUti / RelPath / AbsPath,type-design 建议,重构级)
- [x] copy_back 组装逻辑抽纯函数 `assemble_contents` + 补测试(test-analyzer 建议)
- [x] 前端搜索状态机测试(searchQueryRef 闭包 / search 清空回退,test-analyzer 7/10)

### M1 剪贴板监听 MVP(✅ 完成,见 [[m1-plan]])

- [x] 技术 spike:clipboard-rs watcher + Tauri 后台线程(✅ 验证不触发 NSPasteboard 崩溃)
- [x] M1 规划文档:落盘 `wiki/dev/m1-plan.md`
- [x] **阶段 A**:rusqlite 存储层(items/formats 两表 + CRUD 单测)
- [x] **阶段 B**:剪贴板监听层(watcher → DB → emit 前端)
- [x] **阶段 C**:command 接口(list_items / copy_back / delete_item)
- [x] **阶段 D**:前端 SpotlightPanel 改造(历史列表 + 上下键 + 回填)
- [x] **阶段 E**:端到端实测(6 项全通过)

### M3 常驻体验(✅ 完成,见 [[m3-plan]])

- [x] **阶段 A**:设置持久化层(Settings 模块 + Store 插件 + command + 类型)
- [x] **阶段 B**:全局快捷键 + 窗口控制(toggle/hide/show + close 拦截 + 失焦隐藏 + 进程常驻)
- [x] **阶段 C**:托盘 + 开机自启(LaunchAgent + --hidden)
- [x] **阶段 D**:回填增强(移到最前 + 隐藏窗口 + Esc 逻辑)
- [x] **阶段 E**:设置面板 UI(7 项配置 + 视图切换)
- [x] **阶段 F**:自动清理(保留天数 + 容量上限 + 条数上限) + PR review

### UI 苹果审美改造(✅ 完成,2026-08-09,见 [[ui-redesign-plan]] + [[ui-design-spec]])

> ✅ 已完成。深色毛玻璃主题(① 号方案)落地。
> 设计稿:design-mockups/ 三份风格 HTML。完整规格:[[ui-design-spec]]。

- [x] **Token 层** — index.css 深色①完整配色 + tailwind.config.js 接线
- [x] **半透明外壳** — SpotlightPanel + SettingsPanel(去掉 backdrop-filter 避免 WKWebView 滚动闪烁)
- [x] **配色替换** — 换苹果系统色(暗色深灰蓝 #1C1C1E,透明度 0.85)
- [x] **列表项重做** — CSS Grid 三列 + 胶囊选中 + hover + 按压 + 选中展开三行 + 图片放大
- [x] **顶/底栏** — 标题居中可拖拽 + 搜索框卡片化 + 半透明分隔线
- [x] **设置面板** — 分组 + Toggle/Select 苹果化 + autostart 加载占位
- [x] **滚动条** — 隐形细滚动条
- [x] **图标** — lucide-react 替换 emoji(Settings/Search/ArrowLeft/Check/FileText/Clipboard)
- [x] **窗口圆角** — #root border-radius 裁剪隐藏灰色直角
- [x] **实测** — macOS 实机多轮调校通过

## 想法池

- [ ] **按格式智能渲染** — 代码语法高亮 / Markdown 预览(图标位已预留,需检测逻辑)
- [ ] 剪贴板内容智能分类(M4 之后考虑)
- [ ] 多设备同步(远期)
