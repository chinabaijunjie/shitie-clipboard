---
title: M1 规划 — macOS 剪贴板监听 MVP
tags: [M1, 规划, 剪贴板, 监听, rusqlite, 历史列表, 回填]
created: 2026-08-05
updated: 2026-08-05
status: active
tech_base: ./m1-research.md
---

# M1 规划:macOS 剪贴板监听 MVP

> 技术依据见 [[m1-research]](三块调研 + crate 选型 + spike 验证,均已完成)。
> 本文档是 M1 施工蓝图,实现阶段严格按此执行(§核心原则 3)。

---

## 一、M1 范围边界

**M1 目标闭环**:复制文字 → 历史列表实时出现新条目 → 上下键选中 → 回车回填到剪贴板。

**M1 做**:
- macOS 剪贴板监听(clipboard-rs watcher,后台线程,500ms 轮询)
- **纯文本**存储(rusqlite,items + formats 两表,但只写 text 格式)
- 历史列表 UI(上下键导航 + 选中高亮)
- 回填(选中条目 → `set_text` 写回剪贴板)
- 去重(相同文本不重复入库,更新 `last_copied_at` + `copy_count`)

**M1 不做**(留给后续里程碑):
- ❌ 富文本 / 图片 / 文件 URL 的存储与回填(M2)
- ❌ 搜索(M2,SQLite FTS5)
- ❌ 全局快捷键唤起 / 托盘 / 自启(M3)—— 窗口仍用临时 visible:true
- ❌ Windows 平台(M3)
- ❌ 容量上限 / 敏感过滤(M4)

**判据**:复制 3 段不同文字 → 列表出现 3 条 → 上下键选中某条 → 回车 → ⌘V 粘贴出该条内容。
复制相同文字 → 列表不新增,已有条目更新时间 + 次数。

---

## 二、架构设计

### 进程模型(纯 Tauri 单进程)

```
┌─────────────────────────────────────────────┐
│  Tauri 主进程(Rust)                        │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐  │
│  │ clipboard    │    │  AppState        │  │
│  │ watcher 线程 │───▶│  - watcher_stop  │  │
│  │ (500ms 轮询) │    │  - db: Mutex<Conn>│  │
│  └──────────────┘    └──────────────────┘  │
│         │ changeCount 变化                  │
│         ▼                                   │
│  读内容 → 写 DB → emit("clipboard://added")│
│                                  │          │
│  ┌──────────────┐                 │          │
│  │ commands     │                 │          │
│  │ list_items   │◀── invoke ──────┼──────────┼──┐
│  │ copy_back    │                 │          │  │
│  └──────────────┘                 ▼          │  │
└─────────────────────────────────────────────┘  │
                                   │              │
                    ┌──────────────▼──────────────▼──┐
                    │  WebView(React)               │
                    │  SpotlightPanel                │
                    │  - listen("clipboard://added") │
                    │    → 刷新列表                  │
                    │  - invoke('list_items')        │
                    │  - 上下键导航 + 回车回填       │
                    └────────────────────────────────┘
```

### 监听架构(spike 已验证可行)

- **后台线程**:`std::thread::spawn` 起专用线程跑 `ClipboardWatcherContext::start_watch`
- **间隔**:500ms(spike 用值,Maccy/clipboard-master 共识)
- **读内容时机**:`on_clipboard_change` 回调里读(spike 验证后台线程读 NSPasteboard 不崩溃)
- **⚠️ 容错**:`get_text()` 在无文本格式时会报错(实测),必须先 `available_formats()` 判断含 `public.utf8-plain-text` 再读,失败则跳过(非文本内容 M1 不处理)
- **停止**:`WatcherShutdown` 存入 AppState,`RunEvent::Exit` 时 drop

### 数据流

1. 用户复制 → changeCount+1 → watcher 线程 `on_clipboard_change`
2. watcher 读文本 → 写 DB(items + formats 两表)→ `app.emit("clipboard://added", 新条目)`
3. 前端 `listen("clipboard://added")` → 追加到列表顶部
4. 用户上下键选中 + 回车 → `invoke('copy_back', {id})` → Rust 读 DB → `set_text` → 关闭窗口(可选)

---

## 三、数据模型

### 数据库表结构(照搬 m1-research.md,M1 只写 text)

```sql
-- 见 m1-research.md 第三节完整 DDL,M1 实现时:
-- clipboard_items:正常使用所有字段,content_kind 固定 'text'
-- clipboard_formats:每条 item 写一行,uti='public.utf8-plain-text',data_blob=UTF-8 字节

PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;  -- constraints.md SQLite PRAGMA 红线
```

**去重逻辑**:写入前用文本 hash(SHA-256,存 title 之外)查询是否存在;
存在则 `UPDATE last_copied_at = now, copy_count += 1`;不存在则 INSERT。
> M1 用文本内容直接比对即可(纯文本场景),hash 优化 M4 再加。

### Rust 类型(两端镜像)

