//! 业务数据模型 — 与前端 serde 序列化输出对齐
//!
//! M1:ClipboardItem(剪贴板历史条目)。仅暴露给前端的字段(M1 不返回 formats 数组,
//! 只有 text,在 copy_back 时直接从 DB 读)。
//!
//! 数据库 schema 见 storage/mod.rs。本类型是「列表展示用」的精简视图,
//! 不直接映射整张表(formats 走独立的 storage 层读写)。

// M1 阶段 A:存储层先建,阶段 B(watcher)/C(command)接线前公开 API 暂未使用。
// 阶段 B/C 接线后移除此 allow。
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 剪贴板内容类型。
///
/// 序列化为小写字符串,与前端 contentKind 联合类型对齐。
/// - Text:纯文本(public.utf8-plain-text)
/// - Rich:富文本(含 RTF/HTML,且不含图片/文件)
/// - Image:图片(PNG/TIFF)
/// - File:文件 URL(public.file-url)
/// - Mixed:混合(含多种主要类型,如同时有图片和文本)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Text,
    Rich,
    Image,
    File,
    Mixed,
}

impl ContentKind {
    /// 数据库存储用的字符串。
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ContentKind::Text => "text",
            ContentKind::Rich => "rich",
            ContentKind::Image => "image",
            ContentKind::File => "file",
            ContentKind::Mixed => "mixed",
        }
    }

    /// 从数据库字符串解析(容错:未知值回退为 Text)。
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "text" => ContentKind::Text,
            "rich" => ContentKind::Rich,
            "image" => ContentKind::Image,
            "file" => ContentKind::File,
            "mixed" => ContentKind::Mixed,
            unknown => {
                log::warn!("[models] 未知 content_kind 值 {:?},回退为 text", unknown);
                ContentKind::Text
            }
        }
    }
}

/// 剪贴板历史条目(列表展示视图,与前端 ClipboardItem 接口镜像)。
///
/// 对应 `clipboard_items` 表的元数据列,不含 formats 内容。
/// formats 在 copy_back / M2 富文本场景下由 storage 层单独读写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    /// 首次复制时间(unix epoch ms)
    pub first_copied_at: i64,
    /// 最近复制时间(unix epoch ms)
    pub last_copied_at: i64,
    /// 复制次数(去重时累加)
    pub copy_count: i32,
    /// 列表预览文本(前 N 字符,方便查询展示)
    pub title: String,
    /// 内容类型(M1 固定 text)
    pub content_kind: ContentKind,
}

/// 标题(列表预览)的最大字符数。
///
/// 参考 Maccy 的截断策略:长文本只存前 N 字符做预览,
/// 完整内容在 formats 表(M1 的 data_blob 列)。
pub const TITLE_MAX_CHARS: usize = 1000;

/// 把完整文本截断成标题(预览用)。
///
/// 按 char 截断(不是字节),避免切坏多字节字符(中文)。
pub fn truncate_title(text: &str) -> String {
    text.chars().take(TITLE_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_title_handles_multibyte() {
        // 中文按 char 截断,不切坏字节
        let long: String = "你".repeat(2000);
        let title = truncate_title(&long);
        assert_eq!(title.chars().count(), TITLE_MAX_CHARS);
        assert!(title.chars().all(|c| c == '你'));
    }

    #[test]
    fn truncate_title_keeps_short_text_intact() {
        assert_eq!(truncate_title("短文本"), "短文本");
    }

    #[test]
    fn content_kind_db_roundtrip() {
        // 所有变体 round-trip
        for kind in [
            ContentKind::Text,
            ContentKind::Rich,
            ContentKind::Image,
            ContentKind::File,
            ContentKind::Mixed,
        ] {
            assert_eq!(
                ContentKind::from_db_str(kind.as_db_str()),
                kind,
                "{:?} round-trip 失败",
                kind
            );
        }
        // 未知值容错回退为 Text
        assert_eq!(ContentKind::from_db_str("unknown_kind"), ContentKind::Text);
    }

    #[test]
    fn content_kind_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&ContentKind::Text).unwrap(), "\"text\"");
        assert_eq!(serde_json::to_string(&ContentKind::Rich).unwrap(), "\"rich\"");
        assert_eq!(serde_json::to_string(&ContentKind::Image).unwrap(), "\"image\"");
        assert_eq!(serde_json::to_string(&ContentKind::File).unwrap(), "\"file\"");
        assert_eq!(serde_json::to_string(&ContentKind::Mixed).unwrap(), "\"mixed\"");
    }
}
