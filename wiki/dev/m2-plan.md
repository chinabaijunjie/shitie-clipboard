---
title: M2 规划 — 搜索 + 富文本/图片支持
tags: [M2, 规划, FTS5, 搜索, 富文本, 图片, 多格式, 回填]
created: 2026-08-06
updated: 2026-08-06
status: active
tech_base: ./m2-research.md
---

# M2 规划:搜索 + 富文本/图片支持

> 技术依据见 [[m2-research]](三块调研 + FTS5+bigram spike 验证,均已完成)。
> 本文档是 M2 施工蓝图,实现阶段严格按此执行(§核心原则 3)。
>
> **用户决策**(2026-08-06):
> - migration:手写版本号(`PRAGMA user_version`),不引 rusqlite_migration
> - 施工顺序:多格式捕获打底 → 搜索 → 图片渲染
> - 范围:全做(搜索 + 富文本 RTF/HTML + 图片 + 文件 URL)

---

## 一、M2 范围边界

**M2 目标闭环**:
1. 复制富文本/图片/文件 → 按 contentKind 分类正确捕获存储 → 列表按类型渲染(文本/图片缩略图)→ 回填保留原格式
2. 顶部搜索框输入 → 实时 FTS5 + bigram 中文搜索 → 高亮匹配结果

**M2 做**:
- **多格式捕获**:白名单(text/rtf/html/png/tiff/file-url)+ 隐私黑名单(`ConcealedType`/`TransientType` 整条跳过)+ 噪音过滤(dyn.*/linkpresentation 等)
- **多格式存储**:文本/富文本进 BLOB,图片进文件系统(sha256 分桶 + 引用计数删除)
- **多格式回填**:`set(Vec<ClipboardContent>)` 原子写全部格式(保留 Word 富文本格式)
- **搜索**:SQLite FTS5 外部内容表 + 应用层 bigram 预处理(已 spike 验证)+ 单字 LIKE 兜底 + BM25 排序
- **前端**:按 contentKind 分支渲染(文本/图片缩略图)+ 搜索框 + asset protocol 显示磁盘图片
- **schema migration**:手写版本号(`PRAGMA user_version`)管理加列

**M2 不做**(留给后续里程碑):
- ❌ 全局快捷键唤起 / 托盘 / 自启(M3)—— 窗口仍 visible:true
- ❌ Windows 平台(M3)
- ❌ 容量上限 / 敏感过滤(M4)
- ❌ jieba 分词升级(P1,等用户反馈再上)
- ❌ 图片 OCR 提取文字做标题(M3+)
- ❌ 密码管理器浏览器扩展盲区(无法解决,README 告知)

**判据**:
1. 从 Word 复制富文本 → 列表出现 → 回填 → 粘贴到 Word **格式不丢**
2. 系统截图 → 列表出现缩略图(「图片 1920×1080」)→ 回填 → 粘贴到聊天工具出图
3. Finder 复制文件 → 列表出现(文件名)→ 回填 → 粘贴出文件
4. 搜「你好」→ 命中含「你好」的历史(含「你好世界」)→ 搜「async」命中代码片段
5. 1Password **桌面 app** 复制密码 → 不入库(隐私跳过生效)

---

## 二、架构设计

### M2 在 M1 架构上的增量(粗体为新增/改动)

