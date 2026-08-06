//! 存储层 — rusqlite 数据库初始化 + items/formats 表 + migration + FTS 搜索
//!
//! M0/M1:migration 用手写 `CREATE TABLE IF NOT EXISTS`(幂等),无版本号。
//! M2 起 schema 需结构性变更(`ALTER TABLE ADD COLUMN` 不幂等),改用手写版本号
//! (`PRAGMA user_version`),见 migration.rs。
//!
//! 表结构照搬 Maccy/Ditto 的「items(元数据)+ formats(type+value,一对多)」设计
//! (m1-research.md 第三节)。M1 只写 text 一行,M2 富文本/图片加行。
//!
//! ⚠️ PRAGMA 红线(constraints.md):foreign_keys=ON + WAL + busy_timeout=5000。

// 阶段 B/C 接线后移除此 allow。
#![allow(dead_code)]

pub mod fts;
pub mod items;
pub mod migration;
pub mod search;

use rusqlite::Connection;
use std::path::Path;

use crate::error::AppError;

/// 初始化数据库:打开连接 → 开 PRAGMA → 建表 → 跑 migration。
///
/// `db_path` 为应用数据目录下的 db 文件路径(由 lib.rs 传入 `app_data_dir()/clipboard.db`)。
/// 测试时可传 `:memory:` 路径(但 rusqlite 用 `:memory:` 要走 `Connection::open_in_memory`)。
pub fn init_db(db_path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(db_path)
        .map_err(|e| AppError::internal(format!("打开 DB 失败 {}: {}", db_path.display(), e)))?;

    apply_pragmas(&conn)?;
    create_schema(&conn)?;
    migration::migrate(&conn)?;
    Ok(conn)
}

/// 初始化内存数据库(测试用,跑完 migration 到最新版本)。
#[cfg(test)]
pub fn init_db_in_memory() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()
        .map_err(|e| AppError::internal(format!("打开内存 DB 失败: {e}")))?;
    apply_pragmas(&conn)?;
    create_schema(&conn)?;
    migration::migrate(&conn)?;
    Ok(conn)
}

/// 初始化内存数据库但**不跑 migration**(停在 v0/M1 schema,测试 migration 升级用)。
#[cfg(test)]
pub fn init_db_in_memory_v1() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()
        .map_err(|e| AppError::internal(format!("打开内存 DB 失败: {e}")))?;
    apply_pragmas(&conn)?;
    create_schema(&conn)?;
    // 故意不 migrate:user_version 保持 0(M0/M1 库的初始状态)
    Ok(conn)
}

/// 应用必需 PRAGMA(constraints.md SQLite 红线)。
///
/// - `foreign_keys=ON`:启用外键(否则 ON DELETE CASCADE 不生效)
/// - `journal_mode=WAL`:并发读写不冲突(写入时读不被阻塞)
/// - `busy_timeout=5000`:写锁冲突时等 5s 再报错(避免瞬时锁竞争失败)
fn apply_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| AppError::internal(format!("PRAGMA foreign_keys 失败: {e}")))?;
    // WAL 必须用 execute_batch(pragma_update 对 journal_mode 的字符串值处理有问题)
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| AppError::internal(format!("PRAGMA journal_mode=WAL 失败: {e}")))?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .map_err(|e| AppError::internal(format!("PRAGMA busy_timeout 失败: {e}")))?;
    Ok(())
}

/// 建表(M1 手写幂等 migration)。
///
/// `CREATE TABLE IF NOT EXISTS` 保证可重复执行,无需版本号管理。
/// 长期演进(ALTER TABLE)时改引 rusqlite_migration(m1-research.md 建议)。
fn create_schema(conn: &Connection) -> Result<(), AppError> {
    // 用 raw string 保留真实换行,避免反斜杠续行让 SQL 行注释(--)边界混乱。
    conn.execute_batch(
        r#"
        -- 主表:一条历史记录的元数据(不含内容,内容在 formats 表)
        CREATE TABLE IF NOT EXISTS clipboard_items (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            first_copied_at INTEGER NOT NULL,
            last_copied_at  INTEGER NOT NULL,
            copy_count      INTEGER NOT NULL DEFAULT 1,
            pin             TEXT,
            title           TEXT NOT NULL,
            source_app      TEXT,
            content_kind    TEXT NOT NULL DEFAULT 'text'
        );

        -- 预览/排序索引(last_copied_at DESC 是列表默认顺序)
        CREATE INDEX IF NOT EXISTS idx_items_last_copied ON clipboard_items(last_copied_at DESC);

        -- 格式表:一条记录的每个 UTI 格式(一对多)。
        -- M1 每条 item 一行(uti='public.utf8-plain-text');M2 富文本/图片加行。
        CREATE TABLE IF NOT EXISTS clipboard_formats (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id   INTEGER NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
            uti       TEXT NOT NULL,
            data_blob BLOB,
            file_path TEXT,
            size      INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_formats_item ON clipboard_formats(item_id);
        "#,
    )
    .map_err(|e| AppError::internal(format!("建表失败: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_db_in_memory_creates_tables() {
        let conn = init_db_in_memory().expect("内存 DB 初始化应成功");

        // clipboard_items 表存在
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .expect("查 clipboard_items 应成功");
        assert_eq!(count, 0);

        // clipboard_formats 表存在
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_formats", [], |r| r.get(0))
            .expect("查 clipboard_formats 应成功");
        assert_eq!(count, 0);
    }

    #[test]
    fn pragmas_applied() {
        let conn = init_db_in_memory().expect("内存 DB 初始化应成功");
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("查 foreign_keys 应成功");
        assert_eq!(fk, 1, "foreign_keys 应为 ON");
        // busy_timeout = 5000ms
        let bt: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .expect("查 busy_timeout 应成功");
        assert_eq!(bt, 5000);
    }

    #[test]
    fn create_schema_is_idempotent() {
        let conn = init_db_in_memory().expect("首次初始化");
        // 再跑一次不应报错(IF NOT EXISTS 幂等)
        create_schema(&conn).expect("重复建表应幂等成功");
    }
}
