---
title: M2 技术调研 — FTS5 中文搜索 / 图片存储 / 多格式捕获回填
tags: [M2, 调研, FTS5, 中文分词, 图片存储, clipboard-rs, 多格式]
created: 2026-08-06
updated: 2026-08-07
status: done
---

# M2 技术调研报告

> M2 启动前的技术调研结论(§1.16 重大任务先调研)。本文是 [[m2-plan]] 的技术依据。
>
> **调研方式**:2 个并行 Explore agent(联网查 docs.rs 源码 / SQLite 官方文档 / Maccy 源码 / Tauri 文档)+ 现有代码库核实。
>
> **M2 三块**:① 全文搜索(SQLite FTS5 + 中文分词)② 图片存储 ③ 多格式捕获/回填(富文本+图片+文件)。
> 三块一起做,但施工分阶段(先多格式捕获打底,再搜索,最后图片渲染)。

---

## 〇、M1 现状(M2 在此基础上扩展)

> 已核实现有代码,关键现状决定 M2 设计:

| 维度 | M1 现状 | M2 需改 |
|------|---------|---------|
| schema | `clipboard_items`(主表)+ `clipboard_formats`(一对多,含 `uti`/`data_blob`/`file_path`/`size`)**已建好** | 加 `search_text` 冗余列 + FTS5 虚拟表 + `idx_formats_file_path` 索引 |
| formats 写入 | **只写纯文本一行**(`insert_or_update`,`items.rs:66`) | 扩为多格式写入(一次 INSERT 多行 formats) |
| 捕获 | `should_capture` 白名单只认 `public.utf8-plain-text`,图片/文件直接跳过 | 扩为白名单交集 + 隐私黑名单整条跳过 |
| 回填 | `copy_back` 只 `set_text` | 改 `set(Vec<ClipboardContent>)` 原子写多格式 |
| 标题 | 文本前 1000 字符 | 按 contentKind 分支(图片取尺寸 / 文件取文件名) |
| ContentKind | 只有 `Text`(`models.rs:20`) | 加 `Image`/`File`/`Mixed` |
| 依赖 | `clipboard-rs 0.3` + `image` feature 已开 / `rusqlite 0.31` bundled | **FTS5 已含在 bundled 里,无需改 Cargo.toml**(见 §一) |
| 前端 | 纯文本渲染 / `contentKind: z.literal('text')` / 无 search | 按 kind 分支渲染 + contentKind 扩联合类型 + 加 search |

---

## 一、全文搜索:FTS5 + 应用层 bigram(零新依赖)

### ✅ 决定性发现:bundled 已含 FTS5

**`libsqlite3-sys` 的 `build.rs` 在 `build_bundled::main` 里无条件传 `-DSQLITE_ENABLE_FTS5`(没有 cfg 包裹)**。
→ 当前 `rusqlite = { features = ["bundled"] }` **已经把 FTS5 编译进去了**,直接 `CREATE VIRTUAL TABLE USING fts5(...)` 即用。
→ **Cargo.toml 一行都不用改**。rusqlite 根本没有 `fts5` feature(bundled-full 也不是为 FTS5 准备的)。

### 结论:P0 = unicode61 + 应用层 bigram + LIKE 单字兜底

**不引任何分词库**。CJK 连续段切成 bigram(2 字重叠滑窗),ingest/query 双向用同一个纯函数;单字查询走 `LIKE '%x%'`(万条内无感)。

```
"你好世界" → 索引成 "你好 好世 世界"
搜 "你好"   → MATCH '"你好"'            ✅ 精确命中
搜 "你好世" → MATCH '"你好" OR "好世"'  ✅ 召回
搜 "你"     → LIKE '%你%'              ✅ 兜底
搜 "rust async" → MATCH '"rust" OR "async"'(英文走 unicode61 默认空格切,天然适合代码/URL)
```

| 方案 | 中文召回 | 英文/代码 | 工程量 | 评估 |
|------|---------|----------|--------|------|
| **unicode61 + bigram(P0 推荐)** | 好(2 字精确,3 字+ 命中) | 好(默认按空格切) | **极小**(30 行纯函数 + FTS 表) | ✅ 首选 |
| jieba-rs 自定义 tokenizer(P1) | 优(语义分词) | 好 | 大(250 行 unsafe FFI + 升 rusqlite) | ⏸ 留升级 |
| trigram | **差**(<3 字查询静默返回空,2 字查询全军覆没) | 中 | 小 | ❌ 短查询硬伤 |
| 默认 unicode61 不动 | **废**(CJK 黏成一坨,整段 1 个 token) | 好 | 0 | ❌ 中文完全不可用 |

