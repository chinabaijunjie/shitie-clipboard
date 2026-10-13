//! 标题生成 — 按 contentKind 从捕获格式生成列表预览标题
//!
//! 技术依据见 m2-research.md §三(标题生成策略表)。
//! 参考 Maccy previewableText 优先级:file-url → text → rtf.string → html.string。
//!
//! 范围:text / rich(从 RTF/HTML 提取纯文本)/ file(取文件名)/ image(图片 W×H)。

use crate::clipboard::capture::CapturedFormat;
use crate::clipboard::uti;
use crate::models::{truncate_title, ContentKind};

/// 从捕获的格式生成列表预览标题。
///
/// 按 contentKind 分支:
/// - Text:取纯文本前 N 字符
/// - Rich:从 RTF/HTML 提取纯文本(优先 text 格式,无则粗提取)
/// - File:从 file-url 取文件名(多文件:首个 + 「 等 N 项」)
/// - Image:用尺寸生成「图片 W×H」(无尺寸则「图片」)
/// - Mixed:有图片尺寸用图片标题,否则取纯文本
pub fn generate_title(
    formats: &[CapturedFormat],
    kind: ContentKind,
    image_size: Option<(u32, u32)>,
) -> String {
    match kind {
        ContentKind::Text | ContentKind::Rich => title_from_text_or_rich(formats),
        ContentKind::File => title_from_file_url(formats),
        ContentKind::Image => title_for_image(image_size),
        ContentKind::Mixed => {
            // 混合:优先图片标题(若有尺寸),否则纯文本
            if let Some((w, h)) = image_size {
                truncate_title(&format!("图片 {w}×{h}"))
            } else {
                title_from_text_or_rich(formats)
            }
        }
    }
}

/// 生成图片标题:「图片 W×H」(无尺寸则「图片」)。
fn title_for_image(image_size: Option<(u32, u32)>) -> String {
    match image_size {
        Some((w, h)) => truncate_title(&format!("图片 {w}×{h}")),
        None => truncate_title("图片"),
    }
}

/// 从文本/富文本格式生成标题。
///
/// 优先级:plain-text → rtf(粗提取)→ html(粗提取)。
fn title_from_text_or_rich(formats: &[CapturedFormat]) -> String {
    // 优先纯文本
    if let Some(text) = find_text(formats, uti::UTI_PLAIN_TEXT) {
        return truncate_title(&text);
    }
    // 其次 RTF(粗提取:剥离控制词)
    if let Some(rtf) = find_text(formats, uti::UTI_RTF) {
        return truncate_title(&extract_text_from_rtf(&rtf));
    }
    // 最后 HTML(粗提取:剥离标签)
    if let Some(html) = find_text(formats, uti::UTI_HTML) {
        return truncate_title(&extract_text_from_html(&html));
    }
    // 兜底
    truncate_title("")
}

/// 从 file-url 格式生成标题(取文件名)。
///
/// 多文件:第一个文件名 + 「 等 N 项」。
fn title_from_file_url(formats: &[CapturedFormat]) -> String {
    let file_fmt = formats.iter().find(|f| f.uti == uti::UTI_FILE_URL);
    let Some(file_fmt) = file_fmt else {
        return truncate_title("");
    };

    let urls = String::from_utf8_lossy(&file_fmt.data);
    let url_lines: Vec<&str> = urls.lines().filter(|l| !l.is_empty()).collect();

    if url_lines.is_empty() {
        return truncate_title("");
    }

    let first_name = file_url_to_basename(url_lines[0]);
    if url_lines.len() == 1 {
        truncate_title(&first_name)
    } else {
        // 首个 + 「 等 N 项」(N 不含首个)
        let title = format!("{} 等 {} 项", first_name, url_lines.len() - 1);
        // 文件名标题不受 TITLE_MAX_CHARS 严格限制,但仍截断防超长
        truncate_title(&title)
    }
}

