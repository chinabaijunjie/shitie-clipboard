---
title: 全局 TODO
tags: [TODO, 任务]
created: 2026-06-29
updated: 2026-08-06
status: active
---

# 全局 TODO

> 本工程是 Clipboard Manager(轻量常驻的剪贴板历史管理桌面应用),
> 2026-08-05 从模板 `tauri-sidecar-template` 复制起步,M0/M1 已完成。

## 🎯 下次目标

- **M2 搜索 + 富文本/图片支持**:SQLite FTS5 全文搜索 + clipboard-rs 多格式捕获(RTF/HTML/图片)
- M2 启动前需先做技术 spike(FTS5 中文分词 / 图片存储路径 / 多格式回填),见 §1.16
- M3 常驻体验(全局快捷键唤起 + 托盘 + 自启 + Windows)已确认:交互用「唤起+回车回填」

## 路线图

- [x] **M0**:架构清理 + Spotlight UI 骨架(sidecar → 纯 Rust)— ✅ 完成(2026-08-05)
- [x] **M1**:macOS 剪贴板监听 MVP(clipboard-rs watcher + rusqlite + 历史列表 + 回填)— ✅ 完成(2026-08-06)
- [ ] **M2**:搜索 + 富文本/图片支持(SQLite FTS5 + 多格式)— **下一步**
- [ ] **M3**:常驻体验(全局快捷键唤起 + 托盘 + 自启 + Windows 平台补全)
- [ ] **M4**:生产化(打包 dmg/exe + 去重 + 容量上限 + 敏感过滤)

## 待办

### M1 剪贴板监听 MVP(✅ 完成,见 [[m1-plan]])

- [x] 技术 spike:clipboard-rs watcher + Tauri 后台线程(✅ 验证不触发 NSPasteboard 崩溃)
- [x] M1 规划文档:落盘 `wiki/dev/m1-plan.md`
- [x] **阶段 A**:rusqlite 存储层(items/formats 两表 + CRUD 单测)
- [x] **阶段 B**:剪贴板监听层(watcher → DB → emit 前端)
- [x] **阶段 C**:command 接口(list_items / copy_back / delete_item)
- [x] **阶段 D**:前端 SpotlightPanel 改造(历史列表 + 上下键 + 回填)
- [x] **阶段 E**:端到端实测(6 项全通过)

### M3 常驻体验(已确认交互方案,待规划)

- 全局快捷键唤起/隐藏窗口(M1 遗留:窗口目前 visible:true 常驻)
- 交互:**唤起 + ↑↓ 导航 + 回车回填**(主流方案,Maccy/Flycut 同款)
- ⚠️ 不做「松手回填」:Tauri 全局快捷键只支持 keydown(见 progress M1 记录)

## 想法池

- [ ] 剪贴板内容智能分类(M4 之后考虑)
- [ ] 多设备同步(远期)
