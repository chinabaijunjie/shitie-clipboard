//! Tauri command 注册中心
//!
//! 所有暴露给前端 invoke 的 command 按业务领域拆子模块(M0 仅 ping,
//! M1+ 加 clipboard / storage 等)。
//!
//! ⚠️ lib.rs 的 `generate_handler!` 用 `use commands::xxx;` 引入裸名注册,
//! 不在此 re-export(宏对模块路径解析有限制,use 别名最可靠)。

pub mod clipboard;
pub mod ping;
pub mod settings;
