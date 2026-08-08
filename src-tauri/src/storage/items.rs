//! items 表 CRUD — 剪贴板历史条目的增删查
//!
//! M1 只处理纯文本(uti='public.utf8-plain-text')。每次复制:
//! - 去重:formats 表里已有相同文本 → UPDATE items(last_copied_at + copy_count)
//! - 新增:INSERT items + INSERT formats(一行 text)
//!
//! M2 扩展:`insert_or_update_multi` 支持一次写入多格式(RTF/HTML/图片等),
//! content_kind 分类,并写 search_text(FTS 索引用)。M1 的单文本入口保留(handler 仍在用)。
//!
//! 去重用 data_blob 内容比对(M1 数据量小,全表扫可接受;M4 加 hash 索引优化)。

// M1 阶段 A:存储层先建,阶段 B(watcher)/C(command)接线前公开 API 暂未使用。
// 阶段 B/C 接线后移除此 allow。
#![allow(dead_code)]

use rusqlite::{params, Connection};
use std::path::Path;

use crate::error::AppError;
use crate::models::{truncate_title, ClipboardItem, ContentKind};
use crate::storage::search::bigram_cjk;

/// 纯文本的 UTI。
pub const UTI_PLAIN_TEXT: &str = "public.utf8-plain-text";

/// 一个格式的写入数据(uti + 字节内容)。
///
/// M2 多格式捕获时,一次复制会产生多个 FormatInput(text/rtf/html/image...),
/// 统一用这个结构传给 `insert_or_update_multi`。
/// 图片格式的 data 为空(图片走文件系统,data 在 file_path,见 image_store)。
#[derive(Debug, Clone)]
pub struct FormatInput<'a> {
    pub uti: &'a str,
    /// 文本类格式的字节内容(RTF/HTML/text 等)。图片格式为空(走 file_path)。
    pub data: &'a [u8],
    /// 图片格式的磁盘相对路径(file_path)。文本类格式为 None。
    pub file_path: Option<&'a str>,
}

/// 插入或更新剪贴板条目(去重,纯文本入口,M1 handler 用)。
///
/// 返回最终的 ClipboardItem(新增或更新后的)。调用方据此 emit 前端。
///
/// 去重逻辑:formats 表里查找 uti=PLAIN_TEXT 且 data_blob == text 的 item。
/// - 找到:UPDATE 该 item 的 last_copied_at + copy_count
/// - 没找到:INSERT items(含 search_text)+ INSERT formats(text)
pub fn insert_or_update(conn: &Connection, text: &str) -> Result<ClipboardItem, AppError> {
    let now = now_epoch_ms();
    let title = truncate_title(text);
    let text_bytes = text.as_bytes();
    let search_text = bigram_cjk(text);

    // 先查是否已存在相同文本的条目
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT item_id FROM clipboard_formats \
             WHERE uti = ?1 AND data_blob = ?2 LIMIT 1",
            params![UTI_PLAIN_TEXT, text_bytes],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| AppError::internal(format!("查询去重失败: {e}")))?;

    let item_id = match existing_id {
        Some(id) => {
            // 已存在:更新时间 + 次数(search_text 不变,文本没变)
            conn.execute(
                "UPDATE clipboard_items SET last_copied_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| AppError::internal(format!("更新 copy_count 失败: {e}")))?;
            id
        }
        None => {
            // 不存在:INSERT items(含 search_text,触发器自动索引 FTS)
            conn.execute(
                "INSERT INTO clipboard_items \
                 (first_copied_at, last_copied_at, copy_count, title, content_kind, search_text) \
                 VALUES (?1, ?2, 1, ?3, ?4, ?5)",
                params![now, now, title, ContentKind::Text.as_db_str(), &search_text],
            )
            .map_err(|e| AppError::internal(format!("INSERT items 失败: {e}")))?;
            let new_id = conn.last_insert_rowid();

            // INSERT formats(一行纯文本)
            conn.execute(
                "INSERT INTO clipboard_formats (item_id, uti, data_blob, size) VALUES (?1, ?2, ?3, ?4)",
                params![new_id, UTI_PLAIN_TEXT, text_bytes, text_bytes.len() as i64],
            )
            .map_err(|e| AppError::internal(format!("INSERT formats 失败: {e}")))?;

            new_id
        }
    };

    get_item(conn, item_id)
}

