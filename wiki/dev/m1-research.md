---
title: M1 技术调研 — macOS 剪贴板监听 / crate 选型 / 多格式存储
tags: [M1, 调研, 剪贴板, NSPasteboard, clipboard-rs, rusqlite]
created: 2026-08-05
updated: 2026-08-05
status: active
---

# M1 技术调研报告

> M1 启动前的技术 spike 结论(§1.16 重大任务先调研)。
> 本文是 [[m1-plan]] 的技术依据。三块调研 + crate 选型决策记录。
>
> **调研方式**:3 个并行 Explore agent + WebFetch 核对 crate API 源码。

---

## 一、监听机制:轮询 changeCount(macOS 无原生通知)

### 结论

macOS `NSPasteboard` **没有任何原生内容变化通知**——没有 KVO、没有 delegate、
没有 NotificationCenter 回调。唯一方案是**轮询 `NSPasteboard.general.changeCount`**
(整数,每次复制操作自增)。

### 轮询间隔(主流剪贴板管理器实测值)

| 应用 | 间隔 | 机制 |
|------|------|------|
| Maccy | **0.5s**(默认,可调) | Timer 主线程 |
| Flycut | 1.0s(硬编码) | NSTimer + NSRunLoopCommonModes |
| clipboard-master(Rust) | **0.5s** | 后台线程 recv_timeout |

**M1 采用 500ms**(Maccy + clipboard-master 共识值,响应/功耗平衡)。
省电进阶(M3+ 再考虑):全局 ⌘C 拦截即时触发 + 轮询降为兜底。

### ⚠️ Tauri 线程安全坑(关键,必须 spike 验证)

`NSPasteboard` 不是线程安全的(Apple 要求只在主线程访问)。
Tauri 的 async command 跑在 tokio worker 线程,从那里读写 NSPasteboard
会与 WebKit 主线程竞态 → `EXC_BAD_ACCESS (SIGSEGV)` 崩溃。

- 已记录在 tauri-apps/plugins-workspace#3205(arboard 写剪贴板时崩溃)
- clipboard-rs 的 watcher 也是后台线程调 NSPasteboard,**理论上同样可能踩坑**
- **缓解**:纯读 changeCount 较轻量,实测稳定;但真正读内容时建议 dispatch 到主线程

> 🔴 **这是 M1 必须 spike 的第一优先项**:clipboard-rs 的 ClipboardWatcherContext
> 在 Tauri 后台线程跑,会不会触发这个崩溃,光看文档无法确认,必须实测。

