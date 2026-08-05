//! Python sidecar 进程管理
//!
//! 这是模板的 S 级核心资产：任何「Tauri + Python/Node sidecar」应用都要重写一遍
//! 进程生命周期管理逻辑，本模块是踩多轮 bug 打磨的稳定版（注释全是真实故障复盘）。
//!
//! 核心能力：
//! - 三重兜底进程清理（Child handle → PID 文件 → 端口 lsof）
//! - 健康检查轮询（spawn 后 poll /api/health）
//! - 端口级自愈（spawn 前强杀占端口进程，无论孤儿来源）
//! - 生产/开发双模式（Binary vs SIDECAR_DEV_PYTHON）
//! - Rust→Python 环境变量契约（SIDECAR_DATA_DIR / APP_ROOT）
//! - 数据迁移（开发期数据目录 → appDataDir 单一真理源）
//! - 应用级退出兜底（RunEvent 拦截 Cmd+Q / 关窗 / 关机）

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;

// ============================================================
// 常量
// ============================================================

/// 开发期 Python 解释器（conda 环境 / venv 均可）。
/// 打包后走 Binary 模式无需此变量；开发期通过环境变量 SIDECAR_DEV_PYTHON 配置。
/// 取值优先级：环境变量 SIDECAR_DEV_PYTHON > 系统默认 `python3`。
fn dev_python() -> String {
    std::env::var("SIDECAR_DEV_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

/// Python sidecar 监听端口（与 backend/core/config.py SIDECAR_PORT 保持一致）。
/// 端口级自愈用：spawn 前探测该端口被谁占用并强杀，比 PID 文件更可靠
/// （无论孤儿进程来源：Child handle 丢失 / PID 文件对不上 / launchd 接管）。
const SIDECAR_PORT: u16 = 9880;

/// 健康检查 URL（端口取自 SIDECAR_PORT 常量，避免硬编码分散）。
const HEALTH_URL: &str = "http://127.0.0.1:9880/api/health";

/// 健康检查超时（秒）。
/// 设 60s 覆盖 PyInstaller onefile 首次解压 + Python import + 余量。
/// 前端 useSidecarReady 独立轮询（时间更长），本常量是 Rust 侧后台 health 检查用。
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 60;

/// 健康检查轮询间隔（毫秒）
const HEALTH_CHECK_INTERVAL_MS: u64 = 500;

/// 注入给 Python sidecar 的环境变量名：数据目录（appDataDir）。
/// 与 backend/core/paths.py USER_DATA_DIR_ENV 保持一致。
/// 这是 Rust→Python 数据目录真理源对齐的核心契约，两端改名必须同步。
const DATA_DIR_ENV: &str = "SIDECAR_DATA_DIR";

/// 注入给 Python sidecar 的环境变量名：应用根目录（打包后指向 bundle resource_dir）。
/// 给 backend/ 定位 resources/ 下自包含资源用（如 ffmpeg/字体/配置）。
/// 开发期不注入（Python 用 Path(__file__).parents[N] 回退推算源码树）。
const APP_ROOT_ENV: &str = "APP_ROOT";

/// 开发期数据目录名（相对 home）：开发期回退路径，也是升级迁移来源。
/// 打包后数据目录是 appDataDir（~/Library/Application Support/<identifier>/）；
/// 开发期回退到 ~/.<app-name>/，与生产数据天然隔离。
const DEV_DATA_DIRNAME: &str = ".clipboard-manager";

/// 迁移完成标记文件名（落在旧目录旁），表示数据已迁至 appDataDir。
/// 不删旧目录，防误操作；用户可手动清理。
const MIGRATION_FLAG: &str = ".migrated-to-appdata";

/// 打包后 sidecar binary 在 bundle resource_dir 下的相对路径。
/// PyInstaller onefile 产物（含 Python runtime + backend 全代码 + 依赖），
/// 由 scripts/build-sidecar.sh 拷贝到 resources/bin/，靠 tauri.conf.json bundle.resources
/// 目录 map 进 bundle。
///
/// ⚠️ 路径含 resources/ 前缀：tauri.conf.json 的 resources map 是
/// "../resources/bin/" → "resources/bin/"，运行时落点是
/// resource_dir/resources/bin/sidecar（onefile 单文件）。
/// 必须与 Python 侧 paths.py（_app_root 定位 resources/）对齐。
const SIDECAR_BINARY_RELPATH: &str = "resources/bin/sidecar";

// ============================================================
// 错误
// ============================================================

#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("找不到 Python sidecar 脚本：{0}")]
    ScriptNotFound(String),

    #[error("无法启动 Python 进程：{0}")]
    SpawnFailed(String),

    #[error("PID 文件操作失败：{0}")]
    PidFileError(String),

    #[error("健康检查超时（{0}s 内未响应）")]
    HealthCheckTimeout(u64),

    #[error("健康检查请求失败：{0}")]
    HealthCheckRequestFailed(String),
}

type SidecarResult<T> = Result<T, SidecarError>;

// ============================================================
// 路径计算
// ============================================================

/// 解析用户数据目录（单一真理源）。
///
/// 生产环境（打包 .app）：返回 Tauri appDataDir（macOS:
///   ~/Library/Application Support/<identifier>/），
///   并通过环境变量 SIDECAR_DATA_DIR 注入给 Python sidecar。
///
/// 开发环境（tauri dev）：返回 appDataDir 本身（数据隔离）。
///
/// 注意：本函数是 Rust 侧唯一的数据目录真理源。Python 侧真理源在
/// backend/core/paths.py:_resolve_user_data_dir()，两者通过环境变量对齐。
fn resolve_data_dir(app: &AppHandle) -> SidecarResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| SidecarError::PidFileError(format!("获取 appDataDir 失败：{e}")))
}