```
┌──────────────────────────────────────────────────────────┐
│  Tauri 主进程(Rust)                                     │
│                                                          │
│  ┌──────────────────┐         ┌────────────────────┐    │
│  │ clipboard watcher│         │ AppState           │    │
│  │ (500ms 轮询)     │         │  - db: Mutex<Conn> │    │
│  │                  │         │  - self_write_flag │    │
│  │ on_clipboard_    │         │  - watcher_stop    │    │
│  │   change         │         │  ┌──────────────┐  │    │
│  │  ↓               │         │  │ images_dir   │  │    │
│  │ ① classify_formats│        │  │ (app_data/)  │  │    │
│  │   (白/黑名单)    │         │  └──────────────┘  │    │
│  │  ② capture_multi │────────▶│                    │    │
│  │   (get 多格式)   │         └────────────────────┘    │
│  │  ③ 写 DB + 图片盘│                ▲                  │
│  │  ④ emit 前端     │                │                  │
│  └──────────────────┘                │                  │
│                                       │                  │
│  ┌──────────────────────────────────┐│                  │
│  │ commands(新增 search/get_thumb) ││                  │
│  │  list_items / copy_back /delete  ││                  │
│  │  search_items(新)               ││                  │
│  │  get_thumbnail(新)              ││                  │
│  └──────────────────────────────────┘│                  │
│                          copy_back: mark_self_write →    │
│                          set(Vec<ClipboardContent>) ─────┼──┐
└──────────────────────────────────────────────────────────┘  │
                                                │              │
                    ┌───────────────────────────▼──────────────▼──┐
                    │  WebView(React)                             │
                    │  SpotlightPanel                              │
                    │  ┌────────────────────────────────────┐    │
                    │  │ 🔍 [搜索框] (新)                    │    │
                    │  │   输入 → debounce → invoke search  │    │
                    │  ├────────────────────────────────────┤    │
                    │  │ ▸ [图] 图片 1920×1080        14:32  │    │
                    │  │   [文] Hello world            14:30 │    │
                    │  │   [富] 项目进度报告(rtf)     14:28  │    │
                    │  │   [文] fn main() {}           14:25  │    │
                    │  ├────────────────────────────────────┤    │
                    │  │ ↑↓ 导航 · ⏎ 回填 · ⌫ 删除          │    │
                    │  └────────────────────────────────────┘    │
                    └─────────────────────────────────────────────┘
```

### 关键架构决策(均来自 m2-research)

1. **FTS 指向 items 表**:给 `clipboard_items` 加冗余 `search_text` 列(存 bigram 预处理后的全文),FTS5 外部内容表指向 items。**不指向 formats**(跨表触发器太脆)。
2. **图片进文件系统**:`<app_data>/images/<sha256前2位>/<sha256>.png` + 缩略图 `images/thumb/<sha256>.png`。DB 存 `file_path`。删历史时引用计数。
3. **多格式原子回填**:`set(Vec<ClipboardContent>)` 一次写全部格式(内部已 clear),不能连续多次单格式 set。
4. **图片统一转 PNG**:`get_image().to_png()` 存盘(即使原是 TIFF)。`ClipboardContent::Image.as_bytes()` 返回空(坑),必须 to_png。
5. **migration 手写版本号**:`PRAGMA user_version` 管理 schema 版本(见阶段 A)。

### 数据流(M2 增量)

**捕获流(多格式)**:
1. 复制 → changeCount+1 → `on_clipboard_change`
2. `available_formats()` → `classify_formats()` 判定(隐私跳过?白名单交集?)
3. `capture_multi()` 读多格式(get)→ 图片转 PNG + sha256 存盘 + 写 formats 多行 + items.content_kind 分类
4. `bigram_cjk(全文)` → 写 `items.search_text` → FTS 触发器自动索引
5. `app.emit("clipboard://added", 新条目)`

**回填流(多格式)**:
1. 选中条目 + 回车 → `invoke('copy_back', {id})`
2. `mark_self_write()`(防反馈循环,M1 机制保留)
3. 读该 item 所有 formats 行 → 组装 `Vec<ClipboardContent>`(图片从盘读)
4. `clipboard.set(contents)` 原子写(内部已 clear)

**搜索流**:
1. 搜索框输入 → debounce 300ms → `invoke('search_items', {query})`
2. 单字 → `LIKE '%字%'` 兜底;多字 → `bigram_cjk(query)` → `to_fts_query` → FTS MATCH + BM25
3. 返回 `ClipboardItem[]`,前端替换列表

---

## 三、数据模型变更

### Schema migration(手写版本号,阶段 A)

```rust
// storage/mod.rs 加版本管理
const CURRENT_SCHEMA_VERSION: i64 = 2;  // M1=1(隐含),M2=2

fn migrate(conn: &Connection) -> Result<(), AppError> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current < 2 {
        // M2: 加 search_text 列(不幂等,版本号守护)
        conn.execute("ALTER TABLE clipboard_items ADD COLUMN search_text TEXT", [])?;
        // FTS5 外部内容表 + 三触发器(IF NOT EXISTS 幂等)
        conn.execute_batch(FTS_SCHEMA_SQL)?;  // 见 m2-research §一
        // 补 idx_formats_file_path(图片引用计数查询用)
        conn.execute("CREATE INDEX IF NOT EXISTS idx_formats_file_path ON clipboard_formats(file_path)", [])?;
        // 回填存量 search_text(从 formats 表读 plain text → bigram)
        backfill_search_text(conn)?;
        conn.execute(&format!("PRAGMA user_version = {CURRENT_SCHEMA_VERSION}"), [])?;
    }
    Ok(())
}
```

