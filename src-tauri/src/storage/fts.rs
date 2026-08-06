//! FTS5 全文搜索 — schema 建表 + 触发器
//!
//! 技术依据见 m2-research.md §一(spike 已验证 8 测试全绿)。
//!
//! 关键设计:
//! - FTS5 外部内容表指向 `clipboard_items`(content='clipboard_items', content_rowid='id')
//! - `search_text` 列存 bigram 预处理后的全文(应用层切,CJK 2 字滑窗)
//! - tokenize='unicode61'(bigram 已切好,这里按空白分词)
//! - 三触发器(ai/ad/au)同步 items 增删改到 FTS
//!
//! ⚠️ bundled feature 已无条件编译 FTS5(libsqlite3-sys build.rs),
//!    无需改 Cargo.toml,直接 CREATE VIRTUAL TABLE USING fts5 即用(spike 实测确认)。

use rusqlite::Connection;

use crate::error::AppError;

/// FTS5 外部内容表 + 三触发器的建表 SQL(IF NOT EXISTS 幂等)。
///
/// 可重复执行:虚拟表和触发器都带 IF NOT EXISTS。
/// ⚠️ 本 SQL 假设 `clipboard_items` 表已有 `search_text` 列(由 migration.rs v2 加)。
const FTS_SCHEMA_SQL: &str = r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
        search_text,
        content='clipboard_items',
        content_rowid='id',
        tokenize='unicode61'
    );

    CREATE TRIGGER IF NOT EXISTS clipboard_fts_ai AFTER INSERT ON clipboard_items BEGIN
        INSERT INTO clipboard_fts(rowid, search_text) VALUES (new.id, new.search_text);
    END;
    CREATE TRIGGER IF NOT EXISTS clipboard_fts_ad AFTER DELETE ON clipboard_items BEGIN
        INSERT INTO clipboard_fts(clipboard_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text);
    END;
    CREATE TRIGGER IF NOT EXISTS clipboard_fts_au AFTER UPDATE ON clipboard_items BEGIN
        INSERT INTO clipboard_fts(clipboard_fts, rowid, search_text) VALUES ('delete', old.id, old.search_text);
        INSERT INTO clipboard_fts(rowid, search_text) VALUES (new.id, new.search_text);
    END;
"#;

/// 建 FTS schema(虚拟表 + 三触发器)。幂等。
///
/// 由 migration.rs 在升级到 v2 时调用。也可在 init_db 后对已迁移的库安全调用(IF NOT EXISTS)。
pub fn create_fts_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(FTS_SCHEMA_SQL)
        .map_err(|e| AppError::internal(format!("建 FTS schema 失败: {e}")))?;
    Ok(())
}
