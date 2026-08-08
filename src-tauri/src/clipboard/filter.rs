//! 敏感信息过滤 — 内容级检测(与 uti.rs 的 UTI 级过滤正交)
//!
//! M4 新增。基于正则模式匹配已知敏感信息格式(API key/token/私钥),
//! 命中则跳过入库。
//!
//! 设计原则:
//! - 只做**固定前缀**模式匹配(AKIA/ghp_/-----BEGIN…),几乎零误报
//! - 不做高熵检测(Shannon entropy 误报率不可接受,业界已抛弃)
//! - 不做应用级过滤(那是 uti.rs 的 PRIVACY_SKIP_UTIS 职责)
//!
//! 模式来源:gitleaks 默认规则(https://github.com/gitleaks/gitleaks/blob/master/config/gitleaks.toml)
//! 精选高置信度子集(固定前缀 + 确定长度)。

use std::sync::OnceLock;

use regex::Regex;

/// 一个敏感信息检测模式(正则 + 描述)。
struct SensitivePattern {
    regex: Regex,
    /// 模式描述(日志/调试用,未来可暴露给用户看哪些被拦)。
    #[allow(dead_code)]
    description: &'static str,
}

/// 内置敏感信息模式(高置信度,固定前缀)。
///
/// 来源:gitleaks 精选。只选前缀固定 + 几乎零误报的模式。
fn sensitive_patterns() -> &'static Vec<SensitivePattern> {
    static PATTERNS: OnceLock<Vec<SensitivePattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // AWS
            SensitivePattern {
                regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
                description: "AWS Access Key ID",
            },
            SensitivePattern {
                regex: Regex::new(r"ASIA[0-9A-Z]{16}").unwrap(),
                description: "AWS STS Token",
            },
            // GitHub
            SensitivePattern {
                regex: Regex::new(r"gh[pousr]_[0-9a-zA-Z]{36}").unwrap(),
                description: "GitHub Personal Access Token",
            },
            SensitivePattern {
                regex: Regex::new(r"github_pat_[0-9a-zA-Z_]{82}").unwrap(),
                description: "GitHub Fine-grained PAT",
            },
            // GitLab
            SensitivePattern {
                regex: Regex::new(r"glpat-[0-9a-zA-Z\-]{20}").unwrap(),
                description: "GitLab Personal Access Token",
            },
            // Slack
            SensitivePattern {
                regex: Regex::new(r"xox[baprs]-[0-9a-zA-Z\-]{10,48}").unwrap(),
                description: "Slack Token",
            },
            // Stripe
            SensitivePattern {
                regex: Regex::new(r"sk_live_[0-9a-zA-Z]{24}").unwrap(),
                description: "Stripe Secret Key",
            },
            // Google
            SensitivePattern {
                regex: Regex::new(r"AIza[0-9A-Za-z\-_]{35}").unwrap(),
                description: "Google API Key",
            },
            // 私钥(PEM 格式,多行文本中只需匹配头部)
            SensitivePattern {
                regex: Regex::new(r"-----BEGIN (?:RSA |EC |OPENSSH |PGP |DSA )?PRIVATE KEY-----").unwrap(),
                description: "Private Key (PEM)",
            },
        ]
    })
}

/// 检测文本是否包含已知敏感信息模式。
///
/// 纯函数(正则模式内部用 OnceLock 懒初始化,线程安全)。
/// 命中任一模式返回 true,全部不命中返回 false。
pub fn is_sensitive(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    sensitive_patterns()
        .iter()
        .any(|p| p.regex.is_match(text))
}