> ⚠️ **存量回填**:已有 M1 数据库没 search_text。migration 时从 `clipboard_formats WHERE uti='public.utf8-plain-text'` 读出文本 → `bigram_cjk` → 回填 + FTS rebuild。

### 表结构(M2 最终态)

```sql
-- clipboard_items:加 search_text 列
ALTER TABLE clipboard_items ADD COLUMN search_text TEXT;  -- bigram 预处理后的全文

-- FTS5 外部内容表(指向 items,spike 已验证)
CREATE VIRTUAL TABLE clipboard_fts USING fts5(
    search_text, content='clipboard_items', content_rowid='id', tokenize='unicode61'
);
-- + 三触发器 ai/ad/au(spike 代码 create_fts_schema 直接演化)

-- clipboard_formats:加 file_path 索引(图片引用计数)
CREATE INDEX idx_formats_file_path ON clipboard_formats(file_path);
```

### Rust 类型变更(`models.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ContentKind { Text, Rich, Image, File, Mixed }  // M1 只有 Text,M2 扩展

impl ContentKind {
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "rich" => Self::Rich,
            "image" => Self::Image,
            "file" => Self::File,
            "mixed" => Self::Mixed,
            _ => Self::Text,  // 未知/老数据回退
        }
    }
    pub fn to_db_str(&self) -> &'static str { /* 对应字符串 */ }
}

// ClipboardItem 加 image 相关字段(列表渲染缩略图用)
pub struct ClipboardItem {
    pub id: i64,
    pub first_copied_at: i64,
    pub last_copied_at: i64,
    pub copy_count: i32,
    pub title: String,
    pub content_kind: ContentKind,
    // M2 新增(图片用):
    pub thumb_path: Option<String>,  // 缩略图磁盘路径(None=非图片)
    pub image_size: Option<(u32, u32)>, // 宽高(标题 + 缩略图比例)
}
```

### 前端类型(`models.ts`)

```ts
export type ContentKind = 'text' | 'rich' | 'image' | 'file' | 'mixed'  // M2 扩联合类型
// ClipboardItemSchema: contentKind 改 z.enum([...]),加 thumbPath?: string
```

---

## 四、Command 接口(M2 增量)

| command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `list_items` | `limit?` | `ClipboardItem[]` | M1 既有,扩展返回 thumbPath/imageSize |
| `copy_back` | `id` | `void` | M1 既有,改多格式 `set(Vec<ClipboardContent>)` |
| `delete_item` | `id` | `void` | M1 既有,加图片引用计数删除 |
| `search_items` | `query: string` | `ClipboardItem[]` | **M2 新增**:单字 LIKE / 多字 FTS+bigram+BM25 |
| `get_thumbnail` | `id` | `string`(file url) | **M2 新增**:返回 convertFileSrc 路径(或前端直接用 path) |

**事件**:沿用 `clipboard://added`,payload 的 `ClipboardItem` 扩展字段。

**错误码**(两端镜像,error.rs + invoke.ts):
- `CLIPBOARD_READ_FAILED`(M1 既有)
- `IMAGE_WRITE_FAILED`(M2 新增,图片存盘失败)
- `SEARCH_FAILED`(M2 新增,FTS 查询失败)

---

## 五、UI 交互(SpotlightPanel 改造)

### 布局(M2 增量:搜索框 + 按类型渲染)

```
┌──────────────────────────────────────────┐
│  🔍 [搜索框]                       ● 就绪 │  ← 顶部搜索框(新)
├──────────────────────────────────────────┤
│ ▸ [🖼] 图片 1920×1080              14:32 │  ← 图片:缩略图 + 尺寸标题
│   [📝] Hello world                 14:30 │  ← 文本
│   [✏️] 项目进度报告                 14:28 │  ← 富文本(标 rtf/html)
│   [📄] report.pdf                  14:25 │  ← 文件
├──────────────────────────────────────────┤
│  ↑↓ 导航 · ⏎ 回填 · ⌫ 删除 · / 搜索      │  ← 底栏(加 / 聚焦搜索)
└──────────────────────────────────────────┘
```