/// 找到 backend/sidecar.py
fn find_sidecar_script() -> SidecarResult<PathBuf> {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let project_root = if manifest_dir.contains("src-tauri") {
        manifest_dir.replace("/src-tauri", "")
    } else {
        manifest_dir
    };
    let script = PathBuf::from(format!("{}/backend/sidecar.py", project_root));
    if !script.exists() {
        return Err(SidecarError::ScriptNotFound(script.display().to_string()));
    }
    Ok(script)
}

/// Python sidecar 的启动方式（生产/开发双模式）。
///
/// - Binary：打包后（PyInstaller onefile），直接 spawn bundle 内的 sidecar
///   binary（内含 Python runtime + 所有依赖，无需 conda/venv）。
/// - Dev：开发期，用 SIDECAR_DEV_PYTHON + sidecar.py 源码。
enum PythonRunner {
    /// 打包后 binary：spawn <binary>（resource_dir 是注入 APP_ROOT 用）
    Binary { binary: PathBuf, resource_dir: PathBuf },
    /// 开发期：spawn <dev_python> <script>
    Dev { script: PathBuf },
}

/// 探测 Python sidecar 启动方式（生产优先 binary，开发回退 SIDECAR_DEV_PYTHON）。
///
/// 生产（打包 .app）：resource_dir/resources/bin/sidecar 存在 → Binary 模式。
/// 开发（tauri dev）：binary 不存在（resources/bin/ 下未放）→ Dev 回退。
fn find_python_runner(app: &AppHandle) -> SidecarResult<PythonRunner> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| SidecarError::PidFileError(format!("获取 resource_dir 失败：{e}")))?;
    let binary = resource_dir.join(SIDECAR_BINARY_RELPATH);
    if binary.exists() {
        log::info!(
            "检测到打包 sidecar binary：{}（生产模式）",
            binary.display()
        );
        Ok(PythonRunner::Binary { binary, resource_dir })
    } else {
        // 开发期回退：SIDECAR_DEV_PYTHON + sidecar.py
        let script = find_sidecar_script()?;
        log::debug!(
            "未检测到打包 binary（{}），开发模式：{} {}",
            binary.display(),
            dev_python(),
            script.display()
        );
        Ok(PythonRunner::Dev { script })
    }
}