/// 从文件 URL/路径提取文件名(basename)。
///
/// macOS:`file:///Users/foo/report.pdf` → `report.pdf`
/// Windows:`C:\Users\foo\report.pdf` → `report.pdf`
/// 兜底:无法识别的格式返回「文件」。
fn file_url_to_basename(url: &str) -> String {
    let path = if let Some(p) = url.strip_prefix("file://") {
        // macOS file:// URL
        p
    } else if url.starts_with('/') || url.contains('\\') {
        // 直接是路径(Unix 绝对路径 或 Windows 路径含 \)
        url
    } else {
        return "文件".to_string();
    };

    // 同时处理 / 和 \ 分隔符(macOS 用 /,Windows 用 \)
    let basename = path.rsplit(['/', '\\']).next().unwrap_or("");
    if basename.is_empty() {
        "文件".to_string()
    } else {
        // 简单 URL 解码(处理 %20 等常见空格编码)
        url_decode_basic(basename)
    }
}

/// 简单 URL 解码(只处理 %XX)。完整解码需 url crate,M2 标题场景够用。
fn url_decode_basic(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 从 formats 找指定 UTI 的文本内容。
fn find_text(formats: &[CapturedFormat], uti: &str) -> Option<String> {
    formats
        .iter()
        .find(|f| f.uti == uti)
        .and_then(|f| String::from_utf8(f.data.clone()).ok())
}

/// 从 RTF 粗提取纯文本(剥离控制词和花括号)。
///
/// RTF 格式:`{\rtf1\ansi{\b粗体}普通文本}`
/// 提取:粗体普通文本(剥离 \xxx 控制词和 {} 结构)
///
/// 这是最简实现(不引 rtf crate)。M2 标题只需可读预览,精度够用。
/// 预览面板的 `get_item_text` 命令复用此函数做 rich 回退提取。
pub fn extract_text_from_rtf(rtf: &str) -> String {
    let mut out = String::with_capacity(rtf.len());
    let chars: Vec<char> = rtf.chars().collect();
    let mut i = 0;
    let mut skip_control = false;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                // 控制词 \xxx 或 转义字符 \'xx
                if i + 1 < chars.len() && chars[i + 1] == '\'' {
                    // \'xx 是 hex 字符,跳过 3 个
                    i += 3;
                    continue;
                }
                skip_control = true;
            }
            '{' | '}' => {
                skip_control = false;
            }
            ' ' if skip_control => {
                // 控制词后的空格是分隔符,跳过
                skip_control = false;
            }
            ';' | ',' | '-' | '_' | '0'..='9' | 'a'..='z' | 'A'..='Z' if skip_control => {
                // 控制词内部字符,跳过
            }
            _ if skip_control => {
                // 控制词结束(遇到非控制词字符)
                skip_control = false;
                out.push(c);
            }
            _ => {
                out.push(c);
            }
        }
        i += 1;
    }
    out.trim().to_string()
}