### 交互(M2 增量)

| 键/操作 | 行为 |
|---------|------|
| `/` | 聚焦搜索框 |
| 搜索框输入 | debounce 300ms → invoke search_items → 替换列表 |
| `Esc`(搜索激活时) | 清空搜索 → 恢复全列表 |
| `↑`/`↓`/`Enter`/`Delete` | M1 既有(在搜索结果上同样生效) |

### 图片渲染

- `<img src={convertFileSrc(item.thumbPath)}>`(asset protocol,需配 scope)
- 缩略图固定宽高(如 32×32),保持比例

### 搜索高亮(可选,M2 基础版可不实现高亮,先返回结果列表)

---

## 六、资产处置清单

### ✂️ 删除(spike 代码演化)

| 资产 | 处置 |
|------|------|
| `src-tauri/src/storage/fts_spike.rs` | spike 逻辑演化进正式 `storage/fts.rs` + `storage/search.rs`,本文件删 |

### 🔧 改造

| 资产 | 改造为 |
|------|--------|
| `storage/mod.rs` | 加 `migrate()`(手写版本号)+ CURRENT_SCHEMA_VERSION |
| `storage/items.rs` | `insert_or_update` 扩多格式写入;`delete` 加图片引用计数 |
| `clipboard/handler.rs` | `should_capture` → `classify_formats`(白/黑名单);`on_clipboard_change` 改多格式 capture |
| `clipboard/mod.rs` | AppState 加 `images_dir: PathBuf` |
| `commands/clipboard.rs` | `copy_back` 改多格式回填;`delete_item` 改引用计数 |
| `models.rs` | ContentKind 扩展;ClipboardItem 加 thumbPath/imageSize |
| `error.rs` + 前端 `invoke.ts` | 加 IMAGE_WRITE_FAILED / SEARCH_FAILED 错误码镜像 |
| `lib.rs` | setup 时传 images_dir;注册新 commands |
| `tauri.conf.json` | 加 `assetProtocol.scope: ["$APPDATA/images/**"]` |
| `core/types/models.ts` | contentKind 扩联合类型 + Zod schema |
| `hooks/useClipboardItems.ts` | 加 search action(或新建 useClipboardSearch hook) |
| `components/ClipboardList.tsx` | 按 contentKind 分支渲染(文本/图片缩略图) |
| `components/SpotlightPanel.tsx` | 加搜索框 + `/` 聚焦 |

### 📦 新增

| 新增 | 用途 |
|------|------|
| `src-tauri/src/storage/fts.rs` | FTS schema 建表 + 触发器(从 spike 演化) |
| `src-tauri/src/storage/search.rs` | `bigram_cjk` + `to_fts_query` + `search()`(从 spike 演化) |
| `src-tauri/src/clipboard/capture.rs` | 多格式捕获(白/黑名单 + get 多格式 + 图片存盘) |
| `src-tauri/src/clipboard/uti.rs` | UTI 常量(白/黑名单 + ContentFormat 映射) |
| `src-tauri/src/clipboard/title.rs` | 按 contentKind 标题生成 |
| `src-tauri/src/clipboard/image_store.rs` | 图片存盘 + sha256 + 缩略图 + 引用计数 |
| `src-tauri/src/commands/clipboard.rs` 内 | `search_items` / `get_thumbnail` command |
| `src/components/SearchBox.tsx` | 搜索框组件 |
| `src/components/ClipboardListItem.tsx` | 从 ClipboardList 抽出,按 kind 渲染 |

> 🔒 **模块化(核心原则 7)**:capture/uti/title/image_store/fts/search 各自单一职责,单向依赖(handler → capture → image_store/storage;command → storage/search)。

---

## 七、施工阶段(每阶段一个 commit,完成后汇报确认)

> 顺序:多格式捕获打底 → 搜索 → 图片渲染(用户决策)

### 阶段 A:Schema migration + 多格式存储基础(中风险)

**目标**:加 search_text 列 + FTS 表(空壳)+ ContentKind 扩展 + 多格式 items/formats 写入。