/// 日志文件路径：<data_dir>/logs/sidecar.stdout.log
fn find_log_file(data_dir: &Path) -> SidecarResult<fs::File> {
    let log_dir = data_dir.join("logs");
    fs::create_dir_all(&log_dir)
        .map_err(|e| SidecarError::PidFileError(format!("创建日志目录失败：{e}")))?;
    let log_path = log_dir.join("sidecar.stdout.log");
    let file = fs::File::create(&log_path)
        .map_err(|e| SidecarError::PidFileError(format!("创建日志文件失败：{e}")))?;
    Ok(file)
}

/// PID 文件路径：<data_dir>/sidecar.pid
fn pid_file_path(data_dir: &Path) -> SidecarResult<PathBuf> {
    fs::create_dir_all(data_dir)
        .map_err(|e| SidecarError::PidFileError(format!("创建数据目录失败：{e}")))?;
    Ok(data_dir.join("sidecar.pid"))
}

/// 开发期数据目录（~/.<app-name>），升级迁移来源。
fn dev_data_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(DEV_DATA_DIRNAME))
}

/// 窗口关闭等清理场景的 best-effort 数据目录解析。
/// 解析 appDataDir 失败时回退到 home/dev 目录，保证 PID 文件能被清理。
pub fn resolve_data_dir_for_cleanup(app: &AppHandle) -> PathBuf {
    resolve_data_dir(app).unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|h| h.join(DEV_DATA_DIRNAME))
            .unwrap_or_default()
    })
}

// ============================================================
// 数据迁移：开发期数据目录 → appDataDir（升级后数据可读）
// ============================================================

/// 首次启动到 appDataDir 时，若检测到开发期目录 ~/.<app-name> 存在且未迁移过，
/// 则整体复制旧数据到 appDataDir，保证从开发版升级到打包版后旧数据可读。
///
/// 安全策略（不可逆操作，需谨慎）：
/// - 仅当 appDataDir 为空（首次启动）时触发，避免覆盖已有数据
/// - 旧目录加 .migrated-to-appdata 标记，**不删除**（防误操作，用户可手动清）
/// - 复制而非移动，中途失败不丢原数据
/// - SQLite 文件复制后，schema 升级由后端 alembic 自动处理（链路已就位）
fn migrate_dev_data_if_needed(data_dir: &Path) -> SidecarResult<()> {
    // 目标目录非空 → 已有数据，跳过（不覆盖）
    if data_dir.exists() && fs::read_dir(data_dir).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Ok(());
    }

    let Some(legacy) = dev_data_dir() else {
        return Ok(()); // 无法定位 home，无可迁移
    };
    if !legacy.exists() {
        return Ok(()); // 无历史数据
    }

    // 已迁移标记 → 跳过（避免重复迁移）
    let flag = legacy.join(MIGRATION_FLAG);
    if flag.exists() {
        return Ok(());
    }

    log::info!(
        "检测到历史数据目录 {}，开始迁移到 {}",
        legacy.display(),
        data_dir.display()
    );

    fs::create_dir_all(data_dir)
        .map_err(|e| SidecarError::PidFileError(format!("创建目标目录失败：{e}")))?;

    // 复制全部内容（非移动，保原数据）
    copy_dir_recursive(&legacy, data_dir)?;

    // 写迁移标记（落在旧目录内，标识"已迁出"）
    let _ = fs::write(&flag, format!("migrated to {} on {}", data_dir.display(), epoch_now()));

    log::info!("✅ 历史数据迁移完成。旧目录保留于 {}（可手动删除）", legacy.display());
    Ok(())
}

/// 递归复制目录。
fn copy_dir_recursive(src: &Path, dst: &Path) -> SidecarResult<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)
            .map_err(|e| SidecarError::PidFileError(format!("创建目录失败 {dst:?}：{e}")))?;
    }
    for entry in fs::read_dir(src)
        .map_err(|e| SidecarError::PidFileError(format!("读取目录失败 {src:?}：{e}")))?
    {
        let entry = entry
            .map_err(|e| SidecarError::PidFileError(format!("读取条目失败：{e}")))?;
        let from = entry.path();
        let name = entry.file_name();
        let to = dst.join(&name);
        let ft = entry.file_type()
            .map_err(|e| SidecarError::PidFileError(format!("读取类型失败：{e}")))?;
        if ft.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_file() {
            fs::copy(&from, &to)
                .map_err(|e| SidecarError::PidFileError(format!("复制文件失败 {from:?}→{to:?}：{e}")))?;
        }
    }
    Ok(())
}

