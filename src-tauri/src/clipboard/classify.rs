//! 内容子类型识别 — 从文本内容检测 link / markdown
//!
//! 写入时调用一次,结果存入 DB content_subtype 列。
//! 规则不常变,一次识别永久存储,查询时不重复计算(性能优)。
//!
//! 只对 text/rich 类型调用;image/file/mixed 不参与(无文本或非文本主导)。

/// 内容子类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSubtype {
    /// 无子类型(普通文本)
    None,
    /// 链接(整条内容是单个 URL)
    Link,
    /// Markdown(含结构化 Markdown 标记)
    Markdown,
}

impl ContentSubtype {
    /// 数据库存储用的字符串(None → NULL,在 SQL 层处理)。
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ContentSubtype::None => "",
            ContentSubtype::Link => "link",
            ContentSubtype::Markdown => "markdown",
        }
    }

    /// 从数据库字符串解析(空/NULL → None)。
    #[allow(dead_code)]
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "link" => ContentSubtype::Link,
            "markdown" => ContentSubtype::Markdown,
            _ => ContentSubtype::None,
        }
    }

    /// 是否为有效子类型(非 None),用于决定 DB 写 NULL 还是写值。
    pub fn is_some(&self) -> bool {
        !matches!(self, ContentSubtype::None)
    }
}

/// 识别文本内容的子类型(纯函数)。
///
/// 优先级:link > markdown(一条纯 URL 更适合归为 link)。
///
/// **link**:trim 后以 `http://` / `https://` 开头,且不含换行(纯 URL)。
/// **markdown**:含至少 2 种 Markdown 结构标记(避免普通文本中的 `*` 或 `-` 误判)。
pub fn detect_subtype(text: &str) -> ContentSubtype {
    let trimmed = text.trim();

    // 空 → None
    if trimmed.is_empty() {
        return ContentSubtype::None;
    }

    // link:以 http 开头且无换行(整条是单个 URL)
    if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && !trimmed.contains('\n')
    {
        return ContentSubtype::Link;
    }

    // markdown:统计匹配的 Markdown 结构标记种类数
    let markdown_markers = count_markdown_markers(trimmed);
    if markdown_markers >= 2 {
        return ContentSubtype::Markdown;
    }

    ContentSubtype::None
}

/// 统计文本中匹配的 Markdown 结构标记种类数。
///
/// 每种标记只计一次(不管出现几次),要求 ≥2 种才算 Markdown。
fn count_markdown_markers(text: &str) -> usize {
    let mut count = 0;

    // # 标题(行首 1-6 个 # + 空格)
    if text.lines().any(|line| {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        (1..=6).contains(&hashes) && trimmed.as_bytes().get(hashes) == Some(&b' ')
    }) {
        count += 1;
    }

    // ``` 代码块
    if text.contains("```") {
        count += 1;
    }

    // []() 链接语法(Markdown 链接,非纯 URL)
    if text.contains("](") && text.contains('[') {
        count += 1;
    }

    // **加粗** 或 __加粗__
    if text.contains("**") || text.contains("__") {
        count += 1;
    }

    // - 或 * 开头的列表项(行首)
    if text.lines().any(|line| {
        let t = line.trim_start();
        (t.starts_with("- ") || t.starts_with("* ")) && t.len() > 2
    }) {
        count += 1;
    }

    // > 引用(行首)
    if text.lines().any(|line| line.trim_start().starts_with("> ")) {
        count += 1;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== link 识别 =====

    #[test]
    fn detect_http_url() {
        assert_eq!(detect_subtype("https://github.com"), ContentSubtype::Link);
        assert_eq!(detect_subtype("http://example.com/path?q=1"), ContentSubtype::Link);
    }

    #[test]
    fn detect_url_with_whitespace() {
        assert_eq!(detect_subtype("  https://github.com  "), ContentSubtype::Link);
    }

    #[test]
    fn url_with_newline_not_link() {
        // 含换行的 URL 不是纯链接(是含 URL 的段落)
        assert_ne!(detect_subtype("https://github.com\n描述文字"), ContentSubtype::Link);
    }

    #[test]
    fn non_http_not_link() {
        assert_ne!(detect_subtype("ftp://example.com"), ContentSubtype::Link);
        assert_ne!(detect_subtype("/usr/local/bin"), ContentSubtype::Link);
    }

    // ===== markdown 识别 =====

    #[test]
    fn detect_markdown_title_plus_code() {
        let md = "# 标题\n\n```\ncode\n```";
        assert_eq!(detect_subtype(md), ContentSubtype::Markdown);
    }

    #[test]
    fn detect_markdown_bold_plus_list() {
        let md = "**重要内容**\n\n- 项目一\n- 项目二";
        assert_eq!(detect_subtype(md), ContentSubtype::Markdown);
    }

    #[test]
    fn detect_markdown_link_syntax_plus_quote() {
        let md = "[点击](https://example.com)\n\n> 引用文字";
        assert_eq!(detect_subtype(md), ContentSubtype::Markdown);
    }

    // ===== None(普通文本)=====

    #[test]
    fn plain_text_none() {
        assert_eq!(detect_subtype("你好世界"), ContentSubtype::None);
        assert_eq!(detect_subtype("This is plain text."), ContentSubtype::None);
    }

    #[test]
    fn single_marker_not_markdown() {
        // 只有一种标记(如只有 **加粗**),不算 Markdown(避免误判)
        assert_eq!(detect_subtype("**加粗文字**"), ContentSubtype::None);
        // 只有一个 - 列表项
        assert_eq!(detect_subtype("- 单个列表项"), ContentSubtype::None);
    }

    #[test]
    fn empty_string_none() {
        assert_eq!(detect_subtype(""), ContentSubtype::None);
        assert_eq!(detect_subtype("   "), ContentSubtype::None);
    }

    // ===== ContentSubtype 互转 =====

    #[test]
    fn subtype_db_roundtrip() {
        for s in [ContentSubtype::None, ContentSubtype::Link, ContentSubtype::Markdown] {
            assert_eq!(ContentSubtype::from_db_str(s.as_db_str()), s);
        }
        // 未知值容错 → None
        assert_eq!(ContentSubtype::from_db_str("unknown"), ContentSubtype::None);
    }

    #[test]
    fn subtype_is_some() {
        assert!(!ContentSubtype::None.is_some());
        assert!(ContentSubtype::Link.is_some());
        assert!(ContentSubtype::Markdown.is_some());
    }
}
