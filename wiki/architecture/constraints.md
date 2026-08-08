---
title: 技术硬约束（红线）
tags: [架构, 约束, 红线]
created: 2026-06-29
updated: 2026-08-07
status: active
---

# 技术硬约束（红线）

> 违反这些约束 = 必然踩坑。开发前必读。

> **M0 架构清理说明**:sidecar 时代的约束(端口三处对齐 / 环境变量契约 /
> PyInstaller 路径漂移 / frozen binary fork / 错误码镜像旧版)已随 sidecar 删除而失效,
> 本文件只保留纯 Rust 单进程架构下仍然适用的约束。sidecar 时代方法论归档见
> `wiki/dev/archive/sidecar-methodology-extract.md`。

## 前后端错误码镜像契约

Rust `src-tauri/src/error.rs` 的 `codes` 模块与前端 `src/core/api/invoke.ts` 的
`InvokeErrorCode` 是同一份契约的两端实现。任一端新增/删除错误码必须同步另一端。

Rust 侧返回 `Err(AppError)` 时,Tauri 自动序列化为 `{code, message}` reject,
前端 `call()` catch 后包成 `InvokeError(code, message)` 异常。

## SQLite 必须开 PRAGMA(M1+ 适用)

> M1 起用 rusqlite 存储剪贴板历史时适用。M0 阶段无 DB。

SQLite 连接必须启用:`PRAGMA foreign_keys=ON` + `PRAGMA journal_mode=WAL` +
`PRAGMA busy_timeout=5000`。否则并发读写时会出现 FK 约束失效 / 写锁冲突。

rusqlite 通过 `Connection::pragma_update` 或 `Connection::execute_batch` 设置。

## SQLite migration:ALTER TABLE 不幂等 + 必须包事务(M2+ 适用)

> M2 起 schema 需结构性变更(加列)时适用。M0/M1 用 `CREATE TABLE IF NOT EXISTS`(幂等)无此问题。

`ALTER TABLE ADD COLUMN` **不幂等**(SQLite 不支持 IF NOT EXISTS),重复执行报
"duplicate column name"。M2 起改用手写版本号 `PRAGMA user_version` 守护:
每次启动查版本号,未达 CURRENT 才跑增量迁移分支。

⚠️ **整段迁移必须包事务**(`unchecked_transaction` + `commit`):若中途某步失败
(如 FTS rebuild 失败),前面 ALTER 已提交但 user_version 没写 → 下次启动重跑
ALTER 报 duplicate column → **app 永久启动失败**(init_db 的 expect panic)。
事务保证任一步失败 ROLLBACK 回旧版本,下次干净重试。

```rust
let tx = conn.unchecked_transaction()?;
let result = migrate_inner(&tx);  // ALTER + 回填 + 建触发器 + rebuild
match result {
    Ok(()) => tx.commit()?,        // 全成功才提交
    Err(e) => { /* tx drop 自动 rollback */ return Err(e); }
}
```

## FTS5 中文搜索:必须 bigram 预处理,不能裸用 unicode61(M2+ 适用)

> M2 全文搜索适用。任何用 SQLite FTS5 搜中文的场景都有此坑。

FTS5 默认 `unicode61` 分词器对 CJK 无效:中文无空格会被当成**单个超长 token**
("你好世界" 整段是 1 个 token),任何子串都搜不到。

- **P0 方案(零依赖)**:应用层 bigram 预处理(CJK 连续段切 2 字重叠滑窗),
  ingest + query 双向用同一个纯函数;单字查询走 `LIKE '%字%'` 兜底。
- `bundled` feature 已含 FTS5(libsqlite3-sys build.rs 无条件编译 `-DSQLITE_ENABLE_FTS5`),
  无需改 Cargo.toml,直接 `CREATE VIRTUAL TABLE USING fts5(...)` 即用。