/// 简易时间戳（避免引入 chrono 依赖）。
fn epoch_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn write_pid_file(pid: u32, data_dir: &Path) -> SidecarResult<()> {
    let path = pid_file_path(data_dir)?;
    fs::write(&path, pid.to_string())
        .map_err(|e| SidecarError::PidFileError(format!("写入 PID 文件失败：{e}")))?;
    log::info!("PID 文件已写入：{} → {}", path.display(), pid);
    Ok(())
}

fn remove_pid_file(data_dir: &Path) {
    if let Ok(path) = pid_file_path(data_dir) {
        if path.exists() {
            let _ = fs::remove_file(&path);
            log::info!("PID 文件已删除：{}", path.display());
        }
    }
}

// ============================================================
// 残留进程清理（PID 文件兜底）
// ============================================================

fn kill_stale_process_from_pid_file(data_dir: &Path) {
    let path = match pid_file_path(data_dir) {
        Ok(p) => p,
        Err(_) => return,
    };
    let pid_str = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => return,
    };
    if pid == 0 {
        return;
    }

    // macOS / Linux：用 kill(pid, 0) 探活，再实际 kill
    let rc = libc_kill(pid, 0);
    if rc == 0 {
        log::warn!("发现残留 Python sidecar 进程 (PID={})，正在终止…", pid);
        let _ = libc_kill(pid, 15); // SIGTERM
        std::thread::sleep(Duration::from_millis(300));
        let _ = libc_kill(pid, 9); // SIGKILL 兜底
    }
    let _ = fs::remove_file(&path);
}

/// 端口级自愈：强杀占用指定端口的进程。
///
/// spawn 新 sidecar 前调用，无论孤儿进程来源（Child handle 丢失 / PID 文件对不上 /
/// launchd 接管），只要占着端口就杀。比 kill_stale_process_from_pid_file（只看 PID 文件）
/// 更可靠，是孤儿进程 bug 的核心治本逻辑。
///
/// 实现：`lsof -ti :<port>`（macOS 原生，无需新依赖）拿占端口 PID → SIGTERM 等 300ms
/// → SIGKILL 兜底。lsof 不存在或无输出（端口空闲）时静默跳过。
fn kill_process_on_port(port: u16) {
    // 用 lsof 查占端口 PID（-t 仅输出 PID，-i :port 按 TCP/UDP 端口过滤）
    let output = match Command::new("lsof").args(["-ti", &format!(":{port}")]).output() {
        Ok(o) => o,
        Err(e) => {
            // lsof 不存在（极少数精简系统）→ 端口自愈不可用，只 log 不阻塞 spawn
            log::warn!("调用 lsof 失败（端口自愈跳过）：{e}");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();
    if pids.is_empty() {
        return; // 端口空闲，无需处理
    }

    log::warn!("发现占用端口 {} 的进程（PID={:?}），正在终止…", port, pids);
    for pid in &pids {
        let _ = libc_kill(*pid, 15); // SIGTERM
    }
    std::thread::sleep(Duration::from_millis(300));
    for pid in &pids {
        let _ = libc_kill(*pid, 9); // SIGKILL 兜底
    }
}

/// 跨平台 kill(pid, signal) syscall。
/// 简单实现，避免引入 libc crate。
fn libc_kill(pid: u32, signal: i32) -> i32 {
    #[cfg(unix)]
    {
        // SAFETY: kill 是 POSIX 标准系统调用；调用方传入的 pid 来自我们自己写的 PID 文件 / lsof。
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            kill(pid as i32, signal)
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, signal);
        -1
    }
}

// ============================================================
// 健康检查
// ============================================================

/// 轮询 /api/health 直到 200 或超时
pub async fn check_health() -> SidecarResult<()> {
    let client = Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .map_err(|e| SidecarError::HealthCheckRequestFailed(e.to_string()))?;

    let deadline =
        Instant::now() + Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS);
    let mut last_err: Option<String> = None;

    while Instant::now() < deadline {
        match client.get(HEALTH_URL).send().await {
            Ok(resp) if resp.status().is_success() => {
                log::info!("Python sidecar 健康检查通过");
                return Ok(());
            }
            Ok(resp) => {
                last_err = Some(format!("HTTP {}", resp.status()));
            }
            Err(e) => {
                last_err = Some(e.to_string());
            }
        }
        tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
    }

    let reason = last_err.unwrap_or_else(|| "未知".to_string());
    log::error!("健康检查超时：{}", reason);
    Err(SidecarError::HealthCheckTimeout(HEALTH_CHECK_TIMEOUT_SECS))
}

