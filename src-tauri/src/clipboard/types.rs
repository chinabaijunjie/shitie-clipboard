//! Newtype 类型固化 — 编译期区分白名单 UTI / 相对路径 / 绝对路径
//!
//! 问题背景(type-design agent 建议):全代码用裸 `String`/`&str` 传 UTI 和路径,
//! 编译器无法区分「白名单 UTI vs 任意 UTI 字符串」「相对路径 vs 绝对路径」,
//! 容易把绝对路径误当相对路径 join。
//!
//! 三个 newtype:
//! - [`WhitelistUti`]:白名单 UTI(6 个已知格式,enum 穷尽安全)
//! - [`RelPath`]:相对路径(相对 images_dir,如 `"ab/abcdef.png"`)
//! - [`AbsPath`]:绝对路径(给前端 `convertFileSrc` 用)

use serde::{Deserialize, Serialize};

// ============================================================
// WhitelistUti — 白名单 UTI(enum,编译期穷尽)
// ============================================================

/// 白名单 UTI — 只可能是这 6 个已知格式之一。
///
/// 用 enum 而非 `struct(&str)`:白名单成员固定,enum 保证不存在非法值,
/// match 穷尽(加新白名单成员时编译器强制处理所有分支)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhitelistUti {
    /// 纯文本 `public.utf8-plain-text`
    PlainText,
    /// 富文本(RTF)`public.rtf`
    Rich,
    /// HTML `public.html`
    Html,
    /// PNG 图片 `public.png`
    Png,
    /// TIFF 图片 `public.tiff`
    Tiff,
    /// 文件 URL `public.file-url`
    FileUrl,
}

impl WhitelistUti {
    /// 从 UTI 字符串解析。非白名单返回 `None`。
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            crate::clipboard::uti::UTI_PLAIN_TEXT => Some(Self::PlainText),
            crate::clipboard::uti::UTI_RTF => Some(Self::Rich),
            crate::clipboard::uti::UTI_HTML => Some(Self::Html),
            crate::clipboard::uti::UTI_PNG => Some(Self::Png),
            crate::clipboard::uti::UTI_TIFF => Some(Self::Tiff),
            crate::clipboard::uti::UTI_FILE_URL => Some(Self::FileUrl),
            _ => None,
        }
    }

    /// 对应的 UTI 字符串常量。
    #[allow(dead_code)] // 公开 API,部分变体当前只在测试中使用
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlainText => crate::clipboard::uti::UTI_PLAIN_TEXT,
            Self::Rich => crate::clipboard::uti::UTI_RTF,
            Self::Html => crate::clipboard::uti::UTI_HTML,
            Self::Png => crate::clipboard::uti::UTI_PNG,
            Self::Tiff => crate::clipboard::uti::UTI_TIFF,
            Self::FileUrl => crate::clipboard::uti::UTI_FILE_URL,
        }
    }

    /// 是否为图片格式(PNG / TIFF)。
    #[allow(dead_code)] // 公开 API,预留给 copy_back assemble 等场景使用
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Png | Self::Tiff)
    }
}

// ============================================================
// RelPath — 相对路径(相对 images_dir)
// ============================================================

/// 相对路径 — 相对 `images_dir`,如 `"ab/abcdef.png"` / `"thumb/abcdef.png"`。
///
/// `#[serde(transparent)]`:序列化/反序列化为裸 String(前端只认字符串)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelPath(pub String);

impl RelPath {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    #[allow(dead_code)] // 公开 API,当前只在测试中使用
    pub fn from_str(s: &str) -> Self {
        Self(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RelPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================
// AbsPath — 绝对路径(给前端 convertFileSrc)
// ============================================================

/// 绝对路径 — `images_dir.join(rel)` 的结果,给前端 `convertFileSrc` 用。
///
/// 与 [`RelPath`] 类型隔离,编译期防止「把相对路径当绝对路径用」或反之。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AbsPath(pub String);

impl AbsPath {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    #[allow(dead_code)] // 公开 API,序列化给前端时用 serde transparent,as_str 预留给 Rust 内部
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AbsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== WhitelistUti 测试 =====

    #[test]
    fn whitelist_uti_from_str_all_members() {
        assert_eq!(WhitelistUti::from_str("public.utf8-plain-text"), Some(WhitelistUti::PlainText));
        assert_eq!(WhitelistUti::from_str("public.rtf"), Some(WhitelistUti::Rich));
        assert_eq!(WhitelistUti::from_str("public.html"), Some(WhitelistUti::Html));
        assert_eq!(WhitelistUti::from_str("public.png"), Some(WhitelistUti::Png));
        assert_eq!(WhitelistUti::from_str("public.tiff"), Some(WhitelistUti::Tiff));
        assert_eq!(WhitelistUti::from_str("public.file-url"), Some(WhitelistUti::FileUrl));
    }

    #[test]
    fn whitelist_uti_from_str_rejects_non_whitelist() {
        assert_eq!(WhitelistUti::from_str("dyn.some-dynamic"), None);
        assert_eq!(WhitelistUti::from_str("com.apple.linkpresentation.metadata"), None);
        assert_eq!(WhitelistUti::from_str("org.nspasteboard.ConcealedType"), None);
        assert_eq!(WhitelistUti::from_str(""), None);
    }

    #[test]
    fn whitelist_uti_roundtrip() {
        for uti in [
            WhitelistUti::PlainText,
            WhitelistUti::Rich,
            WhitelistUti::Html,
            WhitelistUti::Png,
            WhitelistUti::Tiff,
            WhitelistUti::FileUrl,
        ] {
            let s = uti.as_str();
            assert_eq!(WhitelistUti::from_str(s), Some(uti), "{:?} roundtrip", uti);
        }
    }

    #[test]
    fn whitelist_uti_is_image() {
        assert!(WhitelistUti::Png.is_image());
        assert!(WhitelistUti::Tiff.is_image());
        assert!(!WhitelistUti::PlainText.is_image());
        assert!(!WhitelistUti::FileUrl.is_image());
    }

    // ===== RelPath / AbsPath 测试 =====

    #[test]
    fn rel_path_construct_and_access() {
        let p = RelPath::new("ab/abc.png".to_string());
        assert_eq!(p.as_str(), "ab/abc.png");
        assert_eq!(p.to_string(), "ab/abc.png");
    }

    #[test]
    fn rel_path_from_str() {
        let p = RelPath::from_str("thumb/xyz.png");
        assert_eq!(p.as_str(), "thumb/xyz.png");
    }

    #[test]
    fn abs_path_construct_and_access() {
        let p = AbsPath::new("/app_data/images/thumb/xyz.png".to_string());
        assert_eq!(p.as_str(), "/app_data/images/thumb/xyz.png");
        assert_eq!(p.to_string(), "/app_data/images/thumb/xyz.png");
    }

    #[test]
    fn rel_path_serde_transparent() {
        let p = RelPath::new("ab/test.png".to_string());
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"ab/test.png\"");
        let back: RelPath = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn abs_path_serde_transparent() {
        let p = AbsPath::new("/abs/path.png".to_string());
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"/abs/path.png\"");
        let back: AbsPath = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn rel_abs_are_distinct_types() {
        // 编译期类型隔离:RelPath 和 AbsPath 不能直接比较(不同类型)
        let rel = RelPath::new("a".to_string());
        let abs = AbsPath::new("a".to_string());
        // 它们内部 String 相同,但类型不同 — 这正是 newtype 的意义
        assert_eq!(rel.as_str(), abs.as_str()); // 值碰巧相同
        // 以下若取消注释会编译失败(类型不匹配):
        // assert_eq!(rel, abs);
    }
}
