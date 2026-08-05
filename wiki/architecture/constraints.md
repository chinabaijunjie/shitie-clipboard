---
title: 技术硬约束（红线）
tags: [架构, 约束, 红线]
created: 2026-06-29
updated: 2026-06-29
status: active
---

# 技术硬约束（红线）

> 违反这些约束 = 必然踩坑。开发前必读。

## 端口三处对齐

`SIDECAR_PORT = 9880` 在三处必须一致，改一处必须同步另两处：
- Rust：`src-tauri/src/sidecar.rs` → `SIDECAR_PORT`
- Python：`backend/core/config.py` → `SIDECAR_PORT`
- TypeScript：`src/core/constants.ts` → `SIDECAR_PORT`

## 环境变量契约对齐

Rust spawn Python 时注入的环境变量名，必须与 Python 侧读取的一致：
- `SIDECAR_DATA_DIR`：Rust `sidecar.rs DATA_DIR_ENV` ↔ Python `paths.py USER_DATA_DIR_ENV`
- `APP_ROOT`：Rust `sidecar.rs APP_ROOT_ENV` ↔ Python `paths.py _app_root()`（frozen 模式必需，缺失 fast-fail）

任一端改名未同步 → spawn 后 Python 读不到 → 全链路断。

## 前后端错误码镜像契约

`backend/core/errors.py` 的 `ErrorCode` 与 `src/core/api/types.ts` 的 `ApiErrorCode`
是同一份契约的两端实现。任一端新增/删除错误码必须同步另一端。

## PyInstaller onefile 路径漂移

打包后 `sys._MEIPASS` 是解压目录，`__file__` 不再指向源码树。路径解析集中在
`backend/core/paths.py` 的 `_is_frozen()` / `_compute_app_dir()` / `_app_root()`，
禁止在业务代码里硬编码路径或直接用 `Path(__file__).parents[N]`。

## PyInstaller 延迟 import 必须 hiddenimport（回流自 ai-audiobook v0.1.1）

PyInstaller 静态分析**扫不到函数内的延迟 import**（运行时才执行的 import）。
若 sidecar 或 worker binary 在 `main()` 的某分支内、或函数体内 import 工程内模块，
frozen binary 启动会 `ModuleNotFoundError`。

- `collect_all` 只收第三方包，**工程内模块必须显式声明** `hiddenimports`
- 跨文件 import 的符号要在 import 源定义（不能"定义在 A、从 B import、B 还没有"）
- **HTTP 冒烟测不到 stdout 握手类 binary**——若 worker 就绪契约是 stdout 第一行 JSON，
  smoke-test 必须验证握手，否则是覆盖盲区

## frozen binary 下任何 fork 必须 freeze_support（回流自 ai-audiobook v0.1.1）

PyInstaller frozen binary 下，**任何 fork 子进程（含第三方库内部 fork）都会重新执行
入口脚本 `main()`**。若入口脚本未调 `multiprocessing.freeze_support()`，fork 出的子进程
会重新跑业务逻辑（如起第二个 uvicorn 实例）→ 端口冲突 / 连锁取消任务。

- 入口脚本（`sidecar.py` / worker 入口）`main()` 开头**必须** `multiprocessing.freeze_support()`
- 即使你不直接用 multiprocessing，第三方库（如 modelscope 下载、torch DataLoader）可能内部 fork

## asyncio.to_thread 返回 coroutine 不是 Task（回流自 ai-audiobook v0.1.2）

`asyncio.to_thread(func)` 返回的是 **coroutine**，不是 `Task`。未 `ensure_future` 包成
Task 就传给轮询协程调 `.done()` / `.result()` 会报 `'coroutine' object has no attribute 'done'`。

```python
# ❌ 错误：to_thread 返回 coroutine，无法 .done()
task = asyncio.to_thread(blocking_fn)
await asyncio.wait([task], timeout=1)  # AttributeError

# ✅ 正确：ensure_future 包成 Task
task = asyncio.ensure_future(asyncio.to_thread(blocking_fn))
await asyncio.wait([task], timeout=1)
```

这是 Python 通用陷阱，任何用 `asyncio.to_thread` 做超时控制的场景适用。


## SQLite 必须开 PRAGMA

