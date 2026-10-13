//! 搜索查询 — bigram 预处理 + FTS MATCH + LIKE 兜底
//!
//! 技术依据见 m2-research.md §一(spike 已验证)。
//!
//! 核心:中文(CJK)在 unicode61 默认分词下会黏成一坨(整段一个 token),
//! 任何子串都搜不到。解决办法是应用层 bigram 预处理:
//! - 写入时把 CJK 连续段切成 2 字重叠滑窗("你好世界"→"你好 好世 世界")
//! - 查询时用同一个函数处理用户输入
//! - 单字查询走 LIKE 兜底(bigram 对单字召回不可靠)
//!
//! ⚠️ 头号坑:bigram 必须 ingest + query 双向用同一个 `bigram_cjk` 函数,
//!    只切一边会导致全部查不到。

use rusqlite::{params, Connection};
use std::path::Path;

use crate::error::AppError;
use crate::models::ClipboardItem;
use crate::storage::items::row_to_item;

/// CJK 统一汉字基本区范围(U+4E00 ~ U+9FFF)。
const CJK_START: char = '\u{4E00}';
const CJK_END: char = '\u{9FFF}';

/// 把文本里的 CJK 连续段切成 bigram(2 字重叠滑窗),英文/数字/ASCII 不动。
///
/// ingest + query 双向用同一个函数。
/// - "你好世界" → " 你好 好世 世界 "
/// - 单字 CJK 原样保留(走 LIKE 兜底)
/// - "hello 世界" → "hello  世界 "
/// - "fn main() {}" → 不变(纯 ASCII)
pub fn bigram_cjk(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut cjk_run: Vec<char> = Vec::new();
    for ch in text.chars() {
        if is_cjk(ch) {
            cjk_run.push(ch);
        } else {
            flush_cjk_run(&cjk_run, &mut out);
            cjk_run.clear();
            out.push(ch);
        }
    }
    flush_cjk_run(&cjk_run, &mut out);
    out
}

/// 判断字符是否为 CJK 统一汉字。
fn is_cjk(ch: char) -> bool {
    (CJK_START..=CJK_END).contains(&ch)
}

/// 把一个 CJK 连续段刷进输出(bigram 切分)。
fn flush_cjk_run(run: &[char], out: &mut String) {
    if run.is_empty() {
        return;
    }
    if run.len() < 2 {
        // 单字 CJK 原样保留(单字走 LIKE 兜底,FTS 单 token 召回不可靠)
        out.extend(run);
        return;
    }
    out.push(' ');
    for w in run.windows(2) {
        out.extend(w);
        out.push(' ');
    }
}

/// 判断查询是否为单字符(需走 LIKE 兜底而非 FTS)。
///
/// 单字符可能是 CJK 单字(如「你」)或单个 ASCII 字符。
/// bigram 对单字召回不可靠(CJK 单字保留为单 token,unicode61 可能不索引极短 token)。
pub fn is_single_char_query(query: &str) -> bool {
    let count = query.chars().count();
    count == 1
}

/// 用户输入 → FTS5 MATCH 表达式。
///
/// bigram 预处理后,每个 token 包成 FTS5 短语(双引号),多 token 之间 OR。
/// - "你好" → `"你好"`
/// - "你好世" → `"你好" OR "好世"`
/// - "rust async" → `"rust" OR "async"`(英文 unicode61 默认已切)
///
/// ⚠️ token 内的双引号需转义为 `""`(FTS5 短语定界符),否则 `a"b` 会拼成非法 MATCH
/// 表达式触发语法错误。见 code-reviewer 置信 81 发现。
///
/// 返回 None 表示输入经预处理后无有效 token(空查询)。
pub fn to_fts_query(user_input: &str) -> Option<String> {
    let bi = bigram_cjk(user_input);
    let parts: Vec<&str> = bi.split_whitespace().filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|t| format!("\"{}\"", escape_fts_phrase(t)))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

/// 转义 FTS5 短语内的双引号:`"` → `""`(FTS5 短语定界符转义规则)。
fn escape_fts_phrase(token: &str) -> String {
    token.replace('"', "\"\"")
}