/// 插入或更新剪贴板条目(多格式入口,M2 捕获层用)。
///
/// 与 `insert_or_update`(单文本)的区别:
/// - 支持一次写多个 formats(RTF/HTML/text/image/file-url 等)
/// - content_kind 由调用方根据捕获格式分类
/// - 标题由调用方生成(富文本提取纯文本 / 图片尺寸 / 文件名)
///
/// 去重:基于纯文本格式的 data_blob 比对(与 M1 一致)。
/// 若 formats 里没有纯文本(纯图片/纯文件),不做去重(每次都新增)。
///
/// 返回最终的 ClipboardItem。
pub fn insert_or_update_multi(
    conn: &Connection,
    formats: &[FormatInput<'_>],
    kind: ContentKind,
    title: &str,
) -> Result<ClipboardItem, AppError> {
    let now = now_epoch_ms();
    let title = truncate_title(title);

    // search_text:从纯文本格式提取 → bigram;无文本格式则空
    let plain_text = formats
        .iter()
        .find(|f| f.uti == UTI_PLAIN_TEXT)
        .map(|f| f.data);
    let search_text = match plain_text {
        Some(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            bigram_cjk(&text)
        }
        None => String::new(),
    };

    // 去重:有纯文本格式才比对(与 M1 一致)
    let existing_id: Option<i64> = match plain_text {
        Some(bytes) => conn
            .query_row(
                "SELECT item_id FROM clipboard_formats \
                 WHERE uti = ?1 AND data_blob = ?2 LIMIT 1",
                params![UTI_PLAIN_TEXT, bytes],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::internal(format!("多格式查询去重失败: {e}")))?,
        None => None,
    };

    let item_id = match existing_id {
        Some(id) => {
            conn.execute(
                "UPDATE clipboard_items SET last_copied_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| AppError::internal(format!("更新 copy_count 失败: {e}")))?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO clipboard_items \
                 (first_copied_at, last_copied_at, copy_count, title, content_kind, search_text) \
                 VALUES (?1, ?2, 1, ?3, ?4, ?5)",
                params![now, now, &title, kind.as_db_str(), &search_text],
            )
            .map_err(|e| AppError::internal(format!("INSERT items 失败: {e}")))?;
            let new_id = conn.last_insert_rowid();

            // INSERT 所有 formats 行
            for fmt in formats {
                conn.execute(
                    "INSERT INTO clipboard_formats (item_id, uti, data_blob, file_path, size) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        new_id,
                        fmt.uti,
                        fmt.data,
                        fmt.file_path,
                        fmt.data.len() as i64,
                    ],
                )
                .map_err(|e| AppError::internal(format!("INSERT formats ({}) 失败: {e}", fmt.uti)))?;
            }

            new_id
        }
    };

    get_item(conn, item_id)
}

/// 按 id 查单条(返回 ClipboardItem 元数据视图)。
pub fn get_item(conn: &Connection, id: i64) -> Result<ClipboardItem, AppError> {
    let mut item = conn
        .query_row(
            "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind \
             FROM clipboard_items WHERE id = ?1",
            params![id],
            row_to_item,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::new(crate::error::codes::NOT_FOUND, format!("条目 {} 不存在", id))
            }
            other => AppError::internal(format!("查条目 {} 失败: {}", id, other)),
        })?;
    fill_image_fields(conn, &mut item)?;
    Ok(item)
}