`database.py` 的 `_enable_sqlite_pragmas` 必须挂载（FK=ON + WAL + busy_timeout=5000）。
测试 fixture（conftest.py）也必须复用同一 listener，否则 FK 约束在测试中是装饰性的。

## alembic frozen 适配

- env.py 用内联 `Path(__file__).resolve().parent.parent` 相对路径（打包后落 `_MEIPASS` 下自然 work）
- env.py 必须设绝对路径 `script_location`（消除 cwd 依赖，打包后 cwd 不确定）
- SQLite 必须 `render_as_batch=True`（不支持 ALTER COLUMN）
- SQLite 必须 `connection.commit()` 显式提交 DML（alembic 1.18 + SA 2.0 坑）

## Tauri v2 setup 时序坑（setup 内 emit 事件会被丢弃）

> 回流自 pixel-matrix Stage 5A（commit `3e579aa`，2026-07-03）。
> 当前模板用 HTTP 轮询 `/api/health` 探活 sidecar，**不受此坑影响**。
> 但若子工程剥离 sidecar 改 Rust 原生后端，**必踩此坑**——提前沉淀避免将来踩。

### 现象

子工程剥离 sidecar 改 Rust 原生后端后，在 `.setup(|app| ...)` 闭包里 `emit("backend://ready", ())`，
前端用 `listen('backend://ready')` 监听。结果：**前端永远收不到事件**，启动守卫卡在
「后端初始化超时」，应用无法进入主界面。

### 根因

Tauri v2 的执行时序：

```
.build() 阶段同步执行 setup 闭包  ← ① emit 在这里发出
                ↓
            创建 WebView
                ↓
            加载页面 + 注入 __TAURI_INTERNALS__  ← ② JS 桥才可用
                ↓
            执行 JS，前端 listen() 注册 listener  ← ③ 但事件在 ① 已经被丢弃
```

`emit` 是**即发即弃、无缓冲**的——发出时若没有 listener，事件直接丢弃，不会重放。
而 setup 闭包在 WebView 创建、JS 执行之前就跑了，所以 setup 里 emit 的任何事件
前端都收不到。`RunEvent::Ready` / `on_page_load` 也救不了——前者同样早于 JS，
后者在 v2 不可靠（是 v1 的 hack，已被弃用）。

官方文档：https://v2.tauri.app/develop/calling-frontend/ 「Timing in setup hooks」
社区确认：https://github.com/orgs/tauri-apps/discussions/7558（维护者 @FabianLars 回答）

### 修正方案：前端 invoke 探活，不依赖事件

「后端是否就绪」这个问题，**用 invoke 调用本身就是答案**——能调通 command，就证明
`__TAURI_INTERNALS__` 已注入 + Rust AppState 已注册。连 emit 都不需要。

**后端**（删掉 setup 里的 emit，新增无状态 ping command）：

```rust
#[tauri::command]
fn ping() -> &'static str { "ok" }

// lib.rs
.setup(|app| {
    app.manage(AppState::new());
    // ⚠️ 不在这里 emit "backend://ready"：setup 早于 WebView JS，事件会被丢弃
    Ok(())
})
.invoke_handler(tauri::generate_handler![ping, /* ...其他 command */])
```

**前端**（从 listen 改为 invoke 探活，带重试 + 超时兜底）：

```ts
// useBackendReady.ts
import { invoke } from '@tauri-apps/api/core'

const FALLBACK_TIMEOUT_MS = 5000   // 总超时兜底
const PROBE_INTERVAL_MS = 100      // invoke 早期可能偶发失败，重试间隔

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

- ✅ **启动时同步状态 / 拿一次性数据**：invoke 调通即 ready（本方案，最简）
- ✅ **后端需持续推事件**：前端先 listen，再 invoke 触发后端 emit（事件从 command 内发出，不走 setup）
- ✅ **流式数据**：用 `Channel<T>`（绑定在 invoke 调用上，天然无时序问题）
- ❌ `RunEvent::Ready` / `on_page_load`：v1 时代的 hack，v2 不可靠

### 反模式

- ❌ setup 内 `emit` 任何前端需要 `listen` 的事件
- ❌ 25ms 轮询某个 HTTP 端点（v1 sidecar 时代的 hack，invoke 探活更简洁）

