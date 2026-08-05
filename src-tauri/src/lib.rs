//! Clipboard Manager — Tauri 主进程
//!
//! 职责（M0 骨架阶段）：
//! - 注册 Tauri commands 暴露给前端 invoke（M0 仅 ping，M1+ 加剪贴板业务）
//! - 应用级生命周期管理（M1+ 加托盘 / 全局快捷键唤起等常驻体验）
//!
//! ⚠️ 本文件处于 M0 阶段 B/C 过渡期：sidecar 相关代码已删除，
//!    ping command / AppState 将在阶段 C 补齐。当前为编译破窗状态（预期）。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            // 阶段 C 将在此：
            // - 注册 tauri-plugin-log（保留，常驻工具排障必备，改 Warn 级）
            // - app.manage(AppState::new())（M1+ 剪贴板轮询线程 / DB 连接池等）
            // ⚠️ 不在这里 emit 任何事件：setup 早于 WebView JS，emit 即发即弃无缓冲
            //    （constraints.md「Tauri v2 setup 时序坑」）
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 阶段 C 将在此注册 ping command
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // 阶段 C 后：M1+ 在此处理 RunEvent::Exit（托盘清理 / 剪贴板监听停止等）
        });
}