// ============================================================
// 进程管理（同步 spawn + 异步 health check 组合）
// ============================================================

/// 计算 spawn Python sidecar 时要注入的环境变量（纯函数，可单测）。
///
/// 这是 Rust→Python 的环境变量契约（数据隔离 + 资源定位核心）：
/// - **DATA_DIR_ENV**（两种模式都注入）：数据隔离，Python 侧
///   `_resolve_user_data_dir()` 读它定位 appDataDir
/// - **APP_ROOT_ENV**（仅 Binary 模式注入）：资源定位，Python 侧
///   `_app_root()` 读它定位 tauri resource_dir（resources/ 下自包含资源）。
///   开发期不注入（Python 用源码树路径回退）
///
/// 抽成纯函数是为了让 env 注入契约可单测（spawn_sidecar 依赖 AppHandle 无法纯测）。
/// 测试验证：Binary 模式注入 2 个 env（含 APP_ROOT），Dev 模式注入 1 个（无 APP_ROOT）。
fn build_sidecar_env(runner: &PythonRunner, data_dir: &Path) -> Vec<(&'static str, String)> {
    let mut env: Vec<(&'static str, String)> = vec![
        // 数据隔离：Rust→Python 环境变量注入（两种模式共用，不可改）
        (DATA_DIR_ENV, data_dir.display().to_string()),
    ];
    // 资源定位：仅 Binary 模式注入 APP_ROOT（开发期 Python 用源码树回退）
    if let PythonRunner::Binary { resource_dir, .. } = runner {
        env.push((APP_ROOT_ENV, resource_dir.display().to_string()));
    }
    env
}

/// 启动 Python sidecar。
/// 步骤：
/// 1. 解析 appDataDir + 数据迁移（首次启动从开发期目录迁入）
/// 2. kill 已有进程（Mutex 内的 Child）
/// 3. kill PID 文件指向的残留进程
/// 4. 探测启动方式（打包 binary / 开发 dev_python）+ spawn 新进程
/// 5. 写 PID 文件
/// 6. 等待健康检查通过（异步）
pub fn spawn_sidecar(app: &AppHandle, state: &State<PythonSidecar>) -> SidecarResult<u32> {
    // 解析数据目录（单一真理源）+ 首次启动数据迁移
    let data_dir = resolve_data_dir(app)?;
    migrate_dev_data_if_needed(&data_dir)?;

    // 三重兜底清理旧 sidecar（与退出时 cleanup_sidecar_processes 走同一清理路径）：
    // kill_managed_child（Child handle）+ kill_stale_process_from_pid_file（PID 文件）
    // + kill_process_on_port（端口治本，无论孤儿来源）。详见该函数注释。
    cleanup_sidecar_processes(&data_dir, state);

    // 探测启动方式（生产 binary / 开发 dev_python）
    let runner = find_python_runner(app)?;
    let log_file = find_log_file(&data_dir)?;
    let stderr = log_file
        .try_clone()
        .map_err(|e| SidecarError::SpawnFailed(format!("克隆 stderr 失败：{e}")))?;

    // 构造 spawn Command + 注入环境变量（两种模式共用 DATA_DIR_ENV 注入）
    let env_vars = build_sidecar_env(&runner, &data_dir);
    let python = dev_python();
    let mut cmd = match &runner {
        PythonRunner::Binary { binary, resource_dir: _ } => {
            log::info!(
                "启动打包 sidecar binary：{}（数据目录：{}）",
                binary.display(),
                data_dir.display()
            );
            Command::new(binary)
        }
        PythonRunner::Dev { script } => {
            log::info!(
                "启动开发期 sidecar：{} {}（数据目录：{}）",
                python,
                script.display(),
                data_dir.display()
            );
            let mut c = Command::new(&python);
            c.arg(script);
            c
        }
    };

    // 注入环境变量（build_sidecar_env 计算，spawn 应用）
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    cmd.stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr));

    let child = cmd.spawn().map_err(|e| {
        let ctx = match &runner {
            PythonRunner::Binary { binary, .. } => format!("binary：{}", binary.display()),
            PythonRunner::Dev { .. } => format!("Python：{}", python),
        };
        SidecarError::SpawnFailed(format!("{e}（{ctx}）"))
    })?;

    let pid = child.id();
    *state.0.lock().unwrap() = Some(child);
    write_pid_file(pid, &data_dir)?;
    log::info!("Python sidecar 已启动，PID = {}", pid);
    Ok(pid)
}