来源:
- [Apple: NSPasteboard.changeCount](https://developer.apple.com/documentation/appkit/nspasteboard/changecount)
- [SO: 无回调只能 poll](https://stackoverflow.com/questions/5033266/)
- [tauri-apps/plugins-workspace#3205](https://github.com/tauri-apps/plugins-workspace/issues/3205)
- [Maccy Clipboard.swift](https://github.com/p0deje/Maccy/blob/master/Maccy/Clipboard.swift)

---

## 二、Crate 选型:clipboard-rs(已决策)

### 决策

**M1 起统一用 `clipboard-rs` v0.3.5**。后期若作者停维护,fork 自维护。

### 为什么不选 arboard(生态最大但能力受限)

arboard(1Password 维护,3900 万下载)的 API 是「简化版」:只能 `get_text` / `get_image`,
**无 RTF / 无枚举 types / 无监听 / 不能原子写多格式**。对剪贴板**历史管理器**是硬伤:
M2 做富文本/图片时必然要换底层,M1 的代码得推倒重来。

### 为什么不自研 objc2-app-kit(全能但工程量过大)

直接用 `objc2-app-kit` 调 NSPasteboard 最灵活(Maccy 的 Swift 逻辑可一行行翻译),
但要写大量 `unsafe` FFI,且 macOS 专属(Windows 要另写一套)。M1 不该一上来啃 FFI。

### clipboard-rs 选型依据(已核对 API 源码)

**crate 元数据**:v0.3.5 / 2026-06-30 更新 / MIT / 未 yanked /
近 30 天 11 万下载(快速增长,生态小的顾虑在缓解)。

**已确认的核心 API**(docs.rs 源码核对):

```rust
// Clipboard trait — 读写能力齐全
trait Clipboard: Send {
    fn available_formats(&self) -> Result<Vec<String>>;        // 枚举所有 UTI
    fn has(&self, format: ContentFormat) -> bool;
    fn get(&self, formats: &[ContentFormat]) -> Result<Vec<ClipboardContent>>;  // 一次拿多格式
    fn get_text(&self) -> Result<String>;
    fn get_rich_text(&self) -> Result<String>;                  // RTF ✅
    fn get_html(&self) -> Result<String>;                        // HTML ✅
    fn get_image(&self) -> Result<RustImageData>;
    fn get_files(&self) -> Result<Vec<String>>;
    fn set(&self, contents: Vec<ClipboardContent>) -> Result<()>;  // 多格式原子写回填 ✅
    fn set_text / set_rich_text / set_html / set_image / set_files / set_buffer ...
}

// ContentFormat 枚举
enum ContentFormat { Text, Rtf, Html, Image, Files, Other(String) }

// ClipboardContent 枚举(注意:RTF/Html 是 String,Image 是 RustImageData)
enum ClipboardContent {
    Text(String), Rtf(String), Html(String),
    Image(RustImageData), Files(Vec<String>), Other(String, Vec<u8>),
}

// 监听 API
trait ClipboardHandler { fn on_clipboard_change(&mut self); }
trait ClipboardWatcher<T: ClipboardHandler>: Send {
    fn add_handler(&mut self, handler: T) -> &mut Self;
    fn start_watch(&mut self);                          // 阻塞,放独立线程
    fn get_shutdown_channel(&self) -> WatcherShutdown;  // 优雅停止
}
// ClipboardWatcherContext::new_with_interval(Duration) 构造
```

**关键发现**:`set(Vec<ClipboardContent>)` 存在 —— 多格式**原子写回填能力是有的**,
调研 C 说「clipboard-rs 不能原子写」的结论不准确,实际比 arboard 强。

来源:
- [clipboard-rs crates.io](https://crates.io/crates/clipboard-rs) / [GitHub](https://github.com/ChurchTao/clipboard-rs)
- [Clipboard trait 源码](https://docs.rs/clipboard-rs/latest/clipboard_rs/trait.Clipboard.html)
- [common.rs 源码(ContentFormat/ClipboardContent)](https://docs.rs/clipboard-rs/latest/src/clipboard_rs/common.rs.html)
- [tauri-plugin-clipboard-x(基于 clipboard-rs 的 Tauri 插件,证明 Tauri 可用)](https://docs.rs/tauri-plugin-clipboard-x/)

---

## 三、多格式存储:items + formats 一对多两张表

### 结论(照搬 Maccy + Ditto 的生产验证设计)

**一次复制会产生多种格式**(从 Word 复制 → RTF + 纯文本 + HTML 同时存在)。
主流工具都**全格式存储**,回填时按 UTI 全部写回 NSPasteboard。

表结构:**items 表(元数据)+ formats 表(type+value,一对多)**。
两个独立的开源实现(Maccy Swift + Ditto C++/SQLite)走到了同一个设计 —— 这就是答案。

```sql
-- 主表:一条历史记录的元数据
CREATE TABLE clipboard_items (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  first_copied_at INTEGER NOT NULL,   -- unix epoch ms
  last_copied_at  INTEGER NOT NULL,
  copy_count      INTEGER NOT NULL DEFAULT 1,
  pin             TEXT,               -- 置位标记(M1 可不用)
  title           TEXT NOT NULL,      -- 列表预览文本(冗余,方便查询)
  source_app      TEXT,               -- 来源 bundle id
  content_kind    TEXT NOT NULL       -- 'text'|'image'|'file'|'mixed'(M1 只有 text)
);
CREATE INDEX idx_items_last_copied ON clipboard_items(last_copied_at DESC);

-- 格式表:一条记录的每个 UTI 格式(M1 只写纯文本一行,M2 多行)
CREATE TABLE clipboard_formats (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id   INTEGER NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
  uti       TEXT NOT NULL,            -- 'public.utf8-plain-text' 等
  data_blob BLOB,                     -- 文本类格式:直接存字节
  file_path TEXT,                     -- 图片类格式:磁盘相对路径(M1 不用)
  size      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_formats_item ON clipboard_formats(item_id);
```

### 图片存储:混合方案(Raycast 生产验证)

- **文本格式(RTF/HTML/text)**:几十 KB,直接 BLOB 进 formats 表
- **图片**:`sha256(content)` 做文件名(天然去重),存磁盘
  `~/Library/Caches/<bundle>/images/`,DB 存路径
- 理由:SQLite 官方测试小 BLOB(<100KB)比 fs 快 35%,但 1-2MB TIFF 会撑大 DB、拖慢备份

### M1 取舍:现在就建 formats 表,只写纯文本一行

**强烈建议 M1 就建两张表**,即使只写纯文本。理由:迁移成本不对称——M1 多建一张表
几乎零成本,但 M2 再加图片要 ALTER TABLE + 回填 + 改所有查询。Maccy/Ditto 模型照搬,
连 M2 数据结构都不用再设计。

### 捕获/回填策略(参考 Maccy)

- **捕获**:`available_formats()` 列出 UTI → 白名单
  (`public.utf8-plain-text`/`public.rtf`/`public.html`/`public.png`/`public.tiff`/`public.file-url`)
  + 黑名单(过滤 `org.nspasteboard.Transient/Concealed`、`dyn.*`、
  `com.apple.linkpresentation.metadata` 等隐私/噪音)
- **回填**:`clearContents()` → 对每个 UTI 写回 → `set(Vec<ClipboardContent>)`
- **标题**:优先级 file-url → text → rtf → html,取前 1000 字符

来源:
- [Maccy HistoryItem.swift / HistoryItemContent.swift](https://github.com/p0deje/Maccy) — items+formats 一对多模型
- [Ditto DatabaseUtilities.cpp](https://github.com/sabrogden/Ditto) — Main/Data 两张表 CREATE TABLE
- [Migrating Raycast Clipboard History](https://simo.sh/notes/migrating-raycast-clipboard-history) — 文本 SQLite + 图片 Caches
- [SQLite 官方:BLOB 比 fs 快 35%(小文件)](https://www.sqlite.org/fasterthanfs.html)

---

## 四、待 spike 验证的未知项

| 未知项 | 验证方式 | 影响 |
|--------|----------|------|
| 🔴 clipboard-rs watcher 在 Tauri 后台线程会不会触发 NSPasteboard 线程崩溃 | 跑最小 spike:start_watch + 读文本 + emit 前端 | 决定监听架构(后台线程 vs dispatch 主线程) |
| 🟡 clipboard-rs 的 ClipboardContent 枚举变体在真实复制场景下的行为 | spike 时从 Word/浏览器复制,打印 available_formats + get 结果 | 确认 M2 多格式捕获的数据形态 |

### ✅ Spike 验证结果(2026-08-05,实测通过)

跑了最小 spike(`src-tauri/src/spike_clipboard.rs` + 前端 `SpikeClipboardProbe.tsx`),
在 Tauri 后台线程起 `ClipboardWatcherContext`(500ms 轮询),连续触发 5 次剪贴板变化:

| 测试场景 | 结果 |
|----------|------|
| 纯文本(pbcopy)×3 | ✅ 全部捕获,`get_text()` 正确(含中文) |
| 剪贴板清空 | ✅ 捕获(text 长度 0) |
| 文件 URL(Finder /tmp) | ✅ 捕获,formats=`[com.apple.alias-record, CorePasteboardFlavorType...]` |

**两个未知项的结论:**

1. ✅ **NSPasteboard 线程崩溃坑不触发** —— clipboard-rs watcher 在 Tauri 后台线程
   跑,连续 5 次变化(含读内容)零崩溃。
   **→ M1 监听架构定:后台线程 + clipboard-rs watcher,无需 dispatch 主线程。**
   *(注:spike 只测了短时间。长期/高频读取若出问题,备选方案是 dispatch 到主线程读内容。)*

2. ✅ **多格式捕获正常** —— `available_formats()` 正确返回 UTI 列表。
   实测格式:纯文本 = `[public.utf8-plain-text, NSStringPboardType]`(现代+老式 UTI 共存)。

**⚠️ M1 实现注意(实测发现):**
- `get_text()` 在无文本格式时会报错(文件 URL 场景:`No string found`)。
  正式实现必须先 `available_formats()` 判断含文本格式再读,或 try/catch 容错。
- 纯文本复制同时带 `public.utf8-plain-text` + `NSStringPboardType` 两个 UTI
  (现代 + 老式),存库时按白名单只存 `public.utf8-plain-text`,避免重复。

**spike 代码处置**:`spike_clipboard.rs` + `SpikeClipboardProbe.tsx` 是临时验证代码,
spike 通过后 M1 正式实现会演化/重写它们,不留临时代码进主干。

---

_本文档是 M1 的技术地基。spike 验证已完成,结论已回填本文第四节。_
