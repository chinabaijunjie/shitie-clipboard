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

- **M1 macOS 剪贴板监听 MVP**:复制文字 → 历史列表出现新条目 → 上下键/点击回填
- 涉及:NSPasteboard.changeCount 事件监听 + rusqlite 存储 + 前端历史列表 UI + 回填
- 先做 macOS,Windows 在 M1 后期或 M2

## 路线图

- [x] **M0**:架构清理 + Spotlight UI 骨架(sidecar → 纯 Rust)— ✅ 完成(2026-08-05)
- [ ] **M1**:macOS 剪贴板监听 MVP(NSPasteboard.changeCount + rusqlite + 历史列表 + 回填)— **进行中**
- [ ] **M2**:搜索 + 富文本/图片支持(SQLite FTS5 + 多格式)
- [ ] **M3**:常驻体验(全局快捷键唤起 + 托盘 + 自启 + Windows 平台补全)
- [ ] **M4**:生产化(打包 dmg/exe + 去重 + 容量上限 + 敏感过滤)

## 待办

### M1 剪贴板监听 MVP(待规划)

M1 启动前需先做技术 spike + 规划文档(§1.16 重大任务先调研):

- [ ] **技术 spike**:macOS NSPasteboard.changeCount 监听 + arboard/cocoa-objc 读取富文本/图片可行性
- [ ] **M1 规划文档**:落盘 `wiki/dev/m1-plan.md`(进程模型 / 数据模型 / command 设计 / UI 交互)
- [ ] Rust 侧:clipboard 监听线程 + rusqlite 存储 + commands(list_items / copy_back)
- [ ] 前端:SpotlightPanel 改造为历史列表(上下键导航 + 点击回填)
- [ ] 数据模型:ClipboardItem 类型(前端 + Rust 两端定义)

## 想法池

- [ ] 剪贴板内容智能分类(M4 之后考虑)
- [ ] 多设备同步(远期)