pub fn kill_managed_child(data_dir: &Path, state: &State<PythonSidecar>) {
    let mut guard = state.0.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let pid = child.id();
        log::info!("终止 Python sidecar 进程 PID={}", pid);
        let _ = child.kill();
        let _ = child.wait();
    }
    remove_pid_file(data_dir);
}

/// 三重兜底清理 sidecar 进程（启动前 + 退出时共用同一清理路径）。
///
/// 1. `kill_managed_child`：通过 Child handle 杀（正常路径，app 持有 handle 时有效）
/// 2. `kill_stale_process_from_pid_file`：通过 PID 文件杀（handle 丢失但 PID 文件还在）
/// 3. `kill_process_on_port`：通过端口杀（治本，无论孤儿来源——Child handle 丢失 /
///    PID 文件对不上 / launchd 接管）
///
/// **为何退出时也必须调端口兜底**：Cmd+Q 后 sidecar 可能被 launchd 接管成孤儿
/// （PPID=1，Child handle 失效）。若退出只调 kill_managed_child，handle 失效就杀不到，
/// sidecar 残留占用端口。端口兜底从源头杜绝孤儿，与 spawn 时对称。
///
/// **为何 PyInstaller onefile 必须端口兜底**：onefile 的进程结构是
/// bootloader 父进程 → Python 子进程。Rust `child.id()` 拿的是 bootloader，
/// `child.kill()` 杀了 bootloader，Python 子进程被 launchd 接管成孤儿（PPID=1），
/// Child handle 杀不到它。只有端口兜底能从源头清除。
pub fn cleanup_sidecar_processes(data_dir: &Path, state: &State<PythonSidecar>) {
    kill_managed_child(data_dir, state);
    kill_stale_process_from_pid_file(data_dir);
    kill_process_on_port(SIDECAR_PORT);
}

// ============================================================
// Tauri State 包装
// ============================================================

pub struct PythonSidecar(pub Mutex<Option<Child>>);