1. `storage/mod.rs`:加 `migrate()`(PRAGMA user_version 守护 ALTER TABLE)+ CURRENT_SCHEMA_VERSION=2
2. `storage/fts.rs`:从 spike 演化 FTS schema 建表 + 三触发器(IF NOT EXISTS 幂等)
3. 删 `fts_spike.rs`(逻辑已迁移)
4. `storage/items.rs`:`insert_or_update` 扩为接收多格式 `&[(uti, &[u8])]`,一次写多行 formats + 写 search_text(bigram)
5. `models.rs`:ContentKind 扩 Rich/Image/File/Mixed;ClipboardItem 暂不加图片字段(阶段 C 再加)
6. 存量回填:migration 时从 formats 读 plain text → bigram → search_text + FTS rebuild
7. **验证**:`cargo test`(migration 版本号幂等 + 多格式插入 + search_text 写入 + FTS 索引触发)
8. **commit**:`feat(storage): M2 schema migration + 多格式存储基础`

### 阶段 B:多格式捕获 + 回填(中风险,核心)

**目标**:handler 改多格式捕获,copy_back 改多格式回填。纯文本/富文本跑通(图片下一阶段)。

9. `clipboard/uti.rs`:白/黑名单 UTI 常量 + ContentFormat 映射
10. `clipboard/capture.rs`:`classify_formats`(纯函数,可测)+ `capture_multi`(get 多格式 → 字节)
11. `clipboard/title.rs`:按 contentKind 标题生成(富文本提取纯文本 / 图片尺寸 / 文件名)
12. `clipboard/handler.rs`:`on_clipboard_change` 改用 classify + capture_multi,写多格式 items/formats
13. `commands/clipboard.rs`:`copy_back` 改多格式回填(读 formats 行 → 组装 Vec → set)
14. `error.rs` + 前端 invoke.ts:加 CLIPBOARD 相关错误码镜像
15. **验证**:
    - `cargo test`(classify_formats 纯函数:白名单命中/隐私跳过/噪音过滤 各种场景)
    - GUI 实测:从浏览器复制富文本 → 入库(RTF+HTML+text 三行 formats)→ 回填 → 粘贴到 Word 格式不丢
    - GUI 实测:Finder 复制文件 → 入库(file-url)→ 回填 → 粘贴出文件
16. **commit**:`feat(clipboard): 多格式捕获 + 回填(富文本/文件)`

### 阶段 C:图片存储 + 渲染(中风险)

**目标**:图片捕获存盘 + 缩略图 + 前端渲染 + 引用计数删除。

17. `clipboard/image_store.rs`:图片存盘(sha256 分桶 + to_png)+ 缩略图(thumbnail 200px)+ 引用计数删除
18. `clipboard/capture.rs`:图片格式走 image_store(返回 file_path),文本格式仍走 BLOB
19. `clipboard/handler.rs`:图片 contentKind 分类 + 尺寸记录
20. `storage/items.rs`:`delete` 加图片引用计数(COUNT file_path → 1 才删盘)
21. `models.rs` + 前端 models.ts:ClipboardItem 加 thumbPath/imageSize
22. `commands/clipboard.rs`:`copy_back` 图片格式从盘读 RustImageData
23. `tauri.conf.json`:加 `assetProtocol.scope: ["$APPDATA/images/**"]`
24. 前端 `ClipboardListItem.tsx`:按 contentKind 分支渲染,图片用 `<img src={convertFileSrc}>`
25. **验证**:
    - `cargo test`(image_store:sha256 去重 + 引用计数 + 缩略图生成)
    - GUI 实测:系统截图 → 入库(缩略图 + 「图片 W×H」标题)→ 回填 → 粘贴到聊天工具出图
    - GUI 实测:删图片历史 → 磁盘文件引用计数正确(共享文件不误删)
26. **commit**:`feat(clipboard): 图片存储 + 缩略图渲染 + 引用计数`

### 阶段 D:搜索功能(低风险,spike 已验证架构)

**目标**:搜索框 + FTS5+bigram 搜索 + 单字 LIKE 兜底。

