//! ping command — 前端启动守卫探活用
//!
//! 设计要点(constraints.md「Tauri v2 setup 时序坑」):
//! - setup 里**不 emit** 任何事件(setup 早于 WebView JS,emit 即发即弃无缓冲)
//! - 前端用 invoke 调 `ping` 探活:能调通 = `__TAURI_INTERNALS__` 已注入 + AppState 已注册
//! - 「后端是否就绪」这个问题,invoke 调用本身就是答案
//!
//! 本 command 无状态、无副作用,纯粹供前端探活。返回固定标识便于前端调试识别。

/// 探活标识(前端可校验,区分版本/环境)
const PING_PONG: &str = "ok";

/// 无状态探活 command。前端 `invoke<string>('ping')` 调通即代表后端就绪。
#[tauri::command]
pub fn ping() -> &'static str {
    PING_PONG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_ok() {
        assert_eq!(ping(), "ok");
    }
}