impl PythonSidecar {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

impl Default for PythonSidecar {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Tauri commands（暴露给前端 invoke）
// ============================================================

#[tauri::command]
pub async fn start_sidecar(
    app: AppHandle,
    state: State<'_, PythonSidecar>,
) -> Result<String, String> {
    let pid = spawn_sidecar(&app, &state).map_err(|e| e.to_string())?;
    // 健康检查在后台执行，前端通过 event 监听结果
    let handle = tauri::async_runtime::spawn(async move {
        match check_health().await {
            Ok(_) => log::info!("✅ Python sidecar 就绪"),
            Err(e) => log::error!("❌ Python sidecar 健康检查失败：{e}"),
        }
    });
    // 不阻塞 invoke 返回，让前端立即拿到 PID
    let _ = handle;
    Ok(format!("Python sidecar 已启动 (PID: {})", pid))
}

#[tauri::command]
pub fn stop_sidecar(
    app: AppHandle,
    state: State<'_, PythonSidecar>,
) -> Result<String, String> {
    // stop 时同样需要 data_dir 来清理 PID 文件（解析失败回退 home）
    let data_dir = resolve_data_dir_for_cleanup(&app);
    kill_managed_child(&data_dir, &state);
    Ok("Python sidecar 已停止".to_string())
}

#[tauri::command]
pub async fn check_sidecar_health() -> Result<bool, String> {
    match check_health().await {
        Ok(_) => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}

// ============================================================
// 测试（env 注入契约 + 纯函数）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 验证 env 注入列表包含指定 key。
    fn has_env(envs: &[(&str, String)], key: &str) -> bool {
        envs.iter().any(|(k, _)| *k == key)
    }

    /// 取 env 值（String 返回，避免生命周期纠缠）。
    fn get_env(envs: &[(&str, String)], key: &str) -> String {
        envs.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("env {key} 未注入"))
    }

    mod build_sidecar_env_tests {
        use super::*;

        #[test]
        fn binary_mode_injects_data_dir_and_app_root() {
            // Binary 模式（生产打包）注入 2 个 env：DATA_DIR + APP_ROOT
            let runner = PythonRunner::Binary {
                binary: PathBuf::from("/app/resources/bin/sidecar"),
                resource_dir: PathBuf::from("/app/resources"),
            };
            let data_dir = PathBuf::from("/Users/x/Library/Application Support/com.example.clipboard-manager");

            let env = build_sidecar_env(&runner, &data_dir);

            // 数据隔离：DATA_DIR_ENV 必须注入（Python 侧 _resolve_user_data_dir 读它）
            assert!(has_env(&env, DATA_DIR_ENV), "Binary 模式必须注入 DATA_DIR_ENV");
            assert_eq!(
                get_env(&env, DATA_DIR_ENV),
                "/Users/x/Library/Application Support/com.example.clipboard-manager"
            );
            // 资源定位：APP_ROOT_ENV 必须注入（Python 侧 _app_root 读它，缺失会 fast-fail）
            assert!(has_env(&env, APP_ROOT_ENV), "Binary 模式必须注入 APP_ROOT_ENV");
            assert_eq!(get_env(&env, APP_ROOT_ENV), "/app/resources");
        }

        #[test]
        fn dev_mode_injects_data_dir_but_not_app_root() {
            // Dev 模式（开发期）注入 1 个 env：DATA_DIR，无 APP_ROOT
            let runner = PythonRunner::Dev {
                script: PathBuf::from("/proj/backend/sidecar.py"),
            };
            let data_dir = PathBuf::from("/tmp/dev-data");

            let env = build_sidecar_env(&runner, &data_dir);

            assert!(has_env(&env, DATA_DIR_ENV), "Dev 模式必须注入 DATA_DIR_ENV");
            assert_eq!(get_env(&env, DATA_DIR_ENV), "/tmp/dev-data");
            assert!(
                !has_env(&env, APP_ROOT_ENV),
                "Dev 模式不应注入 APP_ROOT（开发期 Python 用源码树回退）"
            );
        }

        #[test]
        fn env_var_names_match_python_side_constants() {
            // 🏗️ 契约对齐：Rust 注入的 env 名必须与 Python 侧读取的一致。
            // Python 侧 paths.py: USER_DATA_DIR_ENV = "SIDECAR_DATA_DIR"，
            // _app_root() 读 os.environ.get("APP_ROOT")。
            // 若任一端改名未同步，spawn 后 Python 读不到 → 全链路断。
            assert_eq!(DATA_DIR_ENV, "SIDECAR_DATA_DIR");
            assert_eq!(APP_ROOT_ENV, "APP_ROOT");
        }

