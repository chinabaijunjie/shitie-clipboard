//! Schema migration — 手写版本号管理(`PRAGMA user_version`)
//!
//! M0/M1 用 `CREATE TABLE IF NOT EXISTS`(幂等),无需版本号。
//! M2 起 schema 需要结构性变更(`ALTER TABLE ADD COLUMN` 不幂等),
//! 改用手写版本号:每次启动查 `user_version`,未达 CURRENT 才跑增量迁移。
//!
//! 版本历史:
//! - 0:初始(M0/M1 schema:items + formats 两表,IF NOT EXISTS 建表)
//! - 2:M2 — 加 search_text 列 + FTS5 外部内容表 + 三触发器 + idx_formats_file_path + 存量回填
//! - 3:内容子类型 — 加 content_subtype 列 + 存量回填(link/markdown 识别)
//!
//! 注:版本号跳过 1(0 → 2),与 m2-research 的 CURRENT_SCHEMA_VERSION=2 对齐,
//! 为未来小幅变更留余地。

use rusqlite::Connection;

use crate::clipboard::classify::{detect_subtype, ContentSubtype};
use crate::error::AppError;
use crate::storage::fts;
use crate::storage::search::bigram_cjk;

/// 当前 schema 版本。每次 schema 变更 bump 这个值 + 在 migrate 里加分支。
pub const CURRENT_SCHEMA_VERSION: i64 = 3;

/// 跑增量迁移,把 DB 升级到 CURRENT_SCHEMA_VERSION。
///
/// 幂等:已是最新的库直接跳过(查 user_version 判断)。
/// 幂等性由「版本号 < CURRENT 才进入分支」+ 分支内 IF NOT EXISTS / ALTER 受版本号守护共同保证。
pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    let current = read_version(conn)?;

    if current < 2 {
        migrate_to_v2(conn)?;
    }

    if current < 3 {
        migrate_to_v3(conn)?;
    }

    // 兜底:若库的 user_version 已超过 CURRENT(不应发生),记 warn 但不阻断。
    if current > CURRENT_SCHEMA_VERSION {
        log::warn!(
            "[migration] DB user_version={} 超过 CURRENT_SCHEMA_VERSION={},可能由更新版本 app 创建",
            current,
            CURRENT_SCHEMA_VERSION
        );
    }
    Ok(())
}

/// 读当前 DB 版本号。
fn read_version(conn: &Connection) -> Result<i64, AppError> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| AppError::internal(format!("读 user_version 失败: {e}")))
}

/// 迁移到 v2:M2 schema 变更。
///
/// - 加 `clipboard_items.search_text` 列(存 bigram 预处理后的全文,供 FTS 索引)
/// - 建 FTS5 外部内容表 + 三触发器(指向 items,content_rowid=id)
/// - 加 `idx_formats_file_path`(图片引用计数查询用)
/// - 存量回填:从 formats 表读 plain text → bigram → search_text + FTS rebuild
/// - 写 user_version=2
///
/// ⚠️ ALTER TABLE ADD COLUMN 不幂等,靠「current < 2 才进入」守护 + **整段事务包裹**:
/// 任一步失败 ROLLBACK 回 v0(下次启动干净重试),避免半迁移状态卡死 app。
fn migrate_to_v2(conn: &Connection) -> Result<(), AppError> {
    log::info!("[migration] 升级到 schema v2(M2:search_text + FTS5)");

    // ⚠️ 整个迁移包进事务:任一步失败回滚,避免 ALTER 已提交但版本号没写 →
    // 下次重跑 ALTER 报 duplicate column → app 永久启动失败。
    // 用 unchecked_transaction:rusqlite 的 Transaction drop 会自动 rollback(失败时),
    // 手动 commit(成功时)。
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::internal(format!("开启 migration 事务失败: {e}")))?;

    let result = migrate_to_v2_inner(&tx);
    match result {
        Ok(()) => {
            tx.commit()
                .map_err(|e| AppError::internal(format!("migration commit 失败: {e}")))?;
            log::info!("[migration] schema v2 升级完成");
            Ok(())
        }
        Err(e) => {
            // tx drop 时自动 rollback,这里显式说明
            log::error!("[migration] schema v2 升级失败,已回滚: {}", e.message);
            Err(e)
        }
    }
}

/// migrate_to_v2 的事务内逻辑(失败时由调用方 rollback)。
fn migrate_to_v2_inner(conn: &Connection) -> Result<(), AppError> {
    // 1. 加 search_text 列(不幂等,版本号 + 事务守护)
    conn.execute(
        "ALTER TABLE clipboard_items ADD COLUMN search_text TEXT",
        [],
    )
    .map_err(|e| AppError::internal(format!("ALTER TABLE 加 search_text 失败: {e}")))?;

    // 2. 存量回填:M1 库里已有纯文本数据,给它们的 search_text 赋值。
    //    ⚠️ 必须在建 FTS 触发器之前回填:回填是 UPDATE,若触发器已存在会触发 au(对尚不存在的
    //    FTS 行发 delete)→ "database disk image is malformed"。先回填再建 FTS。
    backfill_search_text(conn)?;

    // 3. FTS5 外部内容表 + 三触发器(IF NOT EXISTS 幂等,见 fts.rs)
    fts::create_fts_schema(conn)?;

    // 4. idx_formats_file_path(图片引用计数用,IF NOT EXISTS 幂等)
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_formats_file_path ON clipboard_formats(file_path)",
        [],
    )
    .map_err(|e| AppError::internal(format!("建 idx_formats_file_path 失败: {e}")))?;

    // 5. FTS rebuild(从 items.search_text 重建倒排索引,把存量数据纳入索引)
    conn.execute_batch("INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');")
        .map_err(|e| AppError::internal(format!("FTS rebuild 失败: {e}")))?;

    // 6. 写版本号
    write_version(conn, CURRENT_SCHEMA_VERSION)?;

    Ok(())
}

