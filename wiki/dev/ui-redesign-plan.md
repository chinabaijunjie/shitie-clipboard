---
title: UI 苹果审美改造规划(M2 完成后执行)
tags: [UI, 设计, 规划, 苹果, 动效]
created: 2026-08-07
updated: 2026-08-07
status: planned
---

# UI 苹果审美改造规划

> ⏳ **执行时机:M2(搜索 + 富文本/图片)全部完成之后**。
> 原因:M2 会引入搜索框、多格式列表项、图片缩略图等改变 UI 结构的功能,
> 提前改样式会因功能迭代反复返工(详见 §1 执行时机判断)。
>
> 📌 **触发条件**(写进 `new-session-starter.md`):当 M2 五阶段(A-E)全部 ✅,
> 在 `progress.md` 标记 M2 完成后,下一个会话首项任务即启动本规划。

## §0 本文档的来源

基于两个输入:

1. **emilkowalski/skills**(GitHub 14.4k★,Vercel/Linear 设计工程师的 AI 技能包)——
   把顶尖设计师的隐性审美知识转化为 AI 可执行的显性规则。核心技能:
   - `emil-design-eng`(主技能:动画决策框架 + 组件构建原则)
   - `animate`(从零造动画的决策顺序)
   - `review-animations`(动画审查标准)
   - `apple-design`(苹果 WWDC 设计原则,适配 Web)
   - `animation-vocabulary`(动画术语词典)
2. **项目当前 UI 诊断** —— 对 `index.css` / `SpotlightPanel.tsx` / `ClipboardList.tsx`
   的实测审视(2026-08-07)。

⚠️ **两点警惕**(来自对 skills 库的诚实评估):
- 作者利益绑定:`pick-ui-library` 推荐的 Sonner 是作者自研,base-ui/cmdk 是其雇主生态。
  本项目技术栈已定(Tailwind + 原生组件),不涉及选型,故此风险不传导。
- 强硬规则非宗教:如「never ease-in」「never scale(0)」绝大多数正确,但真实产品偶有合理破例。
  本规划把它们当 checklist,不当教条。

## §1 执行时机判断(为什么排在 M2 之后)

| 因素 | 说明 |
|------|------|
| 稳定 > 体验 | 项目核心原则 1。审美是体验增强,排在功能稳定之后 |
| M2 改变 UI 结构 | 搜索框 → 改顶栏;多格式图标/图片缩略图 → 改列表项。提前改必返工 |
| 动效侦察前置 | emilkowalski `improve-animations` 工作流第一步是 Recon —— 摸清"动效面/产品性格/高频项"。功能未定型时侦察结论易过时 |
| 毛玻璃依赖功能态 | 空状态文案、搜索框聚焦动效、图片缩略图圆角 —— 都依赖最终交互形态 |

**结论**:现在只沉淀规划,不改代码。M2 完成后照本规划一次成型。

## §2 设计 Token(一次定好,全项目复用)

> 这是整个改造的核心资产。所有组件改造都引用这些常量,不再散落硬编码。
> 来源:emilkowalski 值表 + apple-design §15 字体。**值可直接抄,不要近似**。

### 2.1 缓动曲线(emilkowalski 核心纪律:绝不近似)

```css
/* 加到 index.css :root,作为全项目动画 token */
--ease-out: cubic-bezier(0.23, 1, 0.32, 1);        /* 强 ease-out,UI 进入/退出默认 */
--ease-in-out: cubic-bezier(0.77, 0, 0.175, 1);    /* 强 ease-in-out,屏内移动 */
--ease-drawer: cubic-bezier(0.32, 0.72, 0, 1);     /* iOS 风格抽屉曲线( Ionic ) */
```

**纪律(emilkowalski 硬规则)**:
- ❌ 永远不用 `ease-in` 做 UI 动画(开头慢,让界面感觉迟钝)
- ❌ 不用内置 `ease-out`(太弱),必须用上面的强 cubic-bezier
- ❌ 不凭记忆编 `cubic-bezier(0.4, 0, 0.2, 1)`,值从本表抄

### 2.2 时长预算(来自 emilkowalski duration 表)

| 元素 | 时长 | 本项目对应 |
|------|------|-----------|
| 按钮按压反馈 | 100-160ms | 列表项点击 |
| tooltip / 小弹出 | 125-200ms | (暂无) |
| 下拉 / 选择 | 150-250ms | (暂无) |
| 弹窗 / 抽屉 | 200-500ms | 窗口展开(M3) |
| **UI 动画铁律** | **< 300ms** | 所有交互动效 |

### 2.3 频率闸门(emilkowalski 最高频复用的规则)

> **这是剪贴板类高频常驻工具最重要的判断框架**。
> 在决定"要不要加动画"前,先回答频率。

