---
title: sidecar 方法论摘要（删前留档）
tags: [归档, 留档, sidecar, 回流素材, 模板母体]
created: 2026-08-05
updated: 2026-08-05
status: archived-extract
source_files:
  - src-tauri/src/sidecar.rs（三重兜底进程管理）
  - backend/core/paths.py（frozen/开发双模式路径解析）
  - backend/core/crypto.py（Fernet 凭据加密）
---

# sidecar 方法论摘要（删前留档）

> **本文档是 M0 架构清理的「删前留档」**（遵循 sidecar-to-rust-migration-guide.md §5.4）。
>
> **为什么留档**:Clipboard Manager 改成纯 Rust 单进程架构后,以下三个 sidecar
> 时代的方法论对本工程**已失效**(代码已删)。但**模板母体(`/Users/baijunjie/Documents/infra/tauri-sidecar-template`)
> 仍在用 sidecar 架构**,这三个方法论是母体的 S 级资产。留档作为回流模板母体的素材,
> 也作为「将来任何 Tauri + sidecar 工程都适用」的通用经验沉淀。
>
> **使用方式**:本工程 M0 删除 sidecar.rs / paths.py / crypto.py 后,这三个方法论
> 仅存于此文档。若模板母体需要这些代码,从 git 历史(commit `ac02fed` 之前)即可恢复;
> 若要回流经验到母体的 wiki,提炼本文档的「方法论要点」即可。

---

## 一、三重兜底进程清理(`sidecar.rs`)

### 解决的问题

Tauri 应用 spawn 子进程(Python sidecar / Node worker / 任意 binary)后,**子进程
孤儿化是必然踩的坑**——以下三种场景都会导致 Child handle 失效,孤儿进程占着端口:

1. **Cmd+Q / 系统关机**:子进程被 launchd/macOS 接管,PPID=1,Child handle 杀不到
2. **PID 文件对不上**:上次崩溃残留的 PID 文件指向了已被系统回收的旧 PID
3. **PyInstaller onefile 的进程结构**:bootloader 父进程 → Python 子进程,
   Rust `child.id()` 拿的是 bootloader,`child.kill()` 杀了 bootloader 后 Python 子进程
   被 launchd 接管成孤儿

### 方法论要点:三重兜底(任一兜底失效不影响整体)

```
cleanup_sidecar_processes(data_dir, state):
  1. kill_managed_child      # 通过 Child handle 杀(正常路径,app 持有 handle 时有效)
  2. kill_stale_process_from_pid_file  # 通过 PID 文件杀(handle 丢失但 PID 文件还在)
  3. kill_process_on_port    # 通过端口杀(治本,无论孤儿来源)
```

**关键洞察**:第 3 层(端口兜底)是治本的。前两层都有失效场景,但「**只要占着端口
就杀**」这一条永远成立——`lsof -ti :<port>` 拿到 PID → SIGTERM 等 300ms → SIGKILL 兜底。

### 应用场景

- 退出时(Cmd+Q / 关窗 / 关机):应用级 `RunEvent::Exit` / `ExitRequested` 拦截,
  调三重兜底(不能只用窗口级 on_window_event,Cmd+Q 不触发窗口事件)
- 启动时(spawn 新 sidecar 前):清理可能残留的旧进程,避免端口冲突

### 幂等性

三重兜底必须**幂等**(kill_managed_child take None 无操作 + 端口空闲直接 return),
因为 `ExitRequested` + `Exit` 会双触发。

---

## 二、frozen/开发双模式路径解析(`paths.py`)

### 解决的问题

PyInstaller onefile 打包后路径漂移:
- 开发期:`__file__` 指向源码树 `backend/core/paths.py`,`parents[N]` 能定位项目根
- 打包后:`__file__` 指向 `_MEIPASS`(解压临时目录),`parents[N]` 拿到的是解压目录,
  **不是真实项目根**

### 方法论要点:集中解析 + 双模式分支