```rust
// Rust: src-tauri/src/models.rs
#[derive(Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub first_copied_at: i64,   // unix epoch ms
    pub last_copied_at: i64,
    pub copy_count: i32,
    pub title: String,          // 列表预览(前 1000 字符)
    pub content_kind: String,   // M1 固定 "text"
}
// M1 不把 formats 数组返回前端(只有 text,直接在 copy_back 时读 DB)
```

```ts
// 前端: src/core/types/models.ts
export interface ClipboardItem {
  id: number
  firstCopiedAt: number
  lastCopiedAt: number
  copyCount: number
  title: string
  contentKind: 'text'  // M1 联合类型,M2 扩展为 'text'|'image'|'file'|'mixed'
}
```

---

## 四、Command 接口设计

| command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `list_items` | `limit?: number`(默认 50) | `ClipboardItem[]` | 按 last_copied_at DESC 查列表 |
| `copy_back` | `id: number` | `void` | 读 DB → `set_text` 写回剪贴板 |
| `delete_item` | `id: number` | `void` | 删除单条(M1 可选,删除触发 ON DELETE CASCADE 清 formats) |
| `ping` | — | `"ok"` | 保留(M0 既有) |

**事件**:
- `clipboard://added` —— payload: `ClipboardItem`(新增或更新去重的条目)

**错误处理**:
- 读剪贴板失败 → `AppError::new("CLIPBOARD_READ_FAILED", ...)`(error.rs 新增错误码)
- DB 操作失败 → `AppError::internal(...)`
- ⚠️ 错误码两端同步(constraints.md 镜像契约):Rust `codes` + 前端 `InvokeErrorCode`

---

## 五、UI 交互(SpotlightPanel 改造)

### 布局

```
┌──────────────────────────────────┐
│  Clipboard Manager        ● 就绪 │  ← 顶栏(标题 + 状态)
├──────────────────────────────────┤
│ ▸ 第一条复制内容预览        14:32 │  ← 选中(高亮背景)
│   第二条复制内容预览        14:30 │
│   第三条...                 14:28 │
│                                  │
├──────────────────────────────────┤
│  ↑↓ 导航 · ⏎ 回填 · ⌫ 删除       │  ← 底栏(快捷键提示)
└──────────────────────────────────┘
```

### 键盘交互

| 键 | 行为 |
|----|------|
| `↑` / `↓` | 上下移动选中条目(循环:末尾按↓回顶部) |
| `Enter` | 回填选中条目 → `invoke('copy_back')` → 可选关闭窗口(M1 先不关) |
| `Delete`/`Backspace` | 删除选中条目(M1 可选) |
| `Esc` | 清空选中 / 隐藏窗口(M1 先不实现隐藏) |

### React state(zustand 或 useState,M1 列表简单用 useState 即可)

- `items: ClipboardItem[]` —— 列表数据
- `selectedIndex: number` —— 当前选中索引
- 初次 mount:`invoke('list_items')` 加载历史
- `listen('clipboard://added')` → 把新条目 unshift 到 items 顶部
- ⚠️ 不可变性:列表更新用展开运算符,不 mutate(AGENTS §8)

---

## 六、资产处置清单

### ✂️ 删除(spike 临时代码)

| 资产 | 处置 |
|------|------|
| `src-tauri/src/spike_clipboard.rs` | 删(spike 完成,演化进正式 clipboard 模块) |
| `src/components/SpikeClipboardProbe.tsx` | 删 |
| SpotlightPanel 里的 SpikeClipboardProbe 引用 | 删 |

### 🔧 改造

| 资产 | 改造为 |
|------|--------|
| `src-tauri/Cargo.toml` | 加 `rusqlite` + `refinery`(或 SQLx migrations);clipboard-rs 保留(spike 已加) |
| `src-tauri/src/lib.rs` | AppState 加 `db: Mutex<Connection>`;setup 初始化 DB + watcher;RunEvent::Exit 停 watcher + 关 DB |
| `src-tauri/src/error.rs` | codes 加 `CLIPBOARD_READ_FAILED` / `DATABASE_ERROR` |
| `src/core/api/invoke.ts` | InvokeErrorCode 加对应错误码(镜像) |
| `src/core/types/models.ts` | 定义 `ClipboardItem` 接口 |
| `src/components/SpotlightPanel.tsx` | 改造成历史列表(上下键 + 回填),删 spike 引用 |

### 📦 新增

| 新增 | 用途 |
|------|------|
| `src-tauri/src/clipboard/` | 剪贴板模块(watcher 启停 + 读内容 + 写 DB) |
| `src-tauri/src/storage/` | 存储模块(DB 初始化 + migrations + items/formats CRUD) |
| `src-tauri/src/models.rs` | ClipboardItem 等 Rust 类型 |
| `src-tauri/src/commands/clipboard.rs` | list_items / copy_back / delete_item |
| `src-tauri/migrations/` | refinery SQL migrations(建表) |

---

## 七、施工阶段(每阶段一个 commit,完成后汇报确认)