- ⚠️ FTS5 MATCH 表达式的 token 内双引号需转义为 `""`(短语定界符),否则用户输入
  含 `"` 触发 FTS5 语法错误。
- 外部内容表(content='items')的三触发器(ai/ad/au)同步时,⚠️ 回填 UPDATE 必须在建
  触发器**之前**,否则 au 对尚不存在的 FTS 行发 delete → "database disk image is malformed"。

## asset protocol 显示磁盘文件需配 scope + protocol-asset feature(M2+ 适用)

> M2 图片缩略图显示适用。任何 Tauri 应用用 convertFileSrc 显示磁盘文件的场景都有此要求。

前端 `convertFileSrc(path)` 把磁盘路径转 `asset://localhost/...` URL 显示图片,需要两处配置:

1. `tauri.conf.json` 的 `app.security.assetProtocol`:
```json
"security": {
  "assetProtocol": { "enable": true, "scope": ["$APPDATA/images/**"] }
}
```
scope 只开需要的目录(最小权限,不开 `$APPDATA/**`)。

2. `Cargo.toml` 的 tauri 依赖加 `protocol-asset` feature(否则 build.rs 报 allowlist 不匹配):
```toml
tauri = { version = "2.11.2", features = ["macos-private-api", "protocol-asset"] }
```

## Tauri v2 setup 时序坑（setup 内 emit 事件会被丢弃）

> 通用 Tauri v2 坑,任何 Tauri 2 应用都适用。本工程 M0 用 invoke ping 探活避开此坑。

### 现象

在 `.setup(|app| ...)` 闭包里 `emit("backend://ready", ())`,前端用 `listen('backend://ready')`
监听。结果:**前端永远收不到事件**,启动守卫卡在「后端初始化超时」。

### 根因

Tauri v2 的执行时序:

```
.build() 阶段同步执行 setup 闭包  ← ① emit 在这里发出
                ↓
            创建 WebView
                ↓
            加载页面 + 注入 __TAURI_INTERNALS__  ← ② JS 桥才可用
                ↓
            执行 JS,前端 listen() 注册 listener  ← ③ 但事件在 ① 已经被丢弃
```

`emit` 是**即发即弃、无缓冲**的——发出时若没有 listener,事件直接丢弃,不会重放。
而 setup 闭包在 WebView 创建、JS 执行之前就跑了,所以 setup 里 emit 的任何事件
前端都收不到。`RunEvent::Ready` / `on_page_load` 也救不了——前者同样早于 JS,
后者在 v2 不可靠(是 v1 的 hack,已被弃用)。

官方文档:https://v2.tauri.app/develop/calling-frontend/ 「Timing in setup hooks」

### 修正方案:前端 invoke 探活,不依赖事件

「后端是否就绪」这个问题,**用 invoke 调用本身就是答案**——能调通 command,就证明
`__TAURI_INTERNALS__` 已注入 + Rust AppState 已注册。连 emit 都不需要。

**后端**(删掉 setup 里的 emit,新增无状态 ping command):

```rust
#[tauri::command]
fn ping() -> &'static str { "ok" }

// lib.rs
.setup(|app| {
    app.manage(AppState::new());
    // ⚠️ 不在这里 emit "backend://ready":setup 早于 WebView JS,事件会被丢弃
    Ok(())
})
.invoke_handler(tauri::generate_handler![ping, /* ...其他 command */])
```

**前端**(从 listen 改为 invoke 探活,带重试 + 超时兜底):

```ts
// SpotlightPanel.tsx 内的探活逻辑
const FALLBACK_TIMEOUT_MS = 5000   // 总超时兜底
const PROBE_INTERVAL_MS = 100      // invoke 早期可能偶发失败,重试间隔

useEffect(() => {
  let cancelled = false
  const deadline = setTimeout(() => { if (!cancelled) setStatus('timeout') }, FALLBACK_TIMEOUT_MS)

  const probe = async () => {
    if (cancelled) return
    try {
      await invoke<string>('ping')       // 调通 = 后端就绪
      if (!cancelled) setStatus('ready')
    } catch {
      if (!cancelled) setTimeout(probe, PROBE_INTERVAL_MS)  // 重试
    }
  }
  void probe()

  return () => { cancelled = true; clearTimeout(deadline) }
}, [])
```

