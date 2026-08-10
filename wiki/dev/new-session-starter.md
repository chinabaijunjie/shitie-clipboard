---
title: 新窗口启动包
tags: [启动, 跨会话]
created: 2026-06-29
updated: 2026-08-09
status: ready-to-use
---

# 新窗口启动包

> 开新窗口续接工作时,直接复制下面的启动咒语。

## 🚀 新窗口启动咒语

**复制以下整段**到新窗口:

```
继续拾贴工作。

## 当前状态 & 下次目标

> 当前状态(版本/测试数/架构) → 见 wiki/dev/progress.md「当前状态」段
> 下次目标 → 见 wiki/dev/TODO.md 顶部「下次目标」
> 当前任务规划 → v2.0.0 代码准备完成(未打包),下一步 macOS 全屏应用呼出窗口修复

## 必读文档
1. AGENTS.md(AI 启动 3 步 + 核心原则 7 模块化 / 8 随写随测 / §8 通用工程纪律)
2. wiki/index.md(知识库总索引,场景驱动速查)
3. wiki/dev/progress.md(当前状态 — 唯一真相源)
4. wiki/dev/TODO.md(下次目标 + 待办)

## ⏰ 待触发任务提醒
- **macOS 全屏应用呼出窗口问题**(核心待解决):
  - 全屏应用(Safari/Chrome 全屏)上快捷键无法呼出拾贴窗口
  - 已尝试 3 种方案均失败:visibleOnAllWorkspaces / NSWindow collectionBehavior / activationPolicy
  - 代码已写(tauri.conf.json 配置)但未生效,需系统排查 Tauri v2 macOS 窗口 Space 行为
  - 详见 wiki/dev/progress.md「v2.0.0 准备完成记录」段
- **v2.0.0 准备已完成**(2026-08-09,commit `d18b057`,未打包):
  - 版本号升级 2.0.0 + 底栏版本号显示(金色 kbd tag)
  - README 重写为面向用户的产品介绍(旧 README 改名 DEVELOPMENT.md)
  - 详见 wiki/dev/progress.md
- **Windows 跨平台适配已完成**(v1.0.0~v1.0.8):
  - Windows 剪贴板格式兼容(CF_* 格式名)+ 单实例锁 + 窗口拖动(data-tauri-drag-region)
  - 详见 wiki/dev/progress.md「Windows 跨平台适配 + CI 打包完成记录」段
- **可选后续**:按格式智能渲染(代码高亮/Markdown 预览,图标位已预留)

## 工作模式
- 遇到决策点先跟我确认,不要自己拍板
- 每完成一个阶段 commit + 汇报,等我确认再进下一阶段
- 里程碑收尾/合并前强制用 zcode-pr-review Skill 跑深度 review(session-checklist 步骤 0)
```

---

> 业务工程起步时,按需补充业务必读文档指针。