/// 从 HTML 粗提取纯文本(剥离标签)。
///
/// 最简实现:`<p>Hello <b>world</b></p>` → `Hello world`
/// 预览面板的 `get_item_text` 命令复用此函数做 rich 回退提取。
pub fn extract_text_from_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // 折叠连续空白
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::capture::CapturedFormat;
    use crate::models::TITLE_MAX_CHARS;

    fn fmt(uti: &str, data: &str) -> CapturedFormat {
        CapturedFormat {
            uti: uti.to_string(),
            data: data.as_bytes().to_vec(),
            file_path: None,
        }
    }

    #[test]
    fn title_image_with_size() {
        // 图片标题:「图片 W×H」
        let formats = vec![fmt("public.png", "")];
        assert_eq!(
            generate_title(&formats, ContentKind::Image, Some((1920, 1080))),
            "图片 1920×1080"
        );
    }

    #[test]
    fn title_image_without_size() {
        let formats = vec![fmt("public.png", "")];
        assert_eq!(generate_title(&formats, ContentKind::Image, None), "图片");
    }

    #[test]
    fn title_plain_text() {
        let formats = vec![fmt("public.utf8-plain-text", "Hello world")];
        assert_eq!(generate_title(&formats, ContentKind::Text, None), "Hello world");
    }

    #[test]
    fn title_plain_text_truncated() {
        let long = "a".repeat(TITLE_MAX_CHARS + 100);
        let formats = vec![fmt("public.utf8-plain-text", &long)];
        let title = generate_title(&formats, ContentKind::Text, None);
        assert_eq!(title.chars().count(), TITLE_MAX_CHARS);
    }

    #[test]
    fn title_chinese_text() {
        let formats = vec![fmt("public.utf8-plain-text", "你好世界")];
        assert_eq!(generate_title(&formats, ContentKind::Text, None), "你好世界");
    }

    #[test]
    fn title_rich_prefers_plain_text() {
        // 富文本场景:有 text 格式优先用 text(不用 rtf 源码)
        let formats = vec![
            fmt("public.utf8-plain-text", "项目进度报告"),
            fmt("public.rtf", "{\\rtf1\\ansi\\b 项目进度报告}"),
        ];
        assert_eq!(generate_title(&formats, ContentKind::Rich, None), "项目进度报告");
    }

    #[test]
    fn title_rich_extracts_from_rtf_when_no_text() {
        // 只有 RTF(无纯文本):从 RTF 提取
        let formats = vec![fmt("public.rtf", "{\\rtf1\\ansi 纯RTF内容}")];
        let title = generate_title(&formats, ContentKind::Rich, None);
        assert!(title.contains("纯RTF内容"), "标题应含提取的文本,实际: {title}");
    }

    #[test]
    fn title_html_extracts_text() {
        let formats = vec![fmt("public.html", "<p>Hello <b>world</b></p>")];
        let title = generate_title(&formats, ContentKind::Rich, None);
        assert_eq!(title, "Hello world");
    }

    #[test]
    fn title_file_single() {
        let url = "file:///Users/foo/report.pdf";
        let formats = vec![fmt("public.file-url", url)];
        assert_eq!(generate_title(&formats, ContentKind::File, None), "report.pdf");
    }

    #[test]
    fn title_file_multiple() {
        let urls = "file:///Users/a.png\nfile:///Users/b.png\nfile:///Users/c.png";
        let formats = vec![fmt("public.file-url", urls)];
        let title = generate_title(&formats, ContentKind::File, None);
        assert_eq!(title, "a.png 等 2 项");
    }

    #[test]
    fn title_file_url_with_spaces() {
        // %20 URL 编码的空格
        let url = "file:///Users/foo/my%20report.pdf";
        let formats = vec![fmt("public.file-url", url)];
        assert_eq!(generate_title(&formats, ContentKind::File, None), "my report.pdf");
    }

    #[test]
    fn title_file_url_decode_chinese() {
        // 中文文件名 URL 编码
        let url = "file:///Users/foo/%E6%8A%A5%E5%91%8A.pdf"; // 报告.pdf
        let formats = vec![fmt("public.file-url", url)];
        let title = generate_title(&formats, ContentKind::File, None);
        assert_eq!(title, "报告.pdf");
    }

    #[test]
    fn file_url_to_basename_handles_edge() {
        assert_eq!(file_url_to_basename("file:///a/b.txt"), "b.txt");
        assert_eq!(file_url_to_basename("file:///a/"), "文件");
        assert_eq!(file_url_to_basename("not-a-url"), "文件");
    }

    #[test]
    fn file_url_to_basename_windows_path() {
        // Windows 路径格式(C:\Users\...)
        assert_eq!(file_url_to_basename("C:\\Users\\foo\\report.pdf"), "report.pdf");
        assert_eq!(file_url_to_basename("D:\\photos\\截图.png"), "截图.png");
        // Windows 路径末尾
        assert_eq!(file_url_to_basename("C:\\file.txt"), "file.txt");
    }
}
