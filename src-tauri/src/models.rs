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

use crate::clipboard::types::AbsPath;
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
#[serde(rename_all = "camelCase")]
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
    /// 内容类型
    pub content_kind: ContentKind,
    /// 内容子类型("link" / "markdown" / None)。仅 text/rich 类型可能有值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_subtype: Option<String>,
    /// 是否置顶(置顶条目排最前 + 豁免自动清理)。
    #[serde(default)]
    pub pinned: bool,
    /// 置顶时间(unix epoch ms)。None = 未置顶。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<i64>,
    /// 图片原图绝对路径(仅 image/mixed 类型有,供 UI 显示存储位置)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<AbsPath>,
    /// 文件 URL(仅 file 类型有,原始文件路径引用,供 UI 显示)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    /// 图片缩略图绝对路径(仅 image/mixed 类型有,前端用 convertFileSrc 显示)。
    /// None = 非图片或无缩略图。
    ///
    /// 类型为 [`AbsPath`](crate::clipboard::types::AbsPath):`fill_image_fields` 填相对路径,
    /// `resolve_thumb_absolute`(handler/command)转成绝对路径后赋值。
    /// 序列化为裸 String(AbsPath serde transparent),前端只认字符串。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_path: Option<AbsPath>,
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

/// 存储统计(设置面板「存储」模块用,序列化给前端)。
///
/// `by_kind` 按 contentKind 分组计数;`link` / `markdown` 是预留位,
/// 当前始终为 0(后期加智能检测逻辑后填充)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    /// 历史记录总条数
    pub total: i64,
    /// 按类型分组计数
    pub by_kind: StorageByKind,
    /// 磁盘占用字节数(DB BLOB + 图片文件 size 列总和)
    pub storage_bytes: i64,
}

/// 分页列表响应(list_items / search_items 命令用,序列化给前端)。
///
/// `items` 为当前页数据,`total` 为过滤条件下的命中总数(前端据此判断 hasMore)。
/// 前端镜像:`src/core/types/models.ts` 的 ListPageSchema。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    /// 当前页条目
    pub items: Vec<ClipboardItem>,
    /// 过滤条件下的命中总数
    pub total: i64,
}

/// 按内容类型的分类计数。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageByKind {
    /// 纯文本
    pub text: i64,
    /// 富文本(含 RTF/HTML)
    pub rich: i64,
    /// 图片
    pub image: i64,
    /// 文件引用
    pub file: i64,
    /// 混合类型
    pub mixed: i64,
    /// 链接(text/rich 中识别为 link 的子类型)
    pub link: i64,
    /// Markdown(text/rich 中识别为 markdown 的子类型)
    pub markdown: i64,
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