/// 搜索剪贴板历史。
///
/// 查询策略:
/// - 空查询 → 返回空列表(调用方可用 list 替代)
/// - 纯 ASCII 字母数字 → 三路 LIKE(search_text / pinyin_full / pinyin_initials)
/// - 单字查询 → `LIKE '%字%'` 兜底(bigram 对单字不可靠)
/// - 多字查询 → bigram 预处理 → FTS MATCH + BM25 排序
///
/// `offset`:分页偏移(与 limit 组成页窗口;排序尾带 id DESC 保证翻页稳定)。
/// 返回的 ClipboardItem 不含 search_text(那是内部索引列,不暴露给前端)。
pub fn search(
    conn: &Connection,
    query: &str,
    limit: i64,
    offset: i64,
    images_dir: &Path,
) -> Result<Vec<ClipboardItem>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = if is_ascii_alnum_query(trimmed) {
        // 拼音直搜:全量合并后内存分页(FTS BM25 在前 + 三路 LIKE 去重追加)
        search_by_pinyin_all(conn, trimmed)?
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect::<Vec<_>>()
    } else if is_single_char_query(trimmed) {
        search_by_like(conn, trimmed, limit, offset)?
    } else {
        search_by_fts(conn, trimmed, limit, offset)?
    };
    // 补充图片缩略图 + 文件 URL(与 list/get_item 保持一致)
    for item in &mut results {
        crate::storage::items::fill_image_fields(conn, item, images_dir)?;
        crate::storage::items::fill_file_url(conn, item)?;
    }
    Ok(results)
}

/// 搜索命中总数(分页 total 用,与 search 的分支口径一致;空查询 = 0)。
pub fn count(conn: &Connection, query: &str) -> Result<i64, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    if is_ascii_alnum_query(trimmed) {
        // 与 search 同源:全量合并后取长度(total = FTS + LIKE 去重并集)
        let total = search_by_pinyin_all(conn, trimmed)?.len() as i64;
        Ok(total)
    } else if is_single_char_query(trimmed) {
        let pattern = format!("%{trimmed}%");
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE search_text LIKE ?1",
            params![&pattern],
            |r| r.get(0),
        )
        .map_err(|e| AppError::internal(format!("like count 查询失败: {e}")))
    } else {
        let fts_query = to_fts_query(trimmed).ok_or_else(|| {
            AppError::internal(format!("查询无法转成 FTS 表达式: {trimmed:?}"))
        })?;
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH ?1",
            params![&fts_query],
            |r| r.get(0),
        )
        .map_err(|e| AppError::internal(format!("fts count 查询失败: {e}")))
    }
}

/// 查询是否为纯 ASCII 字母数字(拼音直搜路由判据)。
///
/// 这类查询走「三路 LIKE」:search_text(英文内容)OR pinyin_full(全拼)OR
/// pinyin_initials(首母)——英文内容搜索行为保持,叠加拼音命中。
pub fn is_ascii_alnum_query(query: &str) -> bool {
    !query.is_empty() && query.chars().all(|c| c.is_ascii_alphanumeric())
}

/// 拼音直搜(纯 ASCII 查询):返回**全部**命中(调用方负责分页切片)。
///
/// 合并策略(Rust 侧两段,不用 SQL UNION):
/// 1. FTS 命中在前——保留 BM25 相关性排序,英文搜索质量不回退
///    (bm25() 不能出现在 LEFT JOIN 非匹配行的排序里,单 SQL 方案实测报 no such cursor)
/// 2. 三路 LIKE 命中(search_text / pinyin_full / pinyin_initials)按 id 去重后追加,
///    置顶优先、最近复制靠前
///
/// 当前量级(≤1000 行)全量取回为毫秒级;条数级增长后再考虑下推 SQL。
fn search_by_pinyin_all(conn: &Connection, query: &str) -> Result<Vec<ClipboardItem>, AppError> {
    let pattern = format!("%{}%", query.to_lowercase());

    // 1. FTS 命中(全量,BM25 排序)
    let mut merged = search_by_fts(conn, query, i64::MAX, 0)?;
    let mut seen: std::collections::HashSet<i64> = merged.iter().map(|i| i.id).collect();

    // 2. 三路 LIKE 命中(去重后追加)
    let mut stmt = conn
        .prepare(
            "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype, pinned, pinned_at \
             FROM clipboard_items \
             WHERE (search_text LIKE ?1 OR pinyin_full LIKE ?1 OR pinyin_initials LIKE ?1) \
             ORDER BY pinned DESC, pinned_at DESC, last_copied_at DESC, id DESC",
        )
        .map_err(|e| AppError::internal(format!("prepare pinyin search 失败: {e}")))?;

    let rows = stmt
        .query_map(params![&pattern], row_to_item)
        .map_err(|e| AppError::internal(format!("pinyin search 查询失败: {e}")))?;
    for row in rows {
        let item = row.map_err(|e| AppError::internal(format!("读 pinyin 命中行失败: {e}")))?;
        if !seen.contains(&item.id) {
            seen.insert(item.id);
            merged.push(item);
        }
    }
    Ok(merged)
}