/// 查列表(按 last_copied_at DESC)。
pub fn list(conn: &Connection, limit: i64) -> Result<Vec<ClipboardItem>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind \
             FROM clipboard_items ORDER BY last_copied_at DESC LIMIT ?1",
        )
        .map_err(|e| AppError::internal(format!("prepare list 失败: {e}")))?;

    let rows = stmt
        .query_map(params![limit], row_to_item)
        .map_err(|e| AppError::internal(format!("query list 失败: {e}")))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| AppError::internal(format!("读 list 行失败: {e}")))?);
    }
    // 补充图片类条目的缩略图路径
    for item in &mut result {
        fill_image_fields(conn, item)?;
    }
    Ok(result)
}

/// 读某条的纯文本内容(给 copy_back 用)。
///
/// M1 从 formats 表读 uti=PLAIN_TEXT 的 data_blob。
pub fn get_text(conn: &Connection, id: i64) -> Result<String, AppError> {
    let blob: Vec<u8> = conn
        .query_row(
            "SELECT data_blob FROM clipboard_formats WHERE item_id = ?1 AND uti = ?2 LIMIT 1",
            params![id, UTI_PLAIN_TEXT],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::new(
                crate::error::codes::NOT_FOUND,
                format!("条目 {} 无纯文本格式", id),
            ),
            other => AppError::internal(format!("读文本失败: {}", other)),
        })?;

    String::from_utf8(blob)
        .map_err(|e| AppError::internal(format!("文本非 UTF-8: {}", e)))
}

/// 一个格式行的读取结果(uti + data_blob + file_path)。
pub type FormatRow = (String, Option<Vec<u8>>, Option<String>);

/// 读某条的所有格式行(给 M2 多格式回填用)。
///
/// 返回每个格式的 (uti, data_blob, file_path)。
pub fn get_formats(conn: &Connection, id: i64) -> Result<Vec<FormatRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT uti, data_blob, file_path FROM clipboard_formats WHERE item_id = ?1",
        )
        .map_err(|e| AppError::internal(format!("prepare get_formats 失败: {e}")))?;

    let rows = stmt
        .query_map(params![id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<Vec<u8>>>(1)?, r.get::<_, Option<String>>(2)?))
        })
        .map_err(|e| AppError::internal(format!("query get_formats 失败: {e}")))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| AppError::internal(format!("读 formats 行失败: {e}")))?);
    }
    Ok(result)
}

/// 删除某条(ON DELETE CASCADE 自动清 formats)。
///
/// M2 图片场景:删 DB 行前,对该 item 的图片格式做引用计数删除
/// (引用数 == 1 时删磁盘原图 + 缩略图)。`images_dir` 为图片根目录。
pub fn delete(conn: &Connection, id: i64, images_dir: &Path) -> Result<(), AppError> {
    // 删 DB 前:查该 item 的图片格式 file_path,引用计数删除磁盘文件
    let image_paths: Vec<String> = conn
        .prepare("SELECT file_path FROM clipboard_formats WHERE item_id = ?1 AND file_path IS NOT NULL")
        .map_err(|e| AppError::internal(format!("prepare 查图片 file_path 失败: {e}")))?
        .query_map(params![id], |r| r.get::<_, String>(0))
        .map_err(|e| AppError::internal(format!("查图片 file_path 失败: {e}")))?
        .filter_map(Result::ok)
        .collect();
    for fp in image_paths {
        // 引用计数删除(此时 DB 行还在,引用数含当前 item)
        crate::clipboard::image_store::delete_if_last_reference(conn, &fp, images_dir)?;
    }

    // 删 DB 行(CASCADE 清 formats)
    conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
        .map_err(|e| AppError::internal(format!("删除条目 {} 失败: {}", id, e)))?;
    Ok(())
}

// ============================================================
// 内部辅助
// ============================================================