```python
def _is_frozen() -> bool:
    return hasattr(sys, "_MEIPASS")

def _compute_app_dir() -> Path:
    if _is_frozen():
        return Path(sys._MEIPASS)  # 打包后:解压目录
    return Path(__file__).resolve().parent.parent.parent  # 开发:源码树根
```

**关键纪律**:
1. **路径解析集中在 paths.py**,禁止业务代码里硬编码 `Path(__file__).parents[N]`
2. **frozen 探测用 `hasattr(sys, "_MEIPASS")`**,不要用 try/except 捕获
3. **APP_ROOT 环境变量对齐**:打包后应用自包含资源(resources/)走 tauri bundle
   分发到 `resource_dir`,不在 `_MEIPASS` 里。Rust spawn 时注入 `APP_ROOT` 环境变量,
   Python 侧 `_app_root()` 读它。**frozen 下 APP_ROOT 缺失必须 fast-fail**(启动期暴露
   Rust spawn 链路异常,而非运行期迷惑性故障)

### 三个路径概念(易混淆)

| 概念 | 开发期 | 打包后 | 用途 |
|------|--------|--------|------|
| `APP_DIR` | 源码树项目根 | `_MEIPASS`(PyInstaller 解压目录) | 定位打入 binary 的代码/data |
| `APP_ROOT` | = APP_DIR | tauri `resource_dir`(Rust 注入) | 定位 resources/ 下自包含资源(ffmpeg/字体) |
| `USER_DATA_DIR` | `~/.<app-name>` | tauri `appDataDir`(Rust 注入) | 用户数据(data.db / config / master.key) |

---

## 三、Fernet 凭据加密(`crypto.py`)

### 解决的问题

应用需要存储用户提供的第三方凭据(云端模型 API key、第三方服务 token)。明文存 DB
不安全,需要一个透明、稳定、容错的加密层。

### 方法论要点

```
master key 文件(USER_DATA_DIR/master.key, 0600 权限)
        ↓ lru_cache(进程级单例)
    Fernet 实例
        ↓ encrypt/decrypt
    DB 存密文 / ORM 读明文(EncryptedText TypeDecorator 透明加解密)
```

**关键纪律**:
1. **master key 首次启动自动生成**:`Fernet.generate_key()` → 写文件 + `chmod 0o600`
   (SSH key / DB 凭据的主流存储方式)。密钥不出文件系统,不进 env / git / bundle
2. **空串透传不加密**:本地无需 key 的服务走空串。加密空串会让掩码 `len()` 判断错乱
3. **解密失败显式抛 RuntimeError**:不静默吞错。给清晰根因(master key 丢失或密文损坏),
   避免下游拿到错误值产生迷惑性故障
4. **master key 永不日志打印**

### 适用场景

任何需要存储用户提供的第三方凭据的应用。**与是否 sidecar 架构无关**——纯 Rust 工程
若有凭据存储需求,同样的方法论用 Rust 的 `age` / `ring` + `serde` 实现即可。

---

## 四、对模板母体的回流价值

| 方法论 | 对母体价值 | 对本工程(纯 Rust)价值 |
|--------|-----------|---------------------|
| 三重兜底进程清理 | ⭐⭐⭐ 母体核心 S 资产,保留 | ❌ 已删,纯 Rust 无子进程 |
| frozen/开发双模式路径 | ⭐⭐⭐ 母体 PyInstaller 必用 | ❌ 已删,无 Python |
| Fernet 凭据加密 | ⭐⭐ 凭据存储通用 | ⚠️ 若将来需要可用 Rust 重写 |

**结论**:前两个方法论是母体 sidecar 架构的命脉,母体 wiki 的 `constraints.md` 已有
沉淀(端口三处对齐 / 环境变量契约 / PyInstaller 路径漂移三节)。本工程 M0 删 sidecar 后,
`constraints.md` 里这三节会标记失效(本工程不再适用),但**代码层面的方法论仅存于此文档**
——母体 git 历史里完整代码仍在,无需额外回流代码。

---

_本文档在母体 wiki 更新对应章节后可标记 `status: superseded`。_
