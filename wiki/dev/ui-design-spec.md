---
title: UI 设计规格(M4 UI 改造)
tags: [UI, 设计, 规格, 主题, 配色]
created: 2026-08-09
updated: 2026-08-09
status: active
---

# UI 设计规格

> 本文件是 **M4 UI 改造的施工依据** + **多主题设计的完整资产存档**。
>
> - 本次改造:深色毛玻璃(① 号方案)作为第一版主方向
> - 未来扩展:浅色系统(②)/ 深灰折中(③)已预设计,用户主题切换功能验证通过后加入
>
> 配套设计稿(HTML,浏览器直接打开看效果):
> `design-mockups/index.html` → 三份风格导览页

---

## §0 决策记录

| 时间 | 决策 | 依据 |
|------|------|------|
| 2026-08-09 | 三份设计稿完成(深色毛玻璃/浅色系统/深灰折中) | 用户要求可视化对比,布局相同配色不同 |
| 2026-08-09 | **深色 ① 作为第一版主方向** | 剪贴板工具主流形态(Raycast/Maccy/Alfred 均深色),最稳选择 |
| 2026-08-09 | 不引入 shadcn/ui,继续手写精雕 | 用户决策,零新依赖,完全可控 |
| 2026-08-09 | 图标库用 lucide-react | 矢量图标,苹果风格,按需引入体积可控 |
| 2026-08-09 | 主面板 + 设置面板全改 | 用户决策,体验一致性 |
| 2026-08-09 | 列表项预留类型图标位置(text/image/file/code/md) | 为未来「按格式智能渲染」铺路,本次不实现检测逻辑 |
| 2026-08-09 | **多主题切换不在第一版做** | 三倍维护成本 + 高频工具追求无感;架构预留,验证后按需加 |
| 2026-08-09 | **多主题切换已落地**(深色/浅色/折中) | UI 稳定后用户要求加入。三套配色从预设计转正式。启动防闪 localStorage 缓存优先 + 分段按钮选择器 |

---

## §1 设计稿资产

三份 HTML 设计稿在 `design-mockups/` 目录:

| 文件 | 风格 | 状态 |
|------|------|------|
| `index.html` | 导览页(三份风格入口) | ✅ |
| `01-dark-glass.html` | ① 深色毛玻璃(Raycast 风) | ✅ **第一版主方向** |
| `02-light-system.html` | ② 浅色系统(macOS 设置风) | ✅ 已落地(.theme-light) |
| `03-mid-balance.html` | ③ 深灰折中(暖橙强调) | ✅ 已落地(.theme-mid) |

每份设计稿内部含 **主面板 / 空状态 / 设置面板** 三个标签视图,可切换查看。

---

## §2 设计 Token(三套配色完整存档)

> 这是整个 UI 的核心资产。所有配色用 CSS 变量,三套主题 = 三套变量值。
> 本次改造只落地「深色 ①」,但三套值都存档在此,未来加主题切换时直接抄。

### 2.1 共享 Token(三套主题通用,不随配色变)

```css
/* 缓动曲线(emilkowalski 核心纪律:绝不近似) */
--ease-out: cubic-bezier(0.23, 1, 0.32, 1);        /* 强 ease-out,UI 进入/退出默认 */
--ease-in-out: cubic-bezier(0.77, 0, 0.175, 1);    /* 强 ease-in-out,屏内移动 */
--ease-drawer: cubic-bezier(0.32, 0.72, 0, 1);     /* iOS 风格抽屉曲线 */

/* 时长 */
--duration-fast: 150ms;     /* hover / 按压反馈 */
--duration-base: 200ms;     /* toggle / 状态切换 */
--duration-slow: 300ms;     /* 上限,UI 动画不超此值 */

/* 圆角 */
--radius-window: 12px;      /* 窗口本身 */
--radius-card: 8px;         /* 列表项选中胶囊 / 搜索框 */
--radius-small: 6px;        /* 图标按钮 / 小标签 */
--radius-thumb: 5px;        /* 缩略图 */
```

### 2.2 阴影(三套主题通用结构,色值随主题微调)

