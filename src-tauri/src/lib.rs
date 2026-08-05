//! Clipboard Manager — Tauri 主进程
//!
//! 职责：
//! - 管理 Python sidecar 生命周期（启动/停止/健康检查）
//! - 注册 Tauri commands 暴露给前端 invoke
//! - 监听应用级退出事件，优雅停止 sidecar（含 Cmd+Q / 关窗 / 关机）

mod sidecar;

use sidecar::{PythonSidecar, check_sidecar_health, start_sidecar, stop_sidecar};
use tauri::{AppHandle, Manager, RunEvent, State};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 日志插件（始终启用：release .app 排障必备，写 ~/Library/Logs/{bundleId}/）。
            // 原仅 debug_assertions 注册会导致 release 日志全丢（实测 0 字节）。
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // 自动 spawn Python sidecar（生产 + tauri dev 都生效）。
            // 异步 spawn：让窗口先渲染，后端在后台起（前端 useSidecarReady 轮询等待）。
            // dev 防双开：spawn_sidecar 内部已有三重兜底清理（Child handle + PID 文件 + 端口）；
            // 手动起的 sidecar 占着端口时，自动 spawn 的新进程会因端口冲突崩溃，
            // 但手动起的仍正常服务，health 检查照常对它生效。
            let spawn_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state: State<PythonSidecar> = spawn_handle.state::<PythonSidecar>();
                match sidecar::spawn_sidecar(&spawn_handle, &state) {
                    Ok(pid) => log::info!("✅ 应用启动自动 spawn sidecar，PID={}", pid),
                    Err(e) => log::error!("❌ 自动 spawn sidecar 失败：{}", e),
                }
                // 后台健康检查（不阻塞，前端 loading 页独立轮询 /api/health）。
                match sidecar::check_health().await {
                    Ok(_) => log::info!("✅ sidecar 健康检查通过"),
                    Err(e) => log::error!("❌ sidecar 健康检查失败：{}", e),
                }
            });

            Ok(())
        })
        .manage(PythonSidecar::new())
        .invoke_handler(tauri::generate_handler![
            start_sidecar,
            stop_sidecar,
            check_sidecar_health,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 应用级生命周期事件：处理 Cmd+Q / 关窗口 / 系统关机等退出路径。
            // 之前只用窗口级 on_window_event(CloseRequested)，Cmd+Q（应用退出）
            // 不触发窗口级事件，导致 sidecar 变孤儿进程占着端口。
            // 现在用应用级 RunEvent 统一兜底，cleanup_sidecar_processes 三重兜底
            // （Child handle + PID 文件 + 端口）幂等，双触发安全。
            //
            // 用 build() + run(callback) 两步而非 Builder::run(context) 简写：
            // 后者内部用空回调 |_,_|{} 无法拦截退出事件。
            if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
                cleanup_sidecar(app_handle);
            }
        });
}

/// 应用退出时清理 Python sidecar：三重兜底确保不残留孤儿进程。
///
/// ⚠️ 不能只调 kill_managed_child：Cmd+Q 后 sidecar 可能被 launchd 接管成孤儿
/// （PPID=1，Child handle 失效），kill_managed_child 杀不到。必须加端口兜底
/// （kill_process_on_port）才能从源头杜绝孤儿占用端口。
/// cleanup_sidecar_processes 幂等（kill_managed_child take None 无操作 +
/// 端口空闲直接 return），ExitRequested + Exit 双触发也安全。
fn cleanup_sidecar(app_handle: &AppHandle) {
    log::info!("应用退出，停止 Python sidecar…");
    let state: State<PythonSidecar> = app_handle.state::<PythonSidecar>();
    let data_dir = sidecar::resolve_data_dir_for_cleanup(app_handle);
    sidecar::cleanup_sidecar_processes(&data_dir, &state);
}