/// 从捕获的格式列表中提取纯文本内容。
///
/// 找 `public.utf8-plain-text` 格式的 data,转成 String。
/// 无纯文本格式返回 None(图片/文件等不参与敏感检测)。
pub fn extract_plain_text(
    formats: &[crate::clipboard::capture::CapturedFormat],
) -> Option<String> {
    use crate::storage::items::UTI_PLAIN_TEXT;

    formats
        .iter()
        .find(|f| f.uti == UTI_PLAIN_TEXT)
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== 正例:应被识别为敏感 =====

    #[test]
    fn detects_github_pat() {
        let text = "ghp_1234567890abcdefghijklmnopqrstuvwxyz1234";
        assert!(is_sensitive(text), "GitHub PAT 应被识别");
    }

    #[test]
    fn detects_github_pat_in_context() {
        let text = "配置文件里写了 token=ghp_1234567890abcdefghijklmnopqrstuvwxyz1234 注意保密";
        assert!(is_sensitive(text), "长文本中间的 GitHub PAT 应被识别");
    }

    #[test]
    fn detects_aws_access_key() {
        let text = "AKIAIOSFODNN7EXAMPLE";
        assert!(is_sensitive(text), "AWS Access Key 应被识别");
    }

    #[test]
    fn detects_aws_sts_token() {
        // ASIA + 16 chars = 20 total
        let text = "ASIATHJKAFOT3XAMPLEX";
        assert!(is_sensitive(text), "AWS STS Token 应被识别");
    }

    #[test]
    fn detects_gitlab_pat() {
        let text = "glpat-1234567890abcdefghij";
        assert!(is_sensitive(text), "GitLab PAT 应被识别");
    }

    #[test]
    fn detects_slack_token() {
        let text = "xoxb-1234567890123-1234567890123-abcdefghij";
        assert!(is_sensitive(text), "Slack Token 应被识别");
    }

    #[test]
    fn detects_stripe_key() {
        let text = "sk_live_1234567890abcdefghijklmn";
        assert!(is_sensitive(text), "Stripe Secret Key 应被识别");
    }

    #[test]
    fn detects_google_api_key() {
        let text = "AIzaSyA1234567890abcdefghijklmnopqrstuv";
        assert!(is_sensitive(text), "Google API Key 应被识别");
    }

    #[test]
    fn detects_rsa_private_key() {
        let text = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...";
        assert!(is_sensitive(text), "RSA 私钥应被识别");
    }

    #[test]
    fn detects_ec_private_key() {
        let text = "-----BEGIN EC PRIVATE KEY-----\nMHQCAQEE...";
        assert!(is_sensitive(text), "EC 私钥应被识别");
    }

    #[test]
    fn detects_openssh_private_key() {
        let text = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNz...";
        assert!(is_sensitive(text), "OpenSSH 私钥应被识别");
    }

    #[test]
    fn detects_generic_private_key() {
        let text = "-----BEGIN PRIVATE KEY-----\nMIIBVwIBADAN...";
        assert!(is_sensitive(text), "通用私钥应被识别");
    }

    #[test]
    fn detects_github_fine_grained_pat() {
        // github_pat_ + 82 chars
        let text = "github_pat_1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890abcdefghij";
        assert!(is_sensitive(text), "GitHub Fine-grained PAT 应被识别");
    }

    // ===== 反例:不应被识别为敏感 =====

    #[test]
    fn normal_text_not_sensitive() {
        assert!(!is_sensitive("普通文本内容"));
    }

    #[test]
    fn empty_string_not_sensitive() {
        assert!(!is_sensitive(""));
    }

    #[test]
    fn short_string_not_sensitive() {
        assert!(!is_sensitive("AK"));
    }

    #[test]
    fn partial_prefix_not_sensitive() {
        // 前缀对但长度不够
        assert!(!is_sensitive("ghp_short"));
        assert!(!is_sensitive("AKIA_SHORT"));
    }

    #[test]
    fn word_token_not_sensitive() {
        // 含 "token" 但非 token 格式
        assert!(!is_sensitive("这是一个 token 变量"));
    }

    #[test]
    fn uuid_not_sensitive() {
        // UUID 不应被误报(不做高熵检测的好处)
        assert!(!is_sensitive("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn normal_code_not_sensitive() {
        let code = r#"
            const config = {
                apiUrl: "https://api.example.com",
                timeout: 5000,
            };
        "#;
        assert!(!is_sensitive(code), "普通代码不应被误报");
    }

    // ===== extract_plain_text 测试 =====

    fn fmt(uti: &str, data: &[u8]) -> crate::clipboard::capture::CapturedFormat {
        crate::clipboard::capture::CapturedFormat {
            uti: uti.to_string(),
            data: data.to_vec(),
            file_path: None,
        }
    }

    #[test]
    fn extract_plain_text_from_mixed_formats() {
        // 含纯文本格式 → 返回 Some(正确字符串)
        let formats = vec![
            fmt("public.rtf", b"{\\rtf1 hello}"),
            fmt("public.utf8-plain-text", b"hello world"),
            fmt("public.html", b"<p>hello</p>"),
        ];
        let result = extract_plain_text(&formats);
        assert_eq!(result.as_deref(), Some("hello world"));
    }

    #[test]
    fn extract_plain_text_none_when_no_text() {
        // 无纯文本格式(只有 RTF/图片)→ None
        let formats = vec![
            fmt("public.rtf", b"{\\rtf1 hello}"),
            fmt("public.html", b"<p>hello</p>"),
        ];
        assert!(extract_plain_text(&formats).is_none());
    }

    #[test]
    fn extract_plain_text_from_single_text() {
        // 单一纯文本格式 → Some(中文)
        let formats = vec![fmt("public.utf8-plain-text", "中文测试".as_bytes())];
        let result = extract_plain_text(&formats);
        assert_eq!(result.as_deref(), Some("中文测试"));
    }

    #[test]
    fn extract_plain_text_empty_formats() {
        // 空格式列表 → None
        let formats: Vec<crate::clipboard::capture::CapturedFormat> = vec![];
        assert!(extract_plain_text(&formats).is_none());
    }
}