```css
/* 窗口阴影:多层半透明(苹果质感核心 —— 不是单层黑) */
/* ① 深色用 */
--shadow-window: 0 22px 70px 4px rgba(0,0,0,0.45),
                 0 8px 24px rgba(0,0,0,0.18),
                 inset 0 0.5px 0 rgba(255,255,255,0.12);  /* 顶边高光 */

/* ② 浅色用 */
--shadow-window: 0 22px 70px 4px rgba(0,0,0,0.14),
                 0 8px 24px rgba(0,0,0,0.06),
                 inset 0 0.5px 0 rgba(255,255,255,0.6);   /* 顶边高光(白) */

/* ③ 深灰折中用 */
--shadow-window: 0 22px 70px 4px rgba(0,0,0,0.4),
                 0 8px 24px rgba(0,0,0,0.16),
                 inset 0 0.5px 0 rgba(255,255,255,0.08);
```

---

## §3 配色变量(三套完整值)

### 3.1 ① 深色毛玻璃(第一版主方向)

> Raycast / Spotlight 风格。深灰蓝半透明 + 强模糊 + 蓝色强调。

```css
/* 根容器毛玻璃 */
--window-bg: rgba(28, 28, 30, 0.72);       /* #1C1C1E 72% 不透明 */
--window-blur: blur(30px) saturate(180%);

/* 背景层 */
--bg-hover: rgba(255, 255, 255, 0.06);     /* hover 态 */
--bg-selected: rgba(10, 132, 255, 0.18);   /* 选中:蓝色半透明胶囊 */

/* 强调色 */
--accent: #0A84FF;                         /* iOS systemBlue(暗色版) */
--accent-text: #0A84FF;

/* 文字(苹果 systemLabel 三级) */
--text: #FFFFFF;                                    /* primary */
--text-2: rgba(235, 235, 245, 0.6);                 /* secondary */
--text-3: rgba(235, 235, 245, 0.3);                 /* tertiary */

/* 分隔线 */
--sep: rgba(84, 84, 88, 0.36);             /* separator */

/* 状态点 */
--status-ready: #0A84FF;                   /* 带 box-shadow 发光 */

/* 徽标(×N 复制次数) */
--badge-bg: rgba(255, 255, 255, 0.08);
--badge-text: rgba(235, 235, 245, 0.6);

/* 类型图标颜色(预留位) */
--type-code: #FFD60A;     /* 黄 */
--type-md: #64D2FF;       /* 青 */
--type-file: #30D158;     /* 绿 */
/* text 类型无图标(透明占位) */
```

### 3.2 ② 浅色系统(预设计,未来主题切换用)

> macOS 系统设置 / iOS 风格。半透明白卡片 + 蓝色强调。

```css
--window-bg: rgba(255, 255, 255, 0.82);    /* 白 82% 不透明 */
--window-blur: blur(30px) saturate(180%);

--bg-hover: rgba(0, 0, 0, 0.04);
--bg-selected: rgba(0, 122, 255, 0.10);    /* 选中:浅蓝半透明 */

--accent: #007AFF;                         /* iOS systemBlue(亮色版) */
--accent-text: #007AFF;

--text: #1D1D1F;
--text-2: #86868B;
--text-3: #AEAEB2;

--sep: rgba(60, 60, 67, 0.12);
--sep-strong: rgba(60, 60, 67, 0.2);

--status-ready: #007AFF;

--badge-bg: rgba(60, 60, 67, 0.12);
--badge-text: #86868B;

--type-code: #FF9F0A;
--type-md: #30B0C7;
--type-file: #34C759;
```

### 3.3 ③ 深灰折中(预设计,未来主题切换用)

> 介于深浅之间。中性暖灰 + 橙色强调(区别于前两份的蓝)。

```css
--window-bg: rgba(42, 42, 45, 0.78);       /* #2A2A2D 78% 不透明 */
--window-blur: blur(28px) saturate(150%);

--bg-hover: rgba(255, 255, 255, 0.05);
--bg-selected: rgba(255, 159, 10, 0.16);   /* 选中:暖橙半透明胶囊 */

--accent: #FF9F0A;                         /* iOS systemOrange */
--accent-text: #FF9F0A;

--text: #F5F5F7;
--text-2: rgba(245, 245, 247, 0.55);
--text-3: rgba(245, 245, 247, 0.3);

--sep: rgba(255, 255, 255, 0.08);

--status-ready: #FF9F0A;                   /* 带 box-shadow 发光 */

--badge-bg: rgba(255, 255, 255, 0.08);
--badge-text: rgba(245, 245, 247, 0.55);

--type-code: #FFD60A;
--type-md: #64D2FF;
--type-file: #30D158;
```