27. `storage/search.rs`:从 spike 演化 `bigram_cjk` + `to_fts_query` + `is_single_cjk_char` + `search()`(多字 FTS+BM25 / 单字 LIKE)
28. `commands/clipboard.rs`:`search_items` command
29. 前端 `SearchBox.tsx`:搜索框 + debounce 300ms + `/` 聚焦 + Esc 清空
30. `hooks/useClipboardItems.ts` 或新 `useClipboardSearch.ts`:search action
31. `SpotlightPanel.tsx`:接搜索框,搜索时用 search 结果替换列表
32. **验证**:
    - `cargo test`(search:中文 bigram 召回 + 英文/代码 + 单字 LIKE + BM25 排序,复用 spike 8 测试)
    - GUI 实测:搜「你好」命中含「你好」历史(含「你好世界」)/ 搜「async」命中代码 / 单字兜底
33. **commit**:`feat(search): FTS5 + bigram 中文搜索`

### 阶段 E:端到端实测 + 收尾(关键,§1.5)

34. GUI 实测全链路(见判据 5 项):富文本回填保格式 / 图片缩略图 + 回填 / 文件 / 搜索 / 密码隐私跳过
35. 密码管理器隐私验证:1Password **桌面 app** 复制 → 确认不入库
36. 验证全绿:`cargo test` + `npm test` + `npm run build` + `npm run test:typecheck`
37. **commit**(如有修正):`fix: M2 端到端实测修正`

---

## 八、必踩坑提醒(M2 执行红线,来自 m2-research)

1. **`ALTER TABLE ADD COLUMN` 不幂等** → 必须用 `PRAGMA user_version` 守护(阶段 A 核心机制)
2. **`ClipboardContent::Image.as_bytes()` 返回空** → 图片转字节必须 `to_png().get_bytes()`(阶段 C 坑)
3. **单格式 set 会清掉其他格式**(`set_text` 内部 clear)→ 多格式回填必须一次 `set(Vec<ClipboardContent>)`(阶段 B 坑)
4. **bigram 必须 ingest + query 双向同函数** → 写入和查询都用 `bigram_cjk`(阶段 A/D,头号坑)
5. **图片引用计数删除** → 删历史前 COUNT file_path,引用数>1 只删 DB 行(阶段 C)
6. **assetProtocol.scope 当前缺失** → 前端显示图片的前置,必须配 `["$APPDATA/images/**"]`(阶段 C)
7. **图片格式统一转 PNG** → 即使原是 TIFF,to_png 转 PNG(阶段 C)
8. **密码管理器浏览器扩展是盲区** → 桌面 app 能防住,浏览器扩展防不住,README 告知(阶段 E 文档)
9. **bigram 验证「旧内容不再被搜到」要用独有 token** → 不能用共用 bigram(如「文本」新旧都含)(spike 踩坑)
10. **不可变性 + 随写随测**(AGENTS §8 + 核心原则 8)→ 每个模块写完立即补单测

---

## 九、红线:什么不做

- ❌ M2 不做全局快捷键 / 托盘 / 自启(M3)
- ❌ M2 不做 Windows 平台(M3)
- ❌ M2 不做容量上限 / 敏感过滤(M4)
- ❌ M2 不做 jieba 分词(P1,FTS 表结构已为升级预留,换 tokenize='jieba' 即可)
- ❌ M2 不做图片 OCR 标题(M3+)
- ❌ M2 不解决密码管理器浏览器扩展盲区(无法解决,README 告知)
- ❌ spike 代码(fts_spike.rs)不留主干(逻辑演化进正式模块后删)

---

## 十、验证流程

```bash
cd /Users/baijunjie/Documents/infra/clipboard-manager

# 每阶段末
cd src-tauri && cargo test && cd ..
npm run build && npm run test:typecheck && npm test

# 阶段 B/C/D 末(GUI 实测)
npm run tauri dev
# 阶段 B:浏览器复制富文本 → 入库 → 回填 → 粘贴到 Word 验证格式
# 阶段 C:截图 → 缩略图 → 回填 → 粘贴验证图片
# 阶段 D:搜索框输入中文/英文/单字 → 验证召回

# 阶段 E(全链路)
# 见 §一 判据 5 项
```

---

## 十一、执行模式

- 每阶段完成后 **commit + 汇报**,等用户确认再进下一阶段
- 阶段内自主执行,不再逐文件确认
- 遇到未预见的决策点停下问用户
- 阶段 B/C 完成后(GUI 能实测时)请用户配合实测(复制富文本/截图等)

---

_本文档 M2 完成后标记 `status: done`,关键决策沉淀到 constraints.md / decision-cases.md。_