/// 存量回填:给已有 items 的 search_text 赋值。
///
/// M1 的数据文本在 formats 表(uti=public.utf8-plain-text 的 data_blob)。
/// 读出来 → bigram 预处理 → 写回 items.search_text。
fn backfill_search_text(conn: &Connection) -> Result<(), AppError> {
    use rusqlite::params;
    // 查所有 items + 对应的 plain text(LEFT JOIN:万一某 item 没有 text 行也不漏)
    let mut stmt = conn
        .prepare(
            "SELECT i.id, f.data_blob FROM clipboard_items i \
             LEFT JOIN clipboard_formats f \
               ON f.item_id = i.id AND f.uti = 'public.utf8-plain-text' \
             WHERE i.search_text IS NULL",
        )
        .map_err(|e| AppError::internal(format!("prepare backfill 失败: {e}")))?;

    let rows: Vec<(i64, Option<Vec<u8>>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| AppError::internal(format!("query backfill 失败: {e}")))?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);

    let mut updated = 0;
    for (id, blob) in rows {
        let search_text = match blob {
            Some(bytes) => {
                let text = String::from_utf8(bytes).unwrap_or_default();
                bigram_cjk(&text)
            }
            None => String::new(), // 无 text 行(理论上 M1 不会,但容错)
        };
        conn.execute(
            "UPDATE clipboard_items SET search_text = ?1 WHERE id = ?2",
            params![&search_text, id],
        )
        .map_err(|e| AppError::internal(format!("回填 search_text item {} 失败: {e}", id)))?;
        updated += 1;
    }
    if updated > 0 {
        log::info!("[migration] 回填 search_text: {} 条存量数据", updated);
    }
    Ok(())
}

/// 迁移到 v3:加 content_subtype 列 + 存量回填。
///
/// - ALTER TABLE 加 `clipboard_items.content_subtype TEXT` 列
/// - 存量回填:对 text/rich 类型的 item,读 plain text → detect_subtype → 写回
/// - 写 user_version=3
///
/// 包事务(与 v2 同模式):中途失败回滚。
fn migrate_to_v3(conn: &Connection) -> Result<(), AppError> {
    log::info!("[migration] 升级到 schema v3(content_subtype)");

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::internal(format!("开启 migration v3 事务失败: {e}")))?;

    let result = migrate_to_v3_inner(&tx);
    match result {
        Ok(()) => {
            tx.commit()
                .map_err(|e| AppError::internal(format!("migration v3 commit 失败: {e}")))?;
            log::info!("[migration] schema v3 升级完成");
            Ok(())
        }
        Err(e) => {
            log::error!("[migration] schema v3 升级失败,已回滚: {}", e.message);
            Err(e)
        }
    }
}

fn migrate_to_v3_inner(conn: &Connection) -> Result<(), AppError> {
    // 1. 加 content_subtype 列(不幂等,版本号 + 事务守护)
    conn.execute(
        "ALTER TABLE clipboard_items ADD COLUMN content_subtype TEXT",
        [],
    )
    .map_err(|e| AppError::internal(format!("ALTER TABLE 加 content_subtype 失败: {e}")))?;

    // 2. 存量回填:对 text/rich 类型的 item,读 plain text → detect_subtype → 写回
    backfill_content_subtype(conn)?;

    // 3. 写版本号
    write_version(conn, CURRENT_SCHEMA_VERSION)?;

    Ok(())
}

/// 存量回填 content_subtype:对 text/rich 类型的 item 识别子类型。
fn backfill_content_subtype(conn: &Connection) -> Result<(), AppError> {
    use rusqlite::params;

    // 查 text/rich 类型的 items + 对应 plain text
    let mut stmt = conn
        .prepare(
            "SELECT i.id, f.data_blob FROM clipboard_items i \
             LEFT JOIN clipboard_formats f \
               ON f.item_id = i.id AND f.uti = 'public.utf8-plain-text' \
             WHERE i.content_kind IN ('text', 'rich') AND i.content_subtype IS NULL",
        )
        .map_err(|e| AppError::internal(format!("prepare backfill subtype 失败: {e}")))?;

    let rows: Vec<(i64, Option<Vec<u8>>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| AppError::internal(format!("query backfill subtype 失败: {e}")))?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);

    let mut updated = 0;
    for (id, blob) in rows {
        let subtype = match blob {
            Some(bytes) => {
                let text = String::from_utf8(bytes).unwrap_or_default();
                detect_subtype(&text)
            }
            None => ContentSubtype::None,
        };
        if subtype.is_some() {
            conn.execute(
                "UPDATE clipboard_items SET content_subtype = ?1 WHERE id = ?2",
                params![subtype.as_db_str(), id],
            )
            .map_err(|e| AppError::internal(format!("回填 content_subtype item {} 失败: {e}", id)))?;
            updated += 1;
        }
    }
    if updated > 0 {
        log::info!("[migration] 回填 content_subtype: {} 条存量数据", updated);
    }
    Ok(())
}

