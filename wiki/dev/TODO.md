---
title: 全局 TODO
tags: [TODO, 任务]
created: 2026-06-29
updated: 2026-08-05
status: active
---

# 全局 TODO

> 本工程是 Clipboard Manager（剪贴板历史管理应用），从模板 `tauri-sidecar-template` 复制起步。

## 🎯 下次目标

- **M0 架构清理**：把 sidecar 架构改造成纯 Tauri + Rust 单进程架构
- 详细任务说明 → [[idle-task-context]]

## 路线图

- [ ] **M0**：架构清理（sidecar → 纯 Rust）— 进行中
  - [ ] 删除 backend/ + sidecar.rs + build-sidecar.sh
  - [ ] 前端通信层 axios HTTP → Tauri invoke
  - [ ] 启动守卫 useSidecarReady → useBackendReady（invoke ping）
  - [ ] 端口 9880 相关代码/文档清理
  - [ ] 完成判据 + 验证流程见 [[idle-task-context]] §2.2 / §2.6
- [ ] **M1**：剪贴板监听 MVP（复制文字 → 历史列表出现新条目）
- [ ] **M2**：历史 UI + 检索（搜索 / 点击回填）
- [ ] **M3**：常驻体验（托盘 + 开机自启 + 全局快捷键）
- [ ] **M4**：生产化（去重 / 容量上限 / 敏感过滤 / 打包）

## 待办

（M0 进行中，详见 [[idle-task-context]]）

## 想法池

- [ ] 剪贴板内容智能分类（M4 之后考虑）
- [ ] 多设备同步（远期）
