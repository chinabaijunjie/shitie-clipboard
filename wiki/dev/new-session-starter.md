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
继续 Clipboard Manager 工作。

## 当前状态 & 下次目标

> 当前状态(版本/测试数/架构) → 见 wiki/dev/progress.md「当前状态」段
> 下次目标 → 见 wiki/dev/TODO.md 顶部「下次目标」
> 当前任务规划 → M4(A+B+C+UI)+ 多主题 + UI 增强迭代全部完成,下一步 M4 阶段 D

## 必读文档
1. AGENTS.md(AI 启动 3 步 + 核心原则 7 模块化 / 8 随写随测 / §8 通用工程纪律)
2. wiki/index.md(知识库总索引,场景驱动速查)
3. wiki/dev/progress.md(当前状态 — 唯一真相源)
4. wiki/dev/TODO.md(下次目标 + 待办)

## ⏰ 待触发任务提醒
- **UI 增强迭代已完成**(2026-08-09):多主题切换 + 透明度可调 + 存储统计卡片 + 清除历史
  + 内容识别(link/markdown) + 类型筛选 tag + 双击回填 + 类型图标 + 文件/图片路径展示。
  Schema v3(content_subtype 列)。详见 wiki/dev/progress.md。
- **下一步:M4 阶段 D**(Windows PD 虚拟机实测 + push tag 触发 CI 打包发布)。
  - ⚠️ CI 前置:用户需在 GitHub 建仓库 + git remote add + push tag v1.0.0
- **可选后续**:按格式智能渲染(代码高亮/Markdown 预览,图标位已预留)

## 工作模式
- 遇到决策点先跟我确认,不要自己拍板
- 每完成一个阶段 commit + 汇报,等我确认再进下一阶段
- 里程碑收尾/合并前强制用 zcode-pr-review Skill 跑深度 review(session-checklist 步骤 0)
```

---

> 业务工程起步时,按需补充业务必读文档指针。
