//! 应用错误契约 — Tauri command 统一错误类型
//!
//! 设计:
//! - 所有 command 返回 `Result<T, AppError>`
//! - Tauri 自动把 `Err(AppError)` 序列化为 `{ code, message }` JSON reject 给前端
//! - 前端 `invoke.ts` 把 reject 包成 `InvokeError(code, message)` 异常
//! - 错误码两端镜像:Rust `codes` 模块 ↔ 前端 `InvokeErrorCode`
//!
//! ⚠️ 错误码新增/删除必须两端同步(constraints.md「前后端错误码镜像契约」)。
//!
//! 错误转换策略:不提供 blanket `From<E: Error>`(会把所有错误吞成 INTERNAL_ERROR,
//! 丢失错误码分类权)。调用方用 `.map_err(|e| AppError::internal(e.to_string()))`
//! 显式归类,保留错误码分类的控制权。
//!
//! M0 阶段:仅建骨架 + 通用错误码,M1 起按剪贴板业务追加(如 CLIPBOARD_READ_FAILED)。

use serde::Serialize;

/// 错误码常量(与前端 `src/core/api/invoke.ts` InvokeErrorCode 镜像)。
///
/// 保持字符串常量(非 enum derive)是为了:
/// 1. 前端能直接用字符串字面量比对,无需映射表
/// 2. 序列化到 JSON 时是裸字符串,不是 `"ErrorCode::NOT_FOUND"`
///
/// M0 阶段尚无业务 command 使用,M1 起按剪贴板业务追加(如 CLIPBOARD_READ_FAILED)。
#[allow(dead_code)]
pub mod codes {
    /// 通用未知错误
    pub const UNKNOWN: &str = "UNKNOWN";
    /// 内部错误(服务端异常,日志排查用)
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// Tauri command 统一错误类型,序列化为 `{ code, message }` 给前端。
///
/// 前端 `invoke.ts` 的 catch 块按此结构解构。
#[derive(Debug, Serialize)]
pub struct AppError {
    /// 错误码(见 [`codes`] 模块),前端据此分支处理
    pub code: String,
    /// 面向用户的友好提示(中文)
    pub message: String,
}

impl AppError {
    /// 便捷构造:已知错误码 + 自定义 message(M1 起业务 command 使用)
    #[allow(dead_code)]
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    /// 通用未知错误(M1 起业务 command 使用)
    #[allow(dead_code)]
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(codes::UNKNOWN, message)
    }

    /// 内部错误(服务端异常,日志排查用,M1 起业务 command 使用)
    #[allow(dead_code)]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL_ERROR, message)
    }
}