**为什么不裸用 unicode61**:CJK 无空格会被当成**单个超长 token**,「你好世界」整段是 1 个 token,任何子串都搜不到。这是 FTS5 中文头号坑,必须 bigram/jieba 其一。

**P1 升级路径**:用户反馈「搜整词搜不到片段」或噪音大时再上 jieba。届时引 `rusqlite-ext`(`xCreateTokenizer_v2` 封装)+ `sqlite-jieba-tokenizer`,换 `tokenize='jieba'`,**FTS 表结构/触发器完全不用动**。注意 rusqlite 0.31 用不了现成封装 crate(跟随 0.39),要么升 rusqlite 要么抄 gist,M2 不为此升级。

### bigram 预处理(纯函数,ingest + query 双向同函数)

```rust
/// CJK 连续段切 bigram(2 字重叠滑窗),英文/数字/ASCII 不动。
/// 写入 FTS 前对正文调一次;查询前对 query 调一次。两边必须同函数。
pub fn bigram_cjk(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut cjk_run: Vec<char> = Vec::new();
    for ch in text.chars() {
        let is_cjk = ('\u{4E00}'..='\u{9FFF}').contains(&ch); // CJK 统一汉字基本区
        if is_cjk { cjk_run.push(ch); }
        else { flush_cjk_run(&cjk_run, &mut out); cjk_run.clear(); out.push(ch); }
    }
    flush_cjk_run(&cjk_run, &mut out);
    out
}
fn flush_cjk_run(run: &[char], out: &mut String) {
    if run.is_empty() { return; }
    if run.len() < 2 { out.extend(run); return; }   // 单字 CJK 原样保留(走 LIKE 兜底)
    out.push(' ');
    for w in run.windows(2) { out.extend(w); out.push(' '); } // "你好世界"→"你好 好世 世界"
}
```

> ⚠️ bigram 方案头号坑:**ingest 和 query 必须用同一个 `bigram_cjk`**。只切一边 → 全部查不到。

### 外部内容表 + 触发器(指向 items 表,不指向 formats)

**关键决策:给 `clipboard_items` 加冗余 `search_text` 列,FTS 指向 items 表。**

理由:FTS5 外部内容表 `content=` 只能指向**一张表**。文本实际在 formats 表(BLOB),让 FTS 指向 formats 要跨表触发器(items 变了连带 formats / formats 变了回查 items),M2 加富文本/图片后触发器逻辑会越来越脆。而 items 表 `insert_or_update` 是**单点写入**,改一处即可同时写 `search_text` + 触发 FTS,数据一致性最简单。

```sql
-- 1) items 表加冗余搜索列(存 bigram 预处理后的全文)
ALTER TABLE clipboard_items ADD COLUMN search_text TEXT;

-- 2) FTS5 外部内容表(tokenize=unicode61,bigram 已在应用层切好,这里按空白分词)
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
    search_text,
    content='clipboard_items',
    content_rowid='id',
    tokenize='unicode61'
);

-- 3) 同步触发器(FTS5 外部内容表标准三件套)
CREATE TRIGGER IF NOT EXISTS clipboard_fts_ai AFTER INSERT ON clipboard_items BEGIN
    INSERT INTO clipboard_fts(rowid, search_text) VALUES (new.id, new.search_text);
END;
CREATE TRIGGER IF NOT EXISTS clipboard_fts_ad AFTER DELETE ON clipboard_items BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text);
END;
CREATE TRIGGER IF NOT EXISTS clipboard_fts_au AFTER UPDATE ON clipboard_items BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text);
    INSERT INTO clipboard_fts(rowid, search_text) VALUES (new.id, new.search_text);
END;

-- 4) 回填存量(先把 search_text 用 bigram 填好,再 rebuild)
INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');
```

### 搜索查询(BM25 排序 + 单字 LIKE 兜底)

```sql
-- 多字查询走 FTS + BM25(分数越低越相关)
SELECT i.* FROM clipboard_fts
JOIN clipboard_items i ON i.id = clipboard_fts.rowid   -- FTS rowid 映射回 items.id
WHERE clipboard_fts MATCH ?        -- ? = to_fts_query(user_input):bigram 后短语 OR
ORDER BY bm25(clipboard_fts), i.last_copied_at DESC LIMIT 50;

-- 单字查询(Rust 判断 char 长度==1)走 LIKE 兜底
SELECT * FROM clipboard_items WHERE search_text LIKE ? ORDER BY last_copied_at DESC LIMIT 50;
```