/// 当前 unix epoch 毫秒。
fn now_epoch_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// DB 行 → ClipboardItem。
///
/// `pub(crate)` 供 search.rs 复用(FTS 查询也返回相同结构的 ClipboardItem)。
///
/// 注意:items 表行不含图片字段(thumb_path/image_size 在 formats 表)。
/// 这里填 None,由 `fill_image_fields` 在 list/get_item 后对图片类条目补充。
pub(crate) fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        first_copied_at: row.get(1)?,
        last_copied_at: row.get(2)?,
        copy_count: row.get(3)?,
        title: row.get(4)?,
        content_kind: ContentKind::from_db_str(row.get::<_, String>(5)?.as_str()),
        thumb_path: None,
        image_size: None,
    })
}

/// 给图片类 item 补充缩略图路径(从 formats 表查 file_path 推导)。
///
/// 非 image/mixed 类型不处理。图片格式(public.png/public.tiff)的 file_path 存的是
/// 原图相对路径,缩略图路径由 image_store::relative_thumb_path_from_image 推导。
fn fill_image_fields(conn: &Connection, item: &mut ClipboardItem) -> Result<(), AppError> {
    if !matches!(item.content_kind, ContentKind::Image | ContentKind::Mixed) {
        return Ok(());
    }
    // 查图片格式的 file_path
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM clipboard_formats \
             WHERE item_id = ?1 AND uti IN ('public.png','public.tiff') AND file_path IS NOT NULL \
             LIMIT 1",
            params![item.id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::internal(format!("查图片 file_path 失败: {e}")))?;
    if let Some(fp) = file_path {
        item.thumb_path = Some(crate::clipboard::image_store::relative_thumb_path_from_image_pub(&fp));
    }
    Ok(())
}

