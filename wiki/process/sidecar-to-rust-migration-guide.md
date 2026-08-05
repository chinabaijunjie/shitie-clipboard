---
title: sidecar → Rust 原生后端迁移指南
tags: [迁移, sidecar, Rust, 架构演进, ping 探活]
created: 2026-07-03
updated: 2026-07-03
status: active
---
---

# sidecar → Rust 原生后端迁移指南

> **回流自**：pixel-matrix Stage 4-5A（剥离 Python sidecar 改 Rust 原生后端）
> **何时用**：当子工程后端逻辑变轻（不再需要 Python 生态的复杂库），
> 或想要单进程架构（无端口、无 spawn、无 PyInstaller 打包链）时，按本指南迁移。
> **完整规划样本**：pixel-matrix `wiki/dev/001-sidecar-removal-and-rust-backend-plan.md`

---

## 一、什么时候该迁移，什么时候不该

### ✅ 适合迁移的场景

- 后端主要是 HTTP 转发 / 调云端 API（用 `reqwest` 足够）
- 后端不需要 Python 独有生态（pandas / numpy / ML 模型推理等）
- 想要单进程、无端口、无 spawn 的简洁架构
- 想要更快的启动速度（PyInstaller onefile 首次解压需数秒~数十秒）

### ❌ 不该迁移的场景

- 后端有重量级 Python 依赖（科学计算 / ML / 复杂图像处理）
- 需要 Python 生态的快速原型验证
- 团队 Rust 经验不足

> **决策原则**：稳定 > 架构 > 维护性 > 短期成本。剥离 sidecar 是架构大改，
> 仅当「单进程简洁性 + 启动速度 + 用户分发简化」的收益 > 迁移成本时才做。

---

## 二、迁移触点清单（动哪些文件）

| 区域 | 删除 | 新增 / 改造 |
|------|------|-----------|
| **Python 后端** | `backend/` 整个目录 | — |
| **Rust 后端** | `src-tauri/src/sidecar.rs`（spawn 逻辑）| 新增 `commands/` + `providers/` + `storage/` 模块 |
| **前端通信层** | `client.ts`（axios 调 localhost）| `invoke.ts`（`@tauri-apps/api/core` invoke）|
| **启动守卫** | `useSidecarReady`（HTTP 轮询 `/api/health`）| `useBackendReady`（**invoke `ping` 探活**，见 §三）|
| **打包链** | `build-sidecar.sh` / PyInstaller spec | `build.rs` 简化（无 PyInstaller 依赖）|
| **错误契约** | axios ApiResponse 包装 | Tauri `AppError` 序列化 `{code, message}` |
| **Cargo.toml** | tokio `process` feature（sidecar spawn 用）| 保留 `rt-multi-thread` / `macros` / `sync` / `time` |
| **运行时配置** | `SIDECAR_DATA_DIR` / `APP_ROOT` 环境变量 | Tauri `app_data_dir()` 直接拿 |

---

## 三、启动守卫改造（必踩坑：Tauri v2 setup 时序）

> ⚠️ **迁移时 100% 会踩的坑**：完整根因 + 修正方案代码见
> [[../architecture/constraints]] 「Tauri v2 setup 时序坑」节。本节只讲迁移视角的要点。

### 迁移视角要点

sidecar 时代前端轮询 HTTP `/api/health`，和 Tauri setup 时序无关。迁移到 Rust 原生后，
直觉会在 setup 里 `emit("backend://ready")` 让前端 listen——**这永远收不到**（setup 早于
WebView JS 执行，emit 是即发即弃无缓冲）。前端启动守卫卡死在「后端初始化超时」。

**迁移时的正确姿势**（细节代码见 constraints.md）：
1. **后端**：setup 里只 `app.manage(AppState)`，**不 emit**；新增无状态 `ping` command
2. **前端**：`useBackendReady` 用 invoke 调 `ping` 探活（带 100ms 重试 + 5s 超时兜底），替换原 `useSidecarReady`