### ⚠️ migration 现状提醒

现有 migration 是手写 `CREATE TABLE IF NOT EXISTS`(幂等,无版本管理)。`CREATE VIRTUAL TABLE IF NOT EXISTS` 同样幂等可纳入。但 **`ALTER TABLE ADD COLUMN` 不幂等**——M2 加 `search_text` 列要么加版本号判断,要么引 `rusqlite_migration`(m1-research 早就预留这个升级点)。**这是 M2 规划时必须定的事**(见第四节 spike 项)。

来源:[SQLite FTS5 官方文档](https://www.sqlite.org/fts5.html) / [rusqlite Cargo.toml feature 全表](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml) / [libsqlite3-sys build.rs](https://github.com/rusqlite/rusqlite/blob/master/libsqlite3-sys/build.rs) / [bigram fix 原型](https://dev.to/foxck016077/sqlite-fts5-wont-tokenize-chinese-heres-the-7-line-bigram-fix-that-did-4fcc) / [unicode61 不支持 CJK(SO)](https://stackoverflow.com/questions/52422437/) / [rusqlite-ext(jieba P1 参考)](https://github.com/miyayaLora/sqlite-simple-tokenizer)

---

## 二、图片存储:混合方案(文本进 BLOB,图片进文件系统)

### 结论:按内容类型分流,不用大小阈值

| 内容类型 | 存储位置 | 理由 |
|---------|---------|------|
| 纯文本 / RTF / HTML | `data_blob`(BLOB) | 几十字节~几十 KB,SQLite 小 BLOB 甜区(比 fs 快 35%),去重要全表扫 blob |
| **图片(PNG/TIFF)** | **文件系统** + `file_path` | 几百 KB~20MB(全屏 TIFF),撑大 DB、拖慢 VACUUM/备份 |
| 文件(`public.file-url`) | `data_blob` 存 URL 字符串 | URL 本身很小 |

不用大小阈值判断:类型分流比大小判断更可预测、更易测。

**图片格式真相**:`get_image()` 返回解码后的 `RustImageData`(内部 `DynamicImage`),与原格式无关。存盘用 `to_png()` 统一转 PNG(更小更通用,即使原是 TIFF)。⚠️ **`ClipboardContent::Image.as_bytes()` 返回空**(源码注释明确不支持),图片转字节**必须 `to_png().get_bytes()`**。

### 文件系统组织(sha256 分桶 + 引用计数)

```
~/Library/Application Support/com.example.clipboard-manager/
├── clipboard.db
└── images/
    ├── <sha256前2位>/<sha256>.png     ← 原图,sha256 文件名天然去重
    └── thumb/<sha256>.png             ← 缩略图(200px,5-20KB)
```

分桶(`ab/<sha256>.png`)防单目录文件数膨胀(>1 万文件后遍历变慢),Maccy/Raycast 都这么做。

### 图片去重 + 引用计数删除

sha256 文件名 → 同一张图复制多次磁盘只存一份 → 多个 formats 行指向同一文件。
**删除某 item 时必须引用计数**:

```rust
// 删除含图片的 item:
// 1. 查出该 file_path
// 2. COUNT clipboard_formats WHERE file_path = ? → 引用数
// 3. 引用数 == 1(只当前 item 用)→ 删 DB 行 + 删磁盘文件 + 删缩略图
// 4. 引用数 > 1                    → 只删 DB 行,保留磁盘文件
```

→ **M2 需补 `idx_formats_file_path` 索引**(当前只有 `idx_formats_item`)。

来源:[SQLite: 35% Faster Than Filesystem](https://sqlite.org/fasterthanfs.html) / [clipboard-rs 源码 macos.rs get_image](https://github.com/ChurchTao/clipboard-rs)

---

## 三、多格式捕获/回填:白名单 + 隐私黑名单

### 真实复制场景的 UTI 组合

| 来源 | 剪贴板 UTI 组合 |
|------|----------------|
| 浏览器复制选中文本 | `public.utf8-plain-text` + `public.html` + `com.apple.WebKit.custom-pasteboard-data`(噪音) |
| Word/Pages 复制 | `public.utf8-plain-text` + `public.rtf` + `public.html` + `com.apple.linkpresentation.metadata`(噪音) |
| 代码编辑器 | `public.utf8-plain-text` + `public.html` + `org.chromium.*`(噪音) |
| 系统截图 | `public.png` + `public.tiff`(两个图像 UTI 同时存在) |
| Finder 复制文件 | `public.file-url` + `com.apple.alias-record` |
| 预览.app 复制 | `public.tiff`(或 png) |

### 白名单(照搬 Maccy supportedTypes,千万用户验证)

```rust
pub const WHITELIST_UTIS: &[&str] = &[
    "public.utf8-plain-text",
    "public.rtf",
    "public.html",
    "public.png",
    "public.tiff",
    "public.file-url",
];
```

### 黑名单两层(隐私跳过 + 噪音过滤)

**A. 隐私跳过标记(检测到 → 整条 item 不入库):**
```rust
pub const PRIVACY_SKIP_UTIS: &[&str] = &[
    "org.nspasteboard.ConcealedType",      // 密码(1Password/Bitwarden 桌面 app)
    "org.nspasteboard.TransientType",      // 临时内容(密码管理器)
    "org.nspasteboard.AutoGeneratedType",  // 程序生成
];
```
这是 [nspasteboard.org](http://nspasteboard.org/) 社区标准,Maccy/Alfred/Raycast 都遵守。

**B. 噪音格式过滤(纯忽略,不存不报错):**
```rust
pub const NOISE_UTIS: &[&str] = &[
    "com.apple.linkpresentation.metadata",      // 链接预览元数据(每次复制都变,破坏去重)
    "com.apple.WebKit.custom-pasteboard-data",
    "org.chromium.web-custom-data",
    "org.chromium.source-url",
    "com.apple.notes.richtext",
    // ... + 所有 dyn.* 前缀(动态 UTI 全过滤)
];
```

### ⚠️ 密码管理器浏览器扩展是盲区(必须告知用户)

**桌面 app 能防住**:1Password/Bitwarden **桌面 macOS app** 复制密码正确标记 `ConcealedType`,检测即跳过。
**浏览器扩展防不住**:1Password/Bitwarden **浏览器扩展**复制密码只写 `public.utf8-plain-text`,不标记任何 `org.nspasteboard.*`(已确认 bug:[1Password community](https://www.1password.community/1password-at-home-31/passwords-copied-to-the-clipboard-from-the-browser-extensions-are-not-marked-as-concealed-19551)、[Bitwarden #326](https://github.com/bitwarden/browser/issues/326))。这种情况下**任何剪贴板管理器都无法区分密码和普通文本**。
→ M2 实现 A 层(防桌面 app)+ README 告知浏览器扩展盲区。

### 捕获决策(纯函数,可测)

```rust
pub fn classify_formats(formats: &[String]) -> CaptureDecision {
    // 1. 隐私跳过:含任一 PRIVACY_SKIP_UTI → 整条拒绝
    if formats.iter().any(|f| PRIVACY_SKIP_UTIS.contains(&f.as_str())) { return Skip; }
    // 2. 白名单交集(噪音/dyn.* 自然被排除)
    let capture: Vec<&str> = formats.iter().map(|s| s.as_str())
        .filter(|f| WHITELIST_UTIS.contains(f)).collect();
    if capture.is_empty() { Skip } else { Capture(capture) }
}
```

### 多格式读取(clipboard-rs 源码确认)

`get(&[ContentFormat])`:格式不存在时**跳过不报错**,只把存在的推入结果 Vec。
`ContentFormat::Other(String)` 能读任意 UTI(用于读不在枚举里的)。

```rust
fn capture(ctx: &ClipboardContext, keep_utis: &[&str]) -> Vec<(String, Vec<u8>)> {
    let req: Vec<ContentFormat> = keep_utis.iter().map(|uti| match *uti {
        "public.utf8-plain-text" => ContentFormat::Text,
        "public.rtf"             => ContentFormat::Rtf,
        "public.html"            => ContentFormat::Html,
        "public.png"|"public.tiff"=> ContentFormat::Image,  // get_image PNG 优先自动处理
        "public.file-url"        => ContentFormat::Files,
        other                    => ContentFormat::Other(other.into()),
    }).collect();
    ctx.get(&req).unwrap_or_default().into_iter().filter_map(|c| match c {
        Text(s)    => Some(("public.utf8-plain-text".into(), s.into_bytes())),
        Rtf(s)     => Some(("public.rtf".into(), s.into_bytes())),
        Html(s)    => Some(("public.html".into(), s.into_bytes())),
        Image(img) => img.to_png().ok()                         // ⚠️ 必须 to_png,as_bytes 返回空
                         .map(|b| ("public.png".into(), b.get_bytes().to_vec())),
        Files(p)   => Some(("public.file-url".into(), p.join("\n").into_bytes())),
        Other(u,d) => Some((u, d)),
    }).collect()
}
```

> ⚠️ 不要同时传 `Text` 和 `Other("public.utf8-plain-text")`,会重复读(Text 用 NSPasteboardTypeString 已覆盖)。
> ⚠️ PNG+TIFF 同时存在时 `get(Image)` 只读一次 PNG,只存 PNG 一份(不存冗余 TIFF)。

### 多格式回填(DB → 组装 → set 原子写)

**核心:回填要把该 item 所有存下来的格式都写回,不能只写 text**(否则 Word 富文本粘贴丢格式)。
`clipboard.set(Vec<ClipboardContent>)` **内部已 `clearContents()` + `writeObjects`**(源码 `write_to_clipboard(.., true)`),所以**一次 set 就是原子多格式写,不需要手动 clear**。
⚠️ **不能连续多次 `set_text` + `set_image`**——单格式 set 内部会 clear,互相清掉。

```rust
pub fn copy_back_multi(conn, ctx, id) -> Result<()> {
    let rows: Vec<(uti, blob, file_path)> = query_formats(conn, id);  // 读所有格式行
    let contents: Vec<ClipboardContent> = rows.into_iter().map(|row| match row {
        ("public.utf8-plain-text", blob, _) => Text(String::from_utf8(blob)?),
        ("public.rtf", blob, _)             => Rtf(String::from_utf8_lossy(&blob).into_owned()),
        ("public.html", blob, _)            => Html(String::from_utf8_lossy(&blob).into_owned()),
        ("public.png"|"public.tiff", _, path) => Image(RustImageData::from_path(&path)?), // 图片从文件读
        ("public.file-url", blob, _)        => Files(String::from_utf8(blob)?.lines().map(String::from).collect()),
        (other, blob, _)                    => Other(other, blob),
    }).collect();
    state.mark_self_write();        // ⚠️ 保留 M1 防反馈循环机制
    ctx.set(contents)?;             // 原子写(内部已 clear)
    Ok(())
}
```

### 标题生成策略(按 contentKind 分支)

参考 Maccy `previewableText`(优先级 fileURL → text → rtf.string → html.string):

| contentKind | 标题规则 | 示例 |
|-------------|---------|------|
| `text` | 现有 `truncate_title`(前 1000 char) | `Hello world` |
| `rtf`/`html`(富文本) | 从 RTF/HTML 提取纯文本部分(不存源码当标题) | Word 复制 → `项目进度报告` |
| `image` | `图片 {w}×{h}`(尺寸在捕获时记录,M2 暂不 OCR) | `图片 1920×1080` |
| `file` | 从 file-url 取文件名(多文件:首个 + ` 等 N 项`) | Finder 复制 → `report.pdf` |
| `mixed` | 按优先级 file > image > text | — |

> ⚠️ 图片 width/height 在 `ClipboardContent::Image` 里有(`get_size()`),转字节存进 formats 后就丢了。捕获时要在 items 表记下尺寸(复用 `title` 直接写 `图片 1920×1080`,或加 `meta TEXT` 存 JSON)。

### 缩略图 + 前端显示

- **生成**:复用 clipboard-rs `RustImageData::thumbnail(200,200)`(保比例,clipboard-rs image feature 已开,**不单独引 image crate 避免重复依赖**)。
- **存储**:磁盘 `images/thumb/<sha256>.png`(200px PNG ~5-20KB),列表只传路径。
- **前端显示**:`convertFileSrc(path)` 转 `asset://localhost/...` URL,`<img src>` 走 WebView 原生加载(不经 invoke 序列化,性能最好)。
- ⚠️ **前置阻塞项**:`tauri.conf.json` 当前 `app.security` 无 `assetProtocol.scope`,M2 必须补:
```json
"security": {
  "csp": null,
  "assetProtocol": { "enable": true, "scope": ["$APPDATA/images/**"] }
}
```
`scope` 只开 `images/`,最小权限(不开 `$APPDATA/**`)。

来源:[Maccy Clipboard.swift supportedTypes/ignoredTypes](https://github.com/p0deje/Maccy) / [nspasteboard.org](http://nspasteboard.org/) / [Tauri v2 Asset Protocol](https://v2.tauri.app/security/asset-protocol/) / [clipboard-rs 源码](https://github.com/ChurchTao/clipboard-rs)

---

## 四、待 spike 验证的未知项

| 未知项 | 验证方式 | 影响 |
|--------|----------|------|
| 🔴 `bundled` 的 FTS5 + bigram 方案在 rusqlite 0.31 实际可用性(建表/触发器/MATCH/BM25) | ✅ 已 spike 验证(见下) | 决定搜索架构 |
| 🟡 `ALTER TABLE ADD COLUMN` 幂等问题 + migration 版本管理 | M2 施工时验证(重复执行 `ALTER TABLE` 报错) | 决定 M2 schema 迁移机制(M0/M1 都是 IF NOT EXISTS 幂等,M2 加列打破这个模式) |
| 🟡 clipboard-rs `get_image().to_png()` 在真实截图场景的字节/尺寸 | GUI 实测:截图到剪贴板 → get_image → to_png | 确认图片存储的数据形态 + 标题生成 |
| 🟡 `convertFileSrc` + asset protocol 显示磁盘图片 | GUI 实测:写图到 app_data_dir/images → 配 scope → 前端 img 显示 | 确认前端图片显示链路(当前 scope 缺失,是前置阻塞) |
| 🟢 多格式回填是否保持格式(Word 富文本复制→回填→粘贴到 Word 格式不丢) | GUI 实测:Word 复制 → 捕获存库 → 回填 → 粘贴到 Word 对比 | 确认多格式回填质量 |

> 标 🟡🟢 的项依赖 GUI 实测或属于施工阶段验证,不单独 spike;放进 m2-plan 的阶段验收点。

### ✅ Spike 验证结果(2026-08-06,FTS5 + bigram 全部实测通过)

写了 8 个 spike 测试(`src-tauri/src/storage/fts_spike.rs`),全部通过:

| 测试 | 验证内容 | 结果 |
|------|---------|------|
| spike01 | 🔴 **bundled feature 确实已含 FTS5**(CREATE VIRTUAL TABLE USING fts5 零报错,无需改 Cargo.toml) | ✅ 通过 |
| spike02 | bigram 纯函数行为(`你好世界`→`你好 好世 世界` / 单字保留 / 中英混合 / 代码不变) | ✅ 通过 |
| spike03 | 🟡 **bigram 中文召回**(搜「你好」命中「你好世界」;搜「你好世」MATCH `"你好" OR "好世"` 召回) | ✅ 通过 |
| spike04 | 英文/代码/URL 在 unicode61 默认分词下正常(按空格切,搜 `async`/`github` 命中) | ✅ 通过 |
| spike05 | BM25 排序(词频高的排前面) | ✅ 通过 |
| spike06 | 单字 LIKE 兜底(搜「你」走 `LIKE '%你%'` 命中含「你」的所有条目) | ✅ 通过 |
| spike07 | rebuild 回填存量(FTS5 外部内容表标准 `VALUES('rebuild')` 幂等) | ✅ 通过 |
| spike08 | UPDATE/DELETE 触发器同步(改/删 items → FTS 正确同步) | ✅ 通过 |

**结论:**
1. ✅ **P0 方案(unicode61 + 应用层 bigram + LIKE 单字兜底)实测可行,整个搜索架构定案**。
2. ✅ **Cargo.toml 无需改动**(bundled 含 FTS5,源码 + 实测双重确认)。
3. ✅ 外部内容表 + 三触发器 + rebuild 全部正常工作。

**⚠️ 实现注意(spike 踩坑):**
- FTS5 外部内容表 UPDATE 触发器工作正常,但测试「旧内容不再被搜到」时要用**独有的 bigram token**(如「旧文」),不能用共用 token(如「文本」——新旧文本都含)。这是 bigram 方案特性:相邻文本内容会有重叠的 bigram token,验证时要注意区分。

**spike 代码处置**:`fts_spike.rs` 是临时验证模块,标注「M2 正式实现后删除」。其 `bigram_cjk`/`to_fts_query`/`is_single_cjk_char`/`create_fts_schema` 逻辑会演化进正式 FTS 模块,M2 施工时迁移后删除本文件。

---

_本文档是 M2 的技术地基。spike 验证完成后,结论回填本文第四节,再据此写 [[m2-plan]]。_
