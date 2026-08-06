---
title: 全局 TODO
tags: [TODO, 任务]
created: 2026-06-29
updated: 2026-08-05
status: active
---

# 全局 TODO

> 本工程是 Clipboard Manager(轻量常驻的剪贴板历史管理桌面应用),
> 2026-08-05 从模板 `tauri-sidecar-template` 复制起步,M0 架构清理已完成。

## 🎯 下次目标

- **M1 macOS 剪贴板监听 MVP**:复制文字 → 历史列表出现新条目 → 上下键/回车回填
- ✅ 技术调研 + spike 已完成(见 [[m1-research]]):clipboard-rs 选定,线程崩溃坑已验证不触发
- ✅ M1 规划已落盘(见 [[m1-plan]]):5 阶段(存储/监听/命令/UI/实测)
- ▶️ **下一步:阶段 A — rusqlite 存储层(items/formats 两表 + CRUD 单测)**

## 路线图

- [x] **M0**:架构清理 + Spotlight UI 骨架(sidecar → 纯 Rust)— ✅ 完成(2026-08-05)
- [ ] **M1**:macOS 剪贴板监听 MVP(clipboard-rs watcher + rusqlite + 历史列表 + 回填)— **进行中**
- [ ] **M2**:搜索 + 富文本/图片支持(SQLite FTS5 + 多格式)
- [ ] **M3**:常驻体验(全局快捷键唤起 + 托盘 + 自启 + Windows 平台补全)
- [ ] **M4**:生产化(打包 dmg/exe + 去重 + 容量上限 + 敏感过滤)

## 待办

### M1 剪贴板监听 MVP(规划已落盘,见 [[m1-plan]])

- [x] 技术 spike:clipboard-rs watcher + Tauri 后台线程(✅ 验证不触发 NSPasteboard 崩溃)
- [x] M1 规划文档:落盘 `wiki/dev/m1-plan.md`
- [ ] **阶段 A**:rusqlite 存储层(items/formats 两表 + CRUD 单测)
- [ ] **阶段 B**:剪贴板监听层(watcher → DB → emit 前端)
- [ ] **阶段 C**:command 接口(list_items / copy_back / delete_item)
- [ ] **阶段 D**:前端 SpotlightPanel 改造(历史列表 + 上下键 + 回填)
- [ ] **阶段 E**:端到端实测

## 想法池

- [ ] 剪贴板内容智能分类(M4 之后考虑)
- [ ] 多设备同步(远期)
