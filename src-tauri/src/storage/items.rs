//! items 表 CRUD — 剪贴板历史条目的增删查
//!
//! M1 只处理纯文本(uti='public.utf8-plain-text')。每次复制:
//! - 去重:formats 表里已有相同文本 → UPDATE items(last_copied_at + copy_count)
//! - 新增:INSERT items + INSERT formats(一行 text)
//!
//! 去重用 data_blob 内容比对(M1 数据量小,全表扫可接受;M4 加 hash 索引优化)。

// M1 阶段 A:存储层先建,阶段 B(watcher)/C(command)接线前公开 API 暂未使用。
// 阶段 B/C 接线后移除此 allow。
#![allow(dead_code)]

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::{truncate_title, ClipboardItem, ContentKind};

/// 纯文本的 UTI(M1 唯一处理的格式)。
pub const UTI_PLAIN_TEXT: &str = "public.utf8-plain-text";

/// 插入或更新剪贴板条目(去重)。
///
/// 返回最终的 ClipboardItem(新增或更新后的)。调用方据此 emit 前端。
///
/// 去重逻辑:formats 表里查找 uti=PLAIN_TEXT 且 data_blob == text 的 item。
/// - 找到:UPDATE 该 item 的 last_copied_at + copy_count
/// - 没找到:INSERT items + INSERT formats(text)
pub fn insert_or_update(conn: &Connection, text: &str) -> Result<ClipboardItem, AppError> {
    let now = now_epoch_ms();
    let title = truncate_title(text);
    let text_bytes = text.as_bytes();

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
            // 已存在:更新时间 + 次数
            conn.execute(
                "UPDATE clipboard_items SET last_copied_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| AppError::internal(format!("更新 copy_count 失败: {e}")))?;
            id
        }
        None => {
            // 不存在:INSERT items
            conn.execute(
                "INSERT INTO clipboard_items \
                 (first_copied_at, last_copied_at, copy_count, title, content_kind) \
                 VALUES (?1, ?2, 1, ?3, ?4)",
                params![now, now, title, ContentKind::Text.as_db_str()],
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

/// 按 id 查单条(返回 ClipboardItem 元数据视图)。
pub fn get_item(conn: &Connection, id: i64) -> Result<ClipboardItem, AppError> {
    conn.query_row(
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
    })
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

/// 删除某条(ON DELETE CASCADE 自动清 formats)。
pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
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
fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        first_copied_at: row.get(1)?,
        last_copied_at: row.get(2)?,
        copy_count: row.get(3)?,
        title: row.get(4)?,
        content_kind: ContentKind::from_db_str(
            row.get::<_, String>(5)?.as_str(),
        ),
    })
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

        delete(&conn, item.id).unwrap();

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
}