/// rusqlite 的 Optional 扩展 trait(把 QueryReturnedNoRows 转成 Option)。
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_in_memory;

    fn db() -> Connection {
        init_db_in_memory().expect("内存 DB 初始化应成功")
    }

    /// 测试用 images_dir(临时目录,纯文本测试不实际写图)。
    fn test_images_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clipboard-items-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn insert_new_item() {
        let conn = db();
        let item = insert_or_update(&conn, "第一条").expect("插入应成功");
        assert_eq!(item.title, "第一条");
        assert_eq!(item.copy_count, 1);
        assert_eq!(item.content_kind, ContentKind::Text);
    }

    #[test]
    fn duplicate_updates_count() {
        let conn = db();
        let first = insert_or_update(&conn, "重复内容").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second = insert_or_update(&conn, "重复内容").unwrap();

        assert_eq!(first.id, second.id, "相同文本应命中同一条");
        assert_eq!(second.copy_count, 2);
        assert!(second.last_copied_at >= first.last_copied_at);
    }

    #[test]
    fn different_text_creates_separate_items() {
        let conn = db();
        let a = insert_or_update(&conn, "AAA").unwrap();
        let b = insert_or_update(&conn, "BBB").unwrap();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn list_orders_by_last_copied_desc() {
        let conn = db();
        insert_or_update(&conn, "旧").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        insert_or_update(&conn, "新").unwrap();

        let list = list(&conn, 10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "新", "最新复制应排第一");
    }

    #[test]
    fn get_text_returns_stored_content() {
        let conn = db();
        let item = insert_or_update(&conn, "要回填的内容").unwrap();
        let text = get_text(&conn, item.id).unwrap();
        assert_eq!(text, "要回填的内容");
    }

    #[test]
    fn delete_removes_item_and_cascades_formats() {
        let conn = db();
        let item = insert_or_update(&conn, "待删除").unwrap();

        // 删除前 formats 有行
        let fmt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_formats WHERE item_id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fmt_count, 1);

        delete(&conn, item.id, &test_images_dir()).unwrap();

        // 删除后 items 和 formats 都没了(CASCADE)
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items WHERE id = ?1", params![item.id], |r| r.get(0))
            .unwrap();
        assert_eq!(item_count, 0);
        let fmt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_formats WHERE item_id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fmt_count, 0, "CASCADE 应清空 formats");
    }

    #[test]
    fn get_nonexistent_item_returns_not_found() {
        let conn = db();
        let err = get_item(&conn, 9999).unwrap_err();
        assert_eq!(err.code, crate::error::codes::NOT_FOUND);
    }

    // ===== M2 多格式写入测试 =====

    #[test]
    fn insert_multi_writes_all_formats() {
        // 一次复制产生多格式(text + rtf + html),全部写入 formats 表
        let conn = db();
        let text = b"Hello world".to_vec();
        let rtf = b"{\\rtf1 Hello}".to_vec();
        let html = b"<p>Hello</p>".to_vec();
        let formats = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
            FormatInput { uti: "public.rtf", data: &rtf, file_path: None },
            FormatInput { uti: "public.html", data: &html, file_path: None },
        ];
        let item = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "Hello world")
            .expect("多格式插入应成功");

        // formats 表应有 3 行
        let fmt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_formats WHERE item_id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fmt_count, 3, "应写入 3 个格式行");

        // content_kind 应为 rich
        assert_eq!(item.content_kind, ContentKind::Rich);

        // get_formats 能读回 3 个格式
        let read_back = get_formats(&conn, item.id).unwrap();
        assert_eq!(read_back.len(), 3);
        let utis: Vec<&str> = read_back.iter().map(|(u, _, _)| u.as_str()).collect();
        assert!(utis.contains(&"public.utf8-plain-text"));
        assert!(utis.contains(&"public.rtf"));
        assert!(utis.contains(&"public.html"));
    }

    #[test]
    fn insert_multi_writes_search_text_from_plain_text() {
        // search_text 从纯文本格式提取 → bigram
        let conn = db();
        let text = "你好世界".as_bytes().to_vec();
        let rtf = "{\\rtf1 你好世界}".as_bytes().to_vec();
        let formats = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
            FormatInput { uti: "public.rtf", data: &rtf, file_path: None },
        ];
        let item = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "你好世界").unwrap();

        // search_text 应为 bigram(你好世界 → " 你好 好世 世界 ")
        let st: String = conn
            .query_row(
                "SELECT search_text FROM clipboard_items WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, crate::storage::search::bigram_cjk("你好世界"));

        // FTS 触发器应已索引(搜「你好」能命中)
        let hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH ?1",
                params!["\"你好\""],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1, "多格式写入后 FTS 应能搜到");
    }

    #[test]
    fn insert_multi_dedup_by_plain_text() {
        // 去重:基于纯文本格式比对,相同文本不新增
        let conn = db();
        let text = "重复内容".as_bytes().to_vec();
        let formats = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
        ];
        let first = insert_or_update_multi(&conn, &formats, ContentKind::Text, "重复内容").unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::Text, "重复内容").unwrap();

        assert_eq!(first.id, second.id, "相同纯文本应命中同一条");
        assert_eq!(second.copy_count, 2);
    }

    #[test]
    fn insert_multi_no_plain_text_no_dedup() {
        // 纯图片(无纯文本格式)不去重,每次新增
        let conn = db();
        let formats = vec![
            FormatInput { uti: "public.png", data: &[], file_path: Some("ab/1234.png") },
        ];
        let first = insert_or_update_multi(&conn, &formats, ContentKind::Image, "图片 100×100").unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::Image, "图片 100×100").unwrap();

        assert_ne!(first.id, second.id, "无纯文本格式不去重");
    }

    #[test]
    fn single_text_insert_indexes_fts() {
        // M1 单文本入口(insert_or_update)也应写 search_text + 索引 FTS
        let conn = db();
        let item = insert_or_update(&conn, "搜索测试文本").unwrap();

        let hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH ?1",
                params!["\"搜索\""],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1, "M1 单文本入口也应索引 FTS");
        // search_text 写入了
        let st: String = conn
            .query_row(
                "SELECT search_text FROM clipboard_items WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, crate::storage::search::bigram_cjk("搜索测试文本"));
    }
}