### 阶段 A:存储层(DB + models)(中风险)
1. 加 rusqlite + refinery 依赖
2. 写 migrations(建 clipboard_items + clipboard_formats 两表 + PRAGMA)
3. `storage/mod.rs`:DB 初始化(开 PRAGMA)+ Connection 封装
4. `models.rs`:ClipboardItem Rust 类型
5. `storage/items.rs`:list / insert / update / delete CRUD
6. **验证**:`cargo test`(写个内存 DB 单测:insert → list → 去重 → delete)
7. commit: `feat(storage): rusqlite 存储层 + items/formats 两表 + CRUD 单测`

### 阶段 B:剪贴板监听层(中风险)
8. `clipboard/mod.rs`:watcher 启停(基于 spike 验证结论,后台线程)
9. `clipboard/handler.rs`:on_clipboard_change → 读文本(容错)→ 写 DB → emit 前端
10. 改 lib.rs:AppState 加 db + watcher_shutdown;setup 初始化;RunEvent::Exit 清理
11. 删 spike_clipboard.rs + 前端 SpikeClipboardProbe.tsx
12. **验证**:`cargo check` 零 warning + tauri dev 启动 + pbcopy 测试 → 日志见写入 + 前端收到事件
13. commit: `feat(clipboard): 监听层(clipboard-rs watcher → DB → emit 前端)`

### 阶段 C:Command 接口层(低风险)
14. `commands/clipboard.rs`:list_items / copy_back / delete_item
15. 改 error.rs + 前端 invoke.ts 错误码镜像
16. 注册 commands 到 generate_handler!
17. **验证**:DevTools Console 测 `invoke('list_items')` / `invoke('copy_back',{id:1})`
18. commit: `feat(commands): list_items / copy_back / delete_item`

### 阶段 D:前端历史列表 UI(中风险)
19. `core/types/models.ts`:ClipboardItem 接口
20. 改 SpotlightPanel:列表渲染 + 上下键导航 + 选中高亮 + 回车回填
21. listen('clipboard://added') → 列表实时更新
22. 删 spike 引用 + 探活逻辑(改由列表加载判断就绪)
23. **验证**:`npm run build` + `npm test` + `npm run test:typecheck` 全过
24. commit: `feat(frontend): SpotlightPanel 改造为历史列表(上下键 + 回填)`

### 阶段 E:端到端实测(关键,§1.5)
25. tauri dev 启动 → 复制 3 段文字 → 列表出现 3 条 → 上下键 → 回车 → ⌘V 验证回填
26. 测试去重:复制相同文字 → 列表不新增,时间更新
27. 测试容错:复制文件(Finder)→ 不崩溃,不入库(无文本格式)
28. commit(如有修正): `fix: M1 端到端实测修正`

---

## 八、必踩坑提醒(M1 执行红线)

1. **SQLite PRAGMA**(constraints.md):连接必须 `foreign_keys=ON` + `WAL` + `busy_timeout=5000`
2. **get_text 容错**(spike 实测):无文本格式时报错,必须先 `available_formats()` 判断
3. **多 UTI 去重**(spike 实测):纯文本复制带 `public.utf8-plain-text` + `NSStringPboardType`,
   白名单只存前者,避免 formats 表写两行
4. **Tauri setup 时序坑**(constraints.md):不在 setup emit;watcher 回调里 emit(那时 WebView 已就绪)
5. **文件 IO 用 spawn_blocking**(constraints.md):M1 暂无文件 IO(图片 M2 才有),但 DB 连接操作
   要注意——rusqlite 是同步的,command handler 里直接调即可(command 跑 tokio 线程),但**不要**
   在 async 任务里长时间持锁 DB
6. **不可变性**(AGENTS §8):前端列表 state 展开运算符更新
7. **错误处理**(AGENTS §8):watcher 里的错误 log 后继续(不崩监听线程);command 错误返回 AppError
8. **tokio features**:不引 `fs`(M1 无文件 IO)

---

## 九、红线:什么不做

- ❌ M1 不存富文本/图片/文件(数据模型预留 formats 表,但只写 text)
- ❌ M1 不实现全局快捷键 / 托盘(窗口用 visible:true,M3 做唤起)
- ❌ M1 不做搜索(M2)
- ❌ M1 不做容量上限(M4)
- ❌ spike 代码不进 M1 主干(演化重写,不留临时代码)

---

## 十、验证流程

```bash
cd /Users/baijunjie/Documents/infra/clipboard-manager

# 阶段 A 末
cd src-tauri && cargo test && cd ..

# 阶段 B/C 末
cd src-tauri && cargo check && cd ..
npm run tauri dev
# DevTools: 复制文字 → 看日志 [clipboard] 写入 → 看前端收到事件

# 阶段 D 末
npm run build && npm run test:typecheck && npm test

# 阶段 E(端到端)
npm run tauri dev
# 复制 → 列表 → 上下键 → 回车 → ⌘V 粘贴验证
```

---

## 十一、执行模式

- 每阶段完成后 **commit + 汇报**,等用户确认再进下一阶段
- 阶段内自主执行,不再逐文件确认
- 遇到未预见的决策点(如 refinery vs 手写 migration、zustand vs useState)停下问用户

---

_本文档 M1 完成后标记 `status: done`,关键决策沉淀到 constraints.md / decision-cases.md。_