---

## §4 主题切换架构(✅ 已实现,2026-08-09)

> 三套主题(深色/浅色/折中)已全部落地。以下架构在实现时遵循并补充了实际决策。

### 4.1 CSS 层:主题 class

三套配色用根元素的 class 切换(`.theme-dark` / `.theme-light` / `.theme-mid`)。
**不沿用当前的 `.dark`**(那是 shadcn 约定,语义是暗色模式,不是主题)。

```css
/* index.css */
:root,
.theme-dark {
  /* §3.1 深色 ① 全部变量 */
}
.theme-light {
  /* §3.2 浅色 ② 全部变量 */
}
.theme-mid {
  /* §3.3 折中 ③ 全部变量 */
}
```

### 4.2 设置层:Settings 加 theme 字段

```rust
// src-tauri/src/settings/mod.rs
pub struct Settings {
    // ... 现有字段
    #[serde(default = "default_theme")]
    pub theme: String,           // "dark" | "light" | "mid"
}
fn default_theme() -> String { "dark".to_string() }
```

```ts
// 前端 Zod schema + SettingsPanel 加主题选择器
// main.tsx 启动时读 settings.theme → 在 <html> 加 .theme-{theme} class
```

### 4.3 切换交互(✅ 已实现)

设置面板「通用」组顶部第一项:
- 标签:外观
- 描述:选择界面配色风格
- 控件:**分段按钮**(ThemeSwitcher 组件,macOS 系统设置风格),三选一(深色 / 浅色 / 折中)
- 即时生效 + 即时保存(save → update_settings)

切换实现:`document.documentElement.className = 'theme-' + theme + ' dark'`,无闪烁(变量级切换,不需重载)。
启动防闪:`main.tsx` 同步读 localStorage 缓存(`getInitialTheme`),settings 加载后纠正(`App.tsx` useEffect → `applyTheme`)。

---

## §5 组件视觉规格(本次改造依据)

> 以下是深色 ① 的具体组件规格,施工时逐项对照。
> 设计稿 `01-dark-glass.html` 是可视化参考。

### 5.1 窗口根容器(SpotlightPanel / SettingsPanel 共用)

```css
width: 600px; height: 480px;        /* tauri.conf.json 固定 */
border-radius: var(--radius-window); /* 12px */
background: var(--window-bg);        /* rgba(28,28,30,0.72) */
backdrop-filter: var(--window-blur); /* blur(30px) saturate(180%) */
box-shadow: var(--shadow-window);    /* 多层带顶边高光 */
color: var(--text);
overflow: hidden;
display: flex; flex-direction: column;
```

**非 macOS 降级**(Windows 不一定支持 backdrop-filter):
```css
@supports not (backdrop-filter: blur(1px)) {
  .window { background: #1C1C1E; }  /* 回退纯色 */
}
```

### 5.2 顶栏

- 布局:`flex justify-between`,padding `10px 14px`
- **无硬边框** —— 用渐变蒙版过渡:`-webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - 1px), transparent)`
- 标题:12px / 600 字重 / letter-spacing 0.01em
- 设置图标:lucide `Settings` 图标,14px,颜色 `--text-2`,hover 变 `--text` + 背景高亮
- 状态点:7×7px 圆点,`--accent` 色,`box-shadow: 0 0 6px` 发光

### 5.3 搜索框

- padding `7px 14px 8px`
- 搜索图标:lucide `Search`,13px,颜色 `--text-3`
- input:13px,透明背景,placeholder 颜色 `--text-3`
- 结果计数:10px,颜色 `--text-3`

### 5.4 列表项(ClipboardList)

```css
/* 容器 */
.list { flex: 1; overflow-y: auto; padding: 4px 8px 8px; }

/* 单项 */
.item {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 10px;
  border-radius: var(--radius-card);  /* 8px 胶囊 */
  transition: background var(--duration-fast) var(--ease-out);
}
.item:hover { background: var(--bg-hover); }
.item:active { transform: scale(0.985); }     /* 按压反馈,极轻微 */
.item.selected {
  background: var(--bg-selected);
  box-shadow: inset 0 0.5px 0 rgba(255,255,255,0.08);
}
```

