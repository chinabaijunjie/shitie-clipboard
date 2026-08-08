---
title: 全局 TODO
tags: [TODO, 任务]
created: 2026-06-29
updated: 2026-08-08
status: active
---

# 全局 TODO

> 本工程是 Clipboard Manager(轻量常驻的剪贴板历史管理桌面应用),
> 2026-08-05 从模板 `tauri-sidecar-template` 复制起步,M0/M1/M2/M3 已完成。

## 🎯 下次目标

- **M4 生产化**:打包 dmg/exe + 去重 + 敏感过滤 + Windows 平台补全
- 可选并行:**UI 苹果审美改造**(见 [[ui-redesign-plan]],M3 已完成可启动)

## 路线图

- [x] **M0**:架构清理 + Spotlight UI 骨架(sidecar → 纯 Rust)— ✅ 完成(2026-08-05)
- [x] **M1**:macOS 剪贴板监听 MVP(clipboard-rs watcher + rusqlite + 历史列表 + 回填)— ✅ 完成(2026-08-06)
- [x] **M2**:搜索 + 富文本/图片支持(SQLite FTS5 + 多格式)— ✅ 完成(2026-08-07)
- [x] **M3**:常驻体验(全局快捷键 + 托盘 + 自启 + 设置面板 + 自动清理)— ✅ 完成(2026-08-08)
- [ ] **M4**:生产化(打包 dmg/exe + 去重 + 敏感过滤 + Windows 平台补全)— **下一步**
- [ ] **UI 苹果审美改造**(M3 完成后执行,见 [[ui-redesign-plan]])

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

### UI 苹果审美改造(M3 完成后执行,见 [[ui-redesign-plan]])

> ⏳ 触发条件:M3 已完成 ✅。基于 emilkowalski/skills 设计规则 + 苹果 WWDC 原则。
> 高频常驻工具,审美改造排 M3 之后避免返工。

- [ ] **Token 层** — `index.css` 加缓动/颜色/圆角/阴影 token,`tailwind.config.ts` 接线
- [ ] **毛玻璃外壳** — SpotlightPanel 根容器改 `backdrop-filter: blur()`(窗口已 transparent)
- [ ] **配色替换** — 换苹果系统色(暗色深灰蓝 #1C1C1E 而非纯黑),亮/暗模式校对
- [ ] **列表项重做** — 胶囊选中态 + hover 过渡 + 按压反馈 + 字距分级
- [ ] **顶/底栏** — 硬边框 `border-b` 改渐变蒙版
- [ ] **动效(克制)** — 只加 hover/按压/新增项/错误提示,高频操作不加动画(频率闸门)
- [ ] **滚动条** — 隐形细滚动条
- [ ] **自检** — emilkowalski Never Ship 清单 + `review-animations` 标准
- [ ] **实测** — macOS 实机看毛玻璃质感

## 想法池

- [ ] 剪贴板内容智能分类(M4 之后考虑)
- [ ] 多设备同步(远期)