### 适用场景

- ✅ **启动时同步状态 / 拿一次性数据**:invoke 调通即 ready(本方案,最简)
- ✅ **后端需持续推事件**:前端先 listen,再 invoke 触发后端 emit(事件从 command 内发出,不走 setup)
- ✅ **流式数据**:用 `Channel<T>`(绑定在 invoke 调用上,天然无时序问题)
- ❌ `RunEvent::Ready` / `on_page_load`:v1 时代的 hack,v2 不可靠

### 反模式

- ❌ setup 内 `emit` 任何前端需要 `listen` 的事件
- ❌ 25ms 轮询某个 HTTP 端点(sidecar 时代的 hack,纯 Rust 用 invoke 探活更简洁)

## tokio features 收紧

> 纯 Rust 单进程架构约束。

tokio 依赖只保留 `["sync", "macros", "rt-multi-thread", "time"]`:

- ❌ **禁止引 `process` feature**:sidecar spawn 专用,纯 Rust 无子进程
- ❌ **禁止引 `fs` feature**:文件 IO 必须走 `spawn_blocking`(见下)
- ❌ **禁止引 `io-util`**:同上,无子进程 stdio

```toml
# ✅ 正确
tokio = { version = "1", features = ["sync", "macros", "rt-multi-thread", "time"] }
```

## 文件 IO 用 spawn_blocking(M1+ 适用)

Tauri command handler 跑在主线程调度上,同步阻塞 IO 会卡 UI。文件写操作必须丢到
blocking pool:

```rust
// ✅ 文件写丢到 blocking pool
tokio::task::spawn_blocking(move || std::fs::write(path, bytes)).await??

// ❌ handler 内直接同步 IO,卡主线程
std::fs::write(path, bytes)
```

## transparent 窗口必须配合 macos-private-api feature(Spotlight 形态)

> 本工程 M0 实测踩坑沉淀。

`tauri.conf.json` 配置 `app.macOSPrivateApi: true` + 窗口 `transparent: true` 时,
**必须**在 `Cargo.toml` 的 tauri 依赖加 `macos-private-api` feature,否则
`tauri-build` 的 build.rs 校验失败:

```toml
# ✅ Cargo.toml — 透明背景对齐 conf.json
tauri = { version = "2.11.2", features = ["macos-private-api"] }
```

```json
// tauri.conf.json
{
  "app": {
    "macOSPrivateApi": true,
    "windows": [{ "transparent": true, "decorations": false }]
  }
}
```

文档未明确此 Cargo feature 要求,是 `cargo check` 报错才发现的。

---

## 已失效约束(sidecar 时代,M0 清理后不再适用)

以下约束随 sidecar 架构删除而失效,仅作历史索引。完整方法论归档见
`wiki/dev/archive/sidecar-methodology-extract.md`(模板母体仍用):

- ~~端口三处对齐(SIDECAR_PORT = 9880)~~ — 无 sidecar,无端口
- ~~环境变量契约对齐(SIDECAR_DATA_DIR / APP_ROOT)~~ — 改用 Tauri `app_data_dir()`
- ~~PyInstaller onefile 路径漂移~~ — 无 PyInstaller
- ~~PyInstaller 延迟 import 必须 hiddenimport~~ — 无 PyInstaller
- ~~frozen binary 下 fork 必须 freeze_support~~ — 无 PyInstaller
- ~~asyncio.to_thread 返回 coroutine 不是 Task~~ — 无 Python

> SQLite PRAGMA 节保留(M1 rusqlite 适用)。Tauri setup 时序坑保留(通用 Tauri 2 坑)。