**列表项内部结构**(从左到右):
1. **类型图标**:16×16px,lucide 图标 14px(预留位,见 §6)
2. **缩略图**(仅图片):26×26px,`border-radius: var(--radius-thumb)`,替代类型图标位
3. **内容**:`flex: 1`,title 12.5px / sub 11px,均 `text-overflow: ellipsis` 截断
4. **徽标**(仅 ×N):10px,`border-radius: 10px` 胶囊
5. **时间**:10px,letter-spacing 0.01em

### 5.5 空状态

- 居中 flex 布局,gap 10px
- 图标:lucide `Clipboard`,40px,opacity 0.4
- 标题:13px,`--text-2`
- 描述:11px,`--text-3`

### 5.6 底栏

- padding `8px 14px`
- **无硬边框** —— 渐变蒙版
- 快捷键提示:10px,每个按键用 `<kbd>` 包裹(9.5px / `border-radius: 4px` / `--badge-bg` 背景)

### 5.7 设置面板

- 顶栏:返回按钮(← lucide `ArrowLeft` 14px,`--accent` 色)+ 标题 + 占位
- 列表分组:组标题 10px / 600 / `--text-3` / 大写 / letter-spacing 0.05em
- 设置行:padding `11px 4px`,底部 0.5px `--sep` 分隔线
- **Toggle**:36×20px,`--accent` 开启色,圆点 17px 带 box-shadow
- **Select**:原生 `<select>`,12px,透明背景
- **快捷键显示**:11px / padding `3px 8px` / `--badge-bg` 背景 / 6px 圆角

### 5.8 滚动条(隐形细滚动条)

```css
.list::-webkit-scrollbar,
.settings-list::-webkit-scrollbar { width: 6px; }
/* 深色 ① */
::-webkit-scrollbar-thumb { background: rgba(235,235,245,0.15); border-radius: 3px; }
::-webkit-scrollbar-track { background: transparent; }
```

---

## §6 类型图标预留位(为未来智能渲染铺路)

> 本次只预留图标位置 + 现有三类(text/image/file)。
> code / md 的智能检测留后续独立迭代(见 §7)。

| 类型 | 判断条件 | 图标 | 颜色 | 本次实现 |
|------|---------|------|------|---------|
| 纯文字 | 只有 utf8-plain-text | 无(透明占位,保持对齐) | — | ✅ |
| 图片 | 有 thumbPath | 显示缩略图(替代图标位) | — | ✅ |
| 文件 | public.file-url | lucide `FileText` | `--type-file` 绿 | ✅ |
| 代码 | ???(待检测逻辑) | lucide `Code` `</>` | `--type-code` 黄 | ⏳ 预留,不实现 |
| Markdown | ???(待检测逻辑) | lucide `FileCode` | `--type-md` 青 | ⏳ 预留,不实现 |

**预留方式**:列表项左侧固定 16px 宽的 `.type-icon` 位,纯文字时 `opacity: 0` 占位(保持对齐),未来加 code/md 时只需填入图标 + 改 opacity。

---

## §7 不做的事(克制)

- ~~❌ **多主题切换不在第一版做**~~ → ✅ **已实现**(2026-08-09,三套主题全部落地)
- ❌ **代码/Markdown 智能检测不在本次做** —— 预留图标位(§6),独立迭代
- ❌ **不引入 shadcn/ui** —— 手写精雕,零新依赖
- ❌ **不引入 Motion/Framer Motion** —— CSS transition 够用
- ❌ **不给高频操作加动画** —— 核心频率闸门(上下导航 / 全局唤起 无动画)

---

## §8 与 ui-redesign-plan.md 的关系

`ui-redesign-plan.md` 是**方法论 + 设计原则来源**(emilkowalski/skills 规则、频率闸门、Never Ship 清单)。
本文件是**具体施工规格**(配色值、组件尺寸、变量定义)。

两者互补:
- 改造时**施工依据** → 本文件 §3 + §5
- 改造后**自检清单** → ui-redesign-plan.md §4.3 + §5.8