| 频率 | 裁决 | 本项目对应 |
|------|------|-----------|
| 100+/天(快捷键、命令面板) | **永不加动画** | ⚠️ M3 全局唤起、列表上下导航 —— 这些不加动画 |
| 几十/天 | 砍掉或极简 | 列表项 hover |
| 偶尔(弹窗/抽屉/toast) | 标准动画 | 错误提示、(将来)设置面板 |
| 罕见/首次(引导/成功) | 才允许惊喜动效 | 首次使用空状态 |

**Raycast 无开关动画**是 emilkowalski 反复举的标杆 —— 命令面板日开百次,任何动画都让它感觉迟钝。
本项目 M3 的全局唤起同理:唤起窗口不加展开动画。

### 2.4 颜色(苹果系统色,替换 shadcn 默认蓝)

```css
/* 亮色:背景不再是纯白,用苹果浅灰系 */
--background: 0 0% 100%;              /* 卡片本身保持白 */
--app-surface: 240 5% 96%;            /* 应用底色 F2F2F7(iOS 分组背景) */

/* 暗色:不用纯黑,用苹果深灰蓝(关键:材质感来自非纯黑) */
--background: 240 10% 12%;            /* #1C1C1E 而非 #0A0A0A */
--app-surface: 240 10% 8%;            /* 更深一层做分层 */

/* 强调色:克制(苹果蓝),不再饱和度拉满 */
--primary: 211 100% 50%;              /* #0066FF iOS systemBlue */
```

### 2.5 圆角与阴影(苹果材质编码层级)

```css
--radius-window: 12px;                /* 窗口本身(tauri.conf 不支持,用 CSS 圆角) */
--radius-card: 10px;                  /* 列表项选中胶囊 */
--radius-small: 6px;                  /* 小标签 ×N */

/* 多层半透明阴影(苹果质感核心:不是单层黑) */
--shadow-window: 0 22px 70px 4px rgba(0,0,0,0.34),
                 0 8px 24px rgba(0,0,0,0.12);
```

### 2.6 字体层级(来自 apple-design §15)

> 苹果设计字体随大小变形:大字负字距,小字正字距。一个固定 letter-spacing 必错。

```css
/* 大字(标题):负字距 */
.title { letter-spacing: -0.02em; }
/* 小字(时间戳/计数):正字距保可读 */
.caption { letter-spacing: 0.01em; }
```

## §3 现状诊断(2026-08-07 实测)

| 维度 | 现状 | 苹果标准 | 差距 |
|------|------|---------|------|
| 材质 | `bg-background` 纯白/纯黑不透明 | 半透明毛玻璃 | 🔴 窗口已 transparent,被 bg 填死 |
| 层级 | 平面,`border-b` 硬分隔 | 材质重量编码层级 | 🟡 需重做 |
| 边框 | 实线 `border-border` | 阴影 + 顶边高光 | 🟡 减少硬边框 |
| 圆角 | `--radius: 0.5rem` 统一 | 大面更圆,小元素更小 | 🟡 需分档 |
| 阴影 | `shadow-2xl` 单层重黑 | 多层半透明带色相 | 🔴 需替换 |
| 字体 | `text-xs` 统一,默认字距 | 光学字号 + 字距分级 | 🟡 需分级 |
| 颜色 | 纯蓝 primary | 苹果系统色,暗色深灰蓝 | 🟡 需换色 |
| 列表选中 | `bg-accent` 方块 | 圆角胶囊 + 半透明强调 | 🔴 需重做 |
| 动效 | **无任何动画** | 按频率闸门决定 | ⚠️ 大多该不加(高频工具) |

**关键洞察**:窗口已是 `transparent + decorations:false + rounded-lg` —— 毛玻璃的完美底子,
只是 `bg-background` 把透明填死了。这是"离苹果质感只差一层"的状态。

## §4 改造范围(用户决策:全面重构 UI + 做毛玻璃)

### 4.1 视觉系统层(改 CSS + token,不动组件结构)

**文件**:`src/index.css` + `tailwind.config.ts`

1. 加 §2.1 缓动 token + §2.4 颜色 + §2.5 圆角阴影到 `:root` / `.dark`
2. 窗口根容器改毛玻璃:
   ```css
   /* SpotlightPanel 根 div */
   background: hsl(var(--background) / 0.72);
   backdrop-filter: blur(30px) saturate(180%);
   /* 亮色顶边高光(模拟光打在材质边缘) */
   box-shadow: var(--shadow-window);
   ```
3. 非 macOS 平台降级:`@supports not (backdrop-filter: blur(1px))` 回退纯色

### 4.2 组件层(SpotlightPanel + ClipboardList)

**SpotlightPanel.tsx**:
- 顶栏:`border-b` 硬线 → 半透明渐变蒙版(苹果滚动边缘效果)
- 底栏:同样改渐变蒙版
- 状态指示灯(●):加轻微 pulse 动效(罕见/装饰场景,允许)