/// FTS + BM25 搜索(多字查询)。
fn search_by_fts(conn: &Connection, query: &str, limit: i64, offset: i64) -> Result<Vec<ClipboardItem>, AppError> {
    let fts_query = to_fts_query(query).ok_or_else(|| {
        AppError::internal(format!("查询无法转成 FTS 表达式: {query:?}"))
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.first_copied_at, i.last_copied_at, i.copy_count, i.title, i.content_kind, i.content_subtype, i.pinned, i.pinned_at \
             FROM clipboard_fts \
             JOIN clipboard_items i ON i.id = clipboard_fts.rowid \
             WHERE clipboard_fts MATCH ?1 \
             ORDER BY i.pinned DESC, i.pinned_at DESC, bm25(clipboard_fts), i.last_copied_at DESC, i.id DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| AppError::internal(format!("prepare fts search 失败: {e}")))?;

    let rows = stmt
        .query_map(params![&fts_query, limit, offset], row_to_item)
        .map_err(|e| AppError::internal(format!("fts search 查询失败: {e}")))?;

    collect_items(rows)
}

/// LIKE 兜底搜索(单字查询)。
fn search_by_like(conn: &Connection, ch: &str, limit: i64, offset: i64) -> Result<Vec<ClipboardItem>, AppError> {
    let pattern = format!("%{ch}%");
    let mut stmt = conn
        .prepare(
            "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype, pinned, pinned_at \
             FROM clipboard_items \
             WHERE search_text LIKE ?1 \
             ORDER BY pinned DESC, pinned_at DESC, last_copied_at DESC, id DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| AppError::internal(format!("prepare like search 失败: {e}")))?;

    let rows = stmt
        .query_map(params![&pattern, limit, offset], row_to_item)
        .map_err(|e| AppError::internal(format!("like search 查询失败: {e}")))?;

    collect_items(rows)
}

/// 收集 query_map 结果(已映射为 ClipboardItem 的迭代器)成 Vec。
fn collect_items(
    rows: impl Iterator<Item = rusqlite::Result<ClipboardItem>>,
) -> Result<Vec<ClipboardItem>, AppError> {
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| AppError::internal(format!("读搜索结果行失败: {e}")))?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_in_memory;
    use crate::storage::items;

    /// 测试用 images_dir(临时目录,搜索测试不实际写图,但 fill_image_fields 签名要求传)。
    fn test_images_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clipboard-search-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 准备带 search_text + FTS 的测试 DB,并插入若干条目。
    fn setup_with_data() -> Connection {
        let conn = init_db_in_memory().expect("init 失败");

        // 直接插 items(带 search_text)+ 触发器自动索引 FTS
        let insert = |conn: &Connection, text: &str, ts: i64| {
            let st = bigram_cjk(text);
            conn.execute(
                "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, search_text) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![ts, ts, text, &st],
            )
            .unwrap();
        };
        insert(&conn, "你好世界", 1000);
        insert(&conn, "今天天气不错", 2000);
        insert(&conn, "hello world", 3000);
        insert(&conn, "rust async await", 4000);
        insert(&conn, "https://github.com/tauri", 5000);
        insert(&conn, "function add(a, b) {}", 6000);
        conn
    }

    #[test]
    fn bigram_cjk_pure_chinese() {
        assert_eq!(bigram_cjk("你好世界"), " 你好 好世 世界 ");
    }

    #[test]
    fn bigram_cjk_single_char_kept() {
        assert_eq!(bigram_cjk("你"), "你");
    }

    #[test]
    fn bigram_cjk_mixed() {
        assert_eq!(bigram_cjk("hello 世界"), "hello  世界 ");
    }

    #[test]
    fn bigram_cjk_pure_ascii_unchanged() {
        assert_eq!(bigram_cjk("fn main() {}"), "fn main() {}");
    }

    #[test]
    fn to_fts_query_chinese_bigram() {
        assert_eq!(to_fts_query("你好世").unwrap(), "\"你好\" OR \"好世\"");
    }

    #[test]
    fn to_fts_query_english() {
        assert_eq!(to_fts_query("rust async").unwrap(), "\"rust\" OR \"async\"");
    }

    #[test]
    fn to_fts_query_empty_returns_none() {
        assert!(to_fts_query("").is_none());
        assert!(to_fts_query("   ").is_none());
    }

    #[test]
    fn to_fts_query_escapes_double_quote() {
        // 含双引号的 token 需转义为 "",否则拼成非法 MATCH 表达式触发 FTS5 语法错误
        assert_eq!(to_fts_query("a\"b").unwrap(), "\"a\"\"b\"");
    }

    #[test]
    fn is_single_char_query_detects() {
        assert!(is_single_char_query("你"));
        assert!(is_single_char_query("a"));
        assert!(!is_single_char_query("你好"));
        assert!(!is_single_char_query(""));
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let conn = setup_with_data();
        let r = search(&conn, "", 50, 0, &test_images_dir()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn search_chinese_bigram_recall() {
        let conn = setup_with_data();
        // 搜「你好」→ 命中「你好世界」
        let r = search(&conn, "你好", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "你好世界");
    }

    #[test]
    fn search_chinese_partial_recall() {
        let conn = setup_with_data();
        // 搜「你好世」→ MATCH "你好" OR "好世" → 命中「你好世界」
        let r = search(&conn, "你好世", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "你好世界");
    }

    #[test]
    fn search_english_token() {
        let conn = setup_with_data();
        let r = search(&conn, "async", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "rust async await");
    }

    #[test]
    fn search_code_url() {
        let conn = setup_with_data();
        let r = search(&conn, "github", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].title.contains("github"));
    }

    #[test]
    fn search_single_char_like_fallback() {
        let conn = setup_with_data();
        // 搜「你」(单字)→ LIKE 兜底,命中含「你」的
        let r = search(&conn, "你", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "你好世界");
    }

    #[test]
    fn search_bm25_ranking() {
        // 词频高的排前
        let conn = init_db_in_memory().expect("init 失败");
        crate::storage::migration::migrate(&conn).expect("migrate 失败");
        let st = bigram_cjk("rust rust rust");
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, search_text) VALUES (1,1,'rust x3', ?1)",
            params![&st],
        )
        .unwrap();
        let st = bigram_cjk("rust once");
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, search_text) VALUES (2,2,'rust x1', ?1)",
            params![&st],
        )
        .unwrap();

        let r = search(&conn, "rust", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r[0].title, "rust x3", "词频高的应排前(BM25)");
        assert_eq!(r[1].title, "rust x1");
    }

    // ============================================================
    // 拼音直搜(纯 ASCII 查询三路 LIKE + FTS 合并)
    // ============================================================

    /// 构造带拼音列的数据:中文条目(可以)+ 英文条目(hello)
    fn setup_pinyin_data() -> Connection {
        let conn = init_db_in_memory().expect("init 失败");
        let dir = test_images_dir();
        // 走正式 insert 路径(拼音列由 compute_pinyin 计算)
        items::insert_or_update(&conn, "可以复制", &dir).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        items::insert_or_update(&conn, "hello world", &dir).unwrap();
        conn
    }

    #[test]
    fn pinyin_search_full_pinyin() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "keyi", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1, "全拼 keyi 应命中「可以复制」");
        assert_eq!(r[0].title, "可以复制");
    }

    #[test]
    fn pinyin_search_initials() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "ky", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1, "首母 ky 应命中「可以复制」");
        assert_eq!(r[0].title, "可以复制");
    }

    #[test]
    fn pinyin_search_case_insensitive() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "KEYI", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1, "LIKE 大小写不敏感,KEYI 应命中");
    }

    #[test]
    fn pinyin_search_english_content_not_regressed() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "hello", 50, 0, &test_images_dir()).unwrap();
        assert_eq!(r.len(), 1, "英文内容搜索不回归");
        assert_eq!(r[0].title, "hello world");
    }

    #[test]
    fn pinyin_search_count_matches_results() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "keyi", 50, 0, &test_images_dir()).unwrap();
        let total = count(&conn, "keyi").unwrap();
        assert_eq!(total, 1);
        assert_eq!(r.len() as i64, total);
    }

    #[test]
    fn pinyin_search_no_match_empty() {
        let conn = setup_pinyin_data();
        let r = search(&conn, "zzzz", 50, 0, &test_images_dir()).unwrap();
        assert!(r.is_empty());
        assert_eq!(count(&conn, "zzzz").unwrap(), 0);
    }
}