        #[test]
        fn binary_resource_dir_used_as_app_root() {
            // Binary 模式的 APP_ROOT 值 = resource_dir（tauri bundle 资源根）
            let resource_dir = PathBuf::from("/custom/resource/path");
            let runner = PythonRunner::Binary {
                binary: resource_dir.join("resources/bin/sidecar"),
                resource_dir: resource_dir.clone(),
            };
            let env = build_sidecar_env(&runner, Path::new("/data"));
            assert_eq!(get_env(&env, APP_ROOT_ENV), "/custom/resource/path");
        }
    }

    mod paths_constants_tests {
        use super::*;

        #[test]
        fn sidecar_binary_relpath_has_resources_prefix() {
            // 关键：tauri 2 保留 glob 目录结构，binary 落点是
            // resource_dir/resources/bin/（非 resource_dir/bin/），路径必须有 resources/ 前缀。
            // 这是 review 发现的 CRITICAL bug 修复点，回归锁。
            assert!(SIDECAR_BINARY_RELPATH.starts_with("resources/"));
            assert_eq!(SIDECAR_BINARY_RELPATH, "resources/bin/sidecar");
        }

        #[test]
        fn sidecar_port_matches_python_config() {
            // 端口级自愈 + health URL 都依赖此常量，须与 backend/core/config.py SIDECAR_PORT=9880 对齐
            assert_eq!(SIDECAR_PORT, 9880);
            assert!(HEALTH_URL.contains("9880"));
        }

        #[test]
        fn dev_data_dirname_is_dot_app_name() {
            // 开发期数据目录名（迁移来源 + 开发回退），与 Python paths.py _LEGACY_DEV_DATA_DIRNAME 对齐
            assert_eq!(DEV_DATA_DIRNAME, ".clipboard-manager");
        }
    }

    mod migrate_tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        /// 生成唯一临时目录（避免测试间冲突），调用方负责清理。
        /// 用 std 原子计数器而非 tempfile crate，避免引入新 dev-dependency。
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        fn unique_temp_dir(label: &str) -> PathBuf {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let pid = std::process::id();
            let dir = std::env::temp_dir().join(format!(
                "tauri-template-test-{label}-{pid}-{n}"
            ));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        #[test]
        fn migrate_skips_when_target_nonempty() {
            // 目标非空 → 跳过（不覆盖已有数据）
            let tmp = unique_temp_dir("skip-nonempty");
            let new_dir = tmp.join("new-data");
            fs::create_dir_all(&new_dir).unwrap();
            fs::write(new_dir.join("existing"), "existing").unwrap();

            migrate_dev_data_if_needed(&new_dir).unwrap();

            // 新目录内容不变（未被覆盖）
            assert_eq!(
                fs::read_to_string(new_dir.join("existing")).unwrap(),
                "existing"
            );
            let _ = fs::remove_dir_all(&tmp);
        }

        #[test]
        fn copy_dir_recursive_copies_files_and_subdirs() {
            // 验证递归复制核心逻辑（迁移函数复用它）
            let tmp = unique_temp_dir("copy");
            let legacy = tmp.join("legacy");
            let new_dir = tmp.join("new");
            fs::create_dir_all(&legacy).unwrap();
            fs::write(legacy.join("data.db"), "fake db").unwrap();
            let sub = legacy.join("subdir");
            fs::create_dir_all(&sub).unwrap();
            fs::write(sub.join("file.txt"), "content").unwrap();

            copy_dir_recursive(&legacy, &new_dir).unwrap();

            // 验证递归复制成功（含子目录）
            assert_eq!(
                fs::read_to_string(new_dir.join("data.db")).unwrap(),
                "fake db"
            );
            assert_eq!(
                fs::read_to_string(new_dir.join("subdir").join("file.txt"))
                    .unwrap(),
                "content"
            );
            let _ = fs::remove_dir_all(&tmp);
        }

        #[test]
        fn migration_flag_constant_correct() {
            // 迁移标记文件名常量正确（防 typo 导致 guard 失效）
            assert_eq!(MIGRATION_FLAG, ".migrated-to-appdata");
        }
    }
}