**ClipboardList.tsx**:
- 列表项选中:方块 `bg-accent` → 圆角胶囊 `rounded-md` + 半透明强调色
- 列表项 hover:用过渡而非瞬切,`transition: background 150ms var(--ease-out)`
- 列表项点击:加 `:active { transform: scale(0.98) }` 按压反馈(emilkowalski 组件原则)
- 时间戳/计数:套 `letter-spacing` 字距分级(§2.6)

### 4.3 动效层(严格按频率闸门,克制为主)

> ⚠️ 这是高频常驻工具。**默认是"不加动画"**,只在以下场景加:

| 场景 | 动效 | 频率档 | 裁决 |
|------|------|-------|------|
| 列表上下导航 | 无 | 100+/天 | ❌ 不加 |
| M3 全局唤起 | 无 | 100+/天 | ❌ 不加(对标 Raycast) |
| 列表项 hover | 背景过渡 150ms | 几十/天 | ✅ 加,极简 |
| 列表项点击 | scale(0.98) 按压 | 几十/天 | ✅ 加,极简 |
| 列表新增项进入 | fade + slide | 偶尔 | ✅ 加,@starting-style |
| 错误提示出现 | fade | 偶尔 | ✅ 加 |
| 空状态(首次) | 轻微入场 | 罕见/首次 | ✅ 允许惊喜 |

**emilkowalski 硬规则清单(Never Ship)**,改造后自检:
- [ ] 无 `transition: all`(必须指明属性)
- [ ] 无 `scale(0)` 入场(从 `scale(0.95)` + opacity 起)
- [ ] 无 `ease-in` 在 UI 上
- [ ] 无键盘触发动画
- [ ] UI 时长不超 300ms
- [ ] popover 的 `transform-origin` 在触发点(本项目暂无 popover,留意)
- [ ] 高频触发的元素不用 keyframes(用 transition)
- [ ] 只动画 `transform` / `opacity`(不动画 width/height/margin)
- [ ] hover 动效用 `@media (hover: hover) and (pointer: fine)` 门控
- [ ] 动画配 `prefers-reduced-motion` 降级

### 4.4 滚动条美化(苹果隐形滚动条)

```css
/* 列表区:细滚动条,hover 时显形,平时半透明 */
.clipboard-list::-webkit-scrollbar { width: 6px; }
.clipboard-list::-webkit-scrollbar-thumb {
  background: hsl(var(--muted-foreground) / 0.2);
  border-radius: 3px;
}
.clipboard-list::-webkit-scrollbar-track { background: transparent; }
```

## §5 施工步骤(M2 完成后执行)

> 按模块化 + 随写随测纪律,分块提交,每块测过再进下一块。

1. **Token 层** — `index.css` 加 §2 全部 token,`tailwind.config.ts` 接线。`npm build` 过。
2. **毛玻璃外壳** — SpotlightPanel 根容器改毛玻璃,实测 macOS 上透明 + 模糊生效。
3. **配色替换** — 换苹果系统色,亮/暗模式都校对。
4. **列表项重做** — 胶囊选中 + hover 过渡 + 按压反馈 + 字距分级。
5. **顶/底栏** — 硬边框改渐变蒙版。
6. **动效(克制)** — 按 §4.3 表,只加获批的几项。每项配 reduced-motion 降级。
7. **滚动条** — 隐形细滚动条。
8. **自检** — 跑 §4.3 的 emilkowalski Never Ship 清单 + `review-animations` 标准。
9. **实测** — macOS 实机看毛玻璃质感(WebView2 与 macOS WKWebView 表现不同,需实机)。

## §6 不做的事(克制)

- ❌ 不引入 Motion/Framer Motion(项目动效极少,CSS transition 够用,符合 emilkowalski「最便宜够用的工具」原则)
- ❌ 不做弹簧物理动画(无手势拖拽场景,M3 才可能有,届时再评估)
- ❌ 不给高频操作加任何动画(核心频率闸门)
- ❌ 不照搬 apple-design 的 haptics / 原生材料(Web 平台只是近似)

## §7 参考(摘录精华,详情查 skills 原文)

> emilkowalski/skills 已 clone 到本地 `/tmp/emil-skills`(会话级,不持久)。
> 永久源:https://github.com/emilkowalski/skills

- **animate/SKILL.md** — 造动画的 7 步决策顺序(频率→目的→工具→属性→曲线→中断→降级)
- **review-animations/STANDARDS.md** — 完整值表(曲线/时长/弹簧/手势/性能/a11y)
- **apple-design/SKILL.md** — 苹果 16 节设计原则(响应/直接操作/可中断/弹簧/动量/空间一致/橡皮筋/材质/字体)
- **improve-animations** 的「决策/执行分离」方法论 —— 强模型审计+写自包含计划,弱模型执行