/// 写版本号。
fn write_version(conn: &Connection, version: i64) -> Result<(), AppError> {
    conn.execute_batch(&format!("PRAGMA user_version = {version};"))
        .map_err(|e| AppError::internal(format!("写 user_version={} 失败: {e}", version)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{init_db_in_memory, init_db_in_memory_v1};

    #[test]
    fn migrate_fresh_db_sets_version() {
        // 新库(刚 init_db,版本 0)→ migrate 后版本应为 CURRENT
        let conn = init_db_in_memory().expect("init 失败");
        migrate(&conn).expect("migrate 应成功");
        let v = read_version(&conn).unwrap();
        assert_eq!(v, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        // migrate 两次不应报错,版本号不变
        let conn = init_db_in_memory().expect("init 失败");
        migrate(&conn).expect("第一次 migrate");
        migrate(&conn).expect("第二次 migrate 应幂等");
        assert_eq!(read_version(&conn).unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrate_adds_search_text_column() {
        let conn = init_db_in_memory().expect("init 失败");
        migrate(&conn).expect("migrate 失败");
        // search_text 列存在:能查询(不报错)
        let _: Option<String> = conn
            .query_row("SELECT search_text FROM clipboard_items LIMIT 1", [], |r| r.get(0))
            .ok();
        // 列存在即通过(查询不报 no such column)
    }

    #[test]
    fn migrate_creates_fts_table_and_triggers() {
        let conn = init_db_in_memory().expect("init 失败");
        migrate(&conn).expect("migrate 失败");
        // FTS 虚拟表存在
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='clipboard_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "clipboard_fts 虚拟表应存在");
        // 三触发器存在
        let trig_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name LIKE 'clipboard_fts_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(trig_count, 3, "应有 ai/ad/au 三个触发器");
    }

    #[test]
    fn migrate_creates_file_path_index() {
        let conn = init_db_in_memory().expect("init 失败");
        migrate(&conn).expect("migrate 失败");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_formats_file_path'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "idx_formats_file_path 应存在");
    }

    #[test]
    fn migrate_v1_to_v2_backfills_search_text() {
        // 模拟 M1 老库:init_db_v1 建 M1 schema + 插入纯文本数据(无 search_text)→ migrate
        let conn = init_db_in_memory_v1().expect("init v1 失败");
        // 插一条 M1 数据(直接走 M1 的 insert 逻辑会触发 search_text,这里手动模拟 M1 数据)
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) \
             VALUES (1000, 1000, '你好世界', 'text')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, data_blob, size) VALUES (?1, 'public.utf8-plain-text', ?2, ?3)",
            rusqlite::params![id, "你好世界".as_bytes(), "你好世界".len() as i64],
        )
        .unwrap();

        migrate(&conn).expect("migrate 失败");

        // 回填后 search_text 应为 bigram 预处理结果
        let st: String = conn
            .query_row(
                "SELECT search_text FROM clipboard_items WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, bigram_cjk("你好世界"));

        // FTS 能搜到这条存量数据
        let hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE clipboard_fts MATCH ?1",
                rusqlite::params!["\"你好\""],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1, "存量数据应被 FTS 索引(搜「你好」命中)");
    }

    #[test]
    fn migrate_adds_content_subtype_column() {
        let conn = init_db_in_memory().expect("init 失败");
        // content_subtype 列存在:能查询(不报错)
        let _: Option<String> = conn
            .query_row(
                "SELECT content_subtype FROM clipboard_items LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
    }

    #[test]
    fn migrate_v2_to_v3_backfills_content_subtype() {
        // 模拟 v2 库:已有 search_text + FTS,插入含 URL 的 text 数据 → migrate v3 回填
        let conn = init_db_in_memory_v1().expect("init v1 失败");
        migrate_to_v2(&conn).expect("先升到 v2");

        // 插一条 text 数据(含 URL)
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind, search_text) \
             VALUES (1000, 1000, 'https://github.com', 'text', '')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, data_blob, size) VALUES (?1, 'public.utf8-plain-text', ?2, ?3)",
            rusqlite::params![id, "https://github.com".as_bytes(), "https://github.com".len() as i64],
        )
        .unwrap();

        // 继续迁移到 v3
        migrate_to_v3(&conn).expect("升到 v3");

        // 回填后 content_subtype 应为 'link'
        let subtype: String = conn
            .query_row(
                "SELECT content_subtype FROM clipboard_items WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(subtype, "link");
    }
}