「后端是否就绪」=「invoke 能调通」——invoke 本身就是答案，连 emit 都不需要。

### 适用场景速查（详见 constraints.md）

- ✅ 启动拿一次性状态：invoke 探活（最简）
- ✅ 后端持续推事件：前端先 listen 再 invoke 触发后端 emit（事件从 command 内发出）
- ✅ 流式数据：用 `Channel<T>`
- ❌ setup 内 emit / `RunEvent::Ready` / `on_page_load`

---

## 四、Tauri Command 错误契约

sidecar 时代用 axios 包装 ApiResponse；迁移后用 Tauri 的 Result + AppError 序列化。

### Rust 侧

```rust
// error.rs
#[derive(Debug, Serialize)]
pub struct AppError { pub code: String, pub message: String }

// Tauri 自动把 Err 序列化为 {code, message} 给前端
#[tauri::command]
async fn some_command() -> Result<SomeResp, AppError> {
    Err(AppError { code: "NOT_FOUND".into(), message: "...".into() })
}
```

### 前端镜像错误码（两端契约）

```ts
// types.ts —— 与 Rust error.rs 的 codes 模块镜像，改名必须两端同步
export const InvokeErrorCode = {
  NOT_FOUND: 'NOT_FOUND',
  // ...
} as const

// invoke.ts 包装：把 Tauri reject 的 {code, message} 包成 InvokeError 异常
export class InvokeError extends Error {
  constructor(public code: string, message: string) { super(message) }
}

export async function call<T>(cmd: string, args?: unknown): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    const { code, message } = e as { code: string; message: string }
    throw new InvokeError(code, message)
  }
}
```

---

## 五、其他迁移注意点

### 5.1 tokio features 收紧

sidecar 时代需要 `process` feature（spawn 子进程），迁移后删掉：

```toml
# 保留
tokio = { version = "1", features = ["sync", "macros", "rt-multi-thread", "time"] }
# 删掉 process（spawn 专用）
# ❌ 不要引入 fs（constraints 禁止，文件 IO 走 spawn_blocking）
```

### 5.2 文件 IO 用 spawn_blocking

Tauri command handler 跑在主线程调度上，同步阻塞 IO 会卡 UI。文件写操作必须丢到
blocking pool（详见模板 constraints.md 「SQLite 必须开 PRAGMA」同款思想）：

```rust
// ✅ 文件写丢到 blocking pool
tokio::task::spawn_blocking(move || std::fs::write(path, bytes)).await??

// ❌ handler 内直接同步 IO，卡主线程
std::fs::write(path, bytes)
```

### 5.3 数据目录从环境变量改为 Tauri API

sidecar 时代用 `SIDECAR_DATA_DIR` 环境变量传给 Python；迁移后直接在 Rust setup 拿：

```rust
use tauri::Manager;

.setup(|app| {
    let data_dir = app.path().app_data_dir()?;  // Tauri 内置，无需环境变量
    std::fs::create_dir_all(&data_dir)?;
    Ok(())
})
```

### 5.4 删 sidecar 留档（不要直接删完）

sidecar.rs / paths.py / crypto.py 的方法论（三重兜底进程清理 / frozen 路径处理 /
Fernet 加密）对本工程已失效，但**模板仍在用**。迁移时在 `wiki/dev/` 留一份
`XXX-sidecar-removal-plan.md §删前留档`，作为回流模板的素材（pixel-matrix 这么做了）。

---

## 六、迁移决策案例参考

| 子工程 | 决策 | 依据 |
|--------|------|------|
| **pixel-matrix**（2026-07）| ✅ 剥离 sidecar 改 Rust 原生 | 后端只调云端 API（智谱/阿里）+ 想 MVP 快速启动 + 无 Python 重量级依赖 |
| _（其他子工程迁移时补此表）_ | | |

---

_本指南随子工程迁移经验持续补充。每次迁移产生的新坑 → 补到对应小节。_
