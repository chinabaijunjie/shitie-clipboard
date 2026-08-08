//! 自动清理 — 保留天数 + 容量上限 + 条数上限
//!
//! M3 新增。在 watcher 每次成功写入后触发。
//!
//! 策略:
//! - retention_days > 0:删除 last_copied_at < (now - days*86400000) 的条目
//! - max_storage_mb > 0:总容量超限时,从最旧(last_copied_at 最小)开始删,直到达标
//! - max_items > 0:条数超限时,从最旧开始删
//!
//! ⚠️ 删除联动图片引用计数(复用 items::delete 逻辑,含磁盘文件清理)。
//! ⚠️ 清理过程包事务(rusqlite unchecked_transaction),中途失败回滚避免半清理状态。
//!
//! 模块化(§1.21):本模块只管「按策略选 id + 逐条删」,不管触发时机(handler 决定)。

use rusqlite::params;
use rusqlite::Connection;
use std::path::Path;

use crate::error::AppError;
use crate::storage::items;

/// 按保留天数清理:删除超过 N 天未使用的条目。
///
/// `days <= 0` 时不清理(0 = 不限)。
/// 返回删除的条目数。
pub fn cleanup_by_retention(
    conn: &Connection,
    days: i64,
    images_dir: &Path,
) -> Result<usize, AppError> {
    if days <= 0 {
        return Ok(0);
    }

    let now = items::now_epoch_ms();
    let threshold = now - days * 86_400_000; // days → ms

    // 查超期条目 id
    let expired_ids: Vec<i64> = conn
        .prepare("SELECT id FROM clipboard_items WHERE last_copied_at < ?1")
        .map_err(|e| AppError::internal(format!("prepare 查超期条目失败: {e}")))?
        .query_map(params![threshold], |r| r.get::<_, i64>(0))
        .map_err(|e| AppError::internal(format!("查超期条目失败: {e}")))?
        .filter_map(Result::ok)
        .collect();

    let count = expired_ids.len();
    for id in expired_ids {
        items::delete(conn, id, images_dir)?;
    }

    if count > 0 {
        log::info!("[cleanup] 保留天数 {} 天:清理了 {} 条过期记录", days, count);
    }

    Ok(count)
}

/// 按容量上限清理:总容量超限时从最旧开始删,直到达标。
///
/// `max_mb <= 0` 时不清理(0 = 不限)。
/// 返回删除的条目数。
///
/// 性能优化:累计已删除字节数,避免每删一条都全表 SUM。
pub fn cleanup_by_capacity(
    conn: &Connection,
    max_mb: i64,
    images_dir: &Path,
) -> Result<usize, AppError> {
    if max_mb <= 0 {
        return Ok(0);
    }

    let max_bytes = max_mb * 1024 * 1024;
    let total = total_storage_bytes(conn)?;

    if total <= max_bytes {
        return Ok(0); // 未超限
    }

    // 查最旧条目的 id + 各自占用的字节数(SUM(size) GROUP BY item_id),一次查完
    let items_with_size: Vec<(i64, i64)> = conn
        .prepare(
            "SELECT i.id, COALESCE(SUM(f.size), 0) AS item_size \
             FROM clipboard_items i \
             LEFT JOIN clipboard_formats f ON f.item_id = i.id \
             GROUP BY i.id \
             ORDER BY i.last_copied_at ASC",
        )
        .map_err(|e| AppError::internal(format!("prepare 查条目容量失败: {e}")))?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| AppError::internal(format!("查条目容量失败: {e}")))?
        .filter_map(Result::ok)
        .collect();

    let excess = total - max_bytes;
    let mut freed: i64 = 0;
    let mut deleted = 0;

    for (id, size) in items_with_size {
        if freed >= excess {
            break;
        }
        items::delete(conn, id, images_dir)?;
        freed += size;
        deleted += 1;
    }

    if deleted > 0 {
        log::info!(
            "[cleanup] 容量上限 {}MB(实际 {}MB):清理了 {} 条最旧记录,释放 {}MB",
            max_mb,
            total / 1024 / 1024,
            deleted,
            freed / 1024 / 1024
        );
    }

    Ok(deleted)
}

/// 按条数上限清理:条数超限时从最旧开始删,直到达标。
///
/// `max_items <= 0` 时不清理(0 = 不限)。
/// 返回删除的条目数。
pub fn cleanup_by_count(
    conn: &Connection,
    max_items: i64,
    images_dir: &Path,
) -> Result<usize, AppError> {
    if max_items <= 0 {
        return Ok(0);
    }

    let total_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .map_err(|e| AppError::internal(format!("查条目总数失败: {e}")))?;

    if total_count <= max_items {
        return Ok(0);
    }

    let excess = (total_count - max_items) as usize;

    // 查最旧的 excess 条
    let oldest_ids: Vec<i64> = conn
        .prepare("SELECT id FROM clipboard_items ORDER BY last_copied_at ASC LIMIT ?1")
        .map_err(|e| AppError::internal(format!("prepare 查超限条目失败: {e}")))?
        .query_map(params![excess as i64], |r| r.get::<_, i64>(0))
        .map_err(|e| AppError::internal(format!("查超限条目失败: {e}")))?
        .filter_map(Result::ok)
        .collect();

    for id in &oldest_ids {
        items::delete(conn, *id, images_dir)?;
    }

    if !oldest_ids.is_empty() {
        log::info!(
            "[cleanup] 条数上限 {} 条(实际 {} 条):清理了 {} 条最旧记录",
            max_items,
            total_count,
            oldest_ids.len()
        );
    }

    Ok(oldest_ids.len())
}

/// 查总存储字节数(SUM(size) from clipboard_formats)。
pub fn total_storage_bytes(conn: &Connection) -> Result<i64, AppError> {
    conn.query_row("SELECT COALESCE(SUM(size), 0) FROM clipboard_formats", [], |r| {
        r.get(0)
    })
    .map_err(|e| AppError::internal(format!("查总容量失败: {e}")))
}

/// 执行完整清理(保留天数 + 容量上限 + 条数上限)。
///
/// 便捷函数,在 watcher handler 里调用。
/// ⚠️ 包事务(unchecked_transaction),中途失败回滚。
pub fn run_cleanup(
    conn: &Connection,
    retention_days: i64,
    max_storage_mb: i64,
    max_items: i64,
    images_dir: &Path,
) -> Result<(), AppError> {
    // 三策略任一启用才执行
    if retention_days <= 0 && max_storage_mb <= 0 && max_items <= 0 {
        return Ok(());
    }

    // 包事务:中途失败回滚(避免半清理状态与前端不一致)
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::internal(format!("开启清理事务失败: {e}")))?;

    cleanup_by_retention(&tx, retention_days, images_dir)?;
    cleanup_by_capacity(&tx, max_storage_mb, images_dir)?;
    cleanup_by_count(&tx, max_items, images_dir)?;

    tx.commit()
        .map_err(|e| AppError::internal(format!("提交清理事务失败: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_in_memory;

    fn make_item(conn: &Connection, text: &str, age_ms: i64, size: i64) {
        // 插入一条 item + 一个 format 行
        let now = items::now_epoch_ms();
        let last_copied = now - age_ms;
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, copy_count, title, content_kind) \
             VALUES (?1, ?2, 1, ?3, 'text')",
            params![last_copied, last_copied, text],
        )
        .unwrap();
        let item_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, data_blob, file_path, size) \
             VALUES (?1, 'public.utf8-plain-text', ?2, NULL, ?3)",
            params![item_id, text.as_bytes(), size],
        )
        .unwrap();
    }

    fn test_images_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clipboard-cleanup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn count_items(conn: &Connection) -> usize {
        conn.query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get::<_, i64>(0))
            .unwrap() as usize
    }

    #[test]
    fn cleanup_by_retention_zero_days_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100);

        let deleted = cleanup_by_retention(&conn, 0, &dir).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(count_items(&conn), 1);
    }

    #[test]
    fn cleanup_by_retention_deletes_expired() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        // 3 天前的(保留 2 天 → 应删)
        make_item(&conn, "旧", 3 * 86_400_000, 100);
        // 1 天前的(保留 2 天 → 保留)
        make_item(&conn, "新", 1 * 86_400_000, 100);

        let deleted = cleanup_by_retention(&conn, 2, &dir).unwrap();
        assert_eq!(deleted, 1, "应删 1 条超期");
        assert_eq!(count_items(&conn), 1, "应剩 1 条");
    }

    #[test]
    fn cleanup_by_retention_keeps_within_window() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 86_400_000, 100); // 1 天前
        make_item(&conn, "B", 86_400_000, 100); // 1 天前

        let deleted = cleanup_by_retention(&conn, 7, &dir).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(count_items(&conn), 2);
    }

    #[test]
    fn cleanup_by_capacity_zero_mb_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100_000_000);

        let deleted = cleanup_by_capacity(&conn, 0, &dir).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn cleanup_by_capacity_deletes_oldest_until_under_limit() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        // 总共 3 条,每条 1MB,上限 2MB → 应删 1 条最旧的
        make_item(&conn, "最旧", 3 * 86_400_000, 1024 * 1024); // 1MB
        make_item(&conn, "中等", 2 * 86_400_000, 1024 * 1024); // 1MB
        make_item(&conn, "最新", 1 * 86_400_000, 1024 * 1024); // 1MB

        let total = total_storage_bytes(&conn).unwrap();
        assert_eq!(total, 3 * 1024 * 1024, "初始总容量 3MB");

        let deleted = cleanup_by_capacity(&conn, 2, &dir).unwrap();
        assert_eq!(deleted, 1, "应删 1 条使总量降到 2MB");
        assert_eq!(count_items(&conn), 2);

        let remaining = total_storage_bytes(&conn).unwrap();
        assert!(remaining <= 2 * 1024 * 1024, "剩余应 ≤ 2MB");
    }

    #[test]
    fn cleanup_by_capacity_under_limit_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100); // 100 字节,远小于 50MB

        let deleted = cleanup_by_capacity(&conn, 50, &dir).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn cleanup_by_count_deletes_excess() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        // 5 条,上限 3 → 删 2 条最旧的
        make_item(&conn, "A", 5 * 86_400_000, 100);
        make_item(&conn, "B", 4 * 86_400_000, 100);
        make_item(&conn, "C", 3 * 86_400_000, 100);
        make_item(&conn, "D", 2 * 86_400_000, 100);
        make_item(&conn, "E", 1 * 86_400_000, 100);

        let deleted = cleanup_by_count(&conn, 3, &dir).unwrap();
        assert_eq!(deleted, 2, "应删 2 条超限");
        assert_eq!(count_items(&conn), 3, "应剩 3 条");

        // 最旧的 A、B 被删,C/D/E 保留
        let titles: Vec<String> = conn
            .prepare("SELECT title FROM clipboard_items ORDER BY last_copied_at DESC")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(titles, vec!["E", "D", "C"]);
    }

    #[test]
    fn cleanup_by_count_zero_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100);

        let deleted = cleanup_by_count(&conn, 0, &dir).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn cleanup_by_count_under_limit_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100);
        make_item(&conn, "B", 0, 100);

        let deleted = cleanup_by_count(&conn, 10, &dir).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn total_storage_bytes_empty_is_zero() {
        let conn = init_db_in_memory().unwrap();
        assert_eq!(total_storage_bytes(&conn).unwrap(), 0);
    }

    #[test]
    fn run_cleanup_runs_all_strategies() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        // 10 天前的 + 1 天前的,各 1MB
        make_item(&conn, "旧", 10 * 86_400_000, 1024 * 1024);
        make_item(&conn, "新", 1 * 86_400_000, 1024 * 1024);

        // 保留 7 天 → 删旧的那条
        run_cleanup(&conn, 7, 0, 0, &dir).unwrap();
        assert_eq!(count_items(&conn), 1, "应只剩 1 条(7天保留删了旧的)");
    }

    #[test]
    fn run_cleanup_all_disabled_noop() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 0, 100);

        run_cleanup(&conn, 0, 0, 0, &dir).unwrap();
        assert_eq!(count_items(&conn), 1);
    }

    #[test]
    fn run_cleanup_count_strategy() {
        let conn = init_db_in_memory().unwrap();
        let dir = test_images_dir();
        make_item(&conn, "A", 5 * 86_400_000, 100);
        make_item(&conn, "B", 3 * 86_400_000, 100);
        make_item(&conn, "C", 1 * 86_400_000, 100);

        // 上限 1 条 → 删 2 条最旧
        run_cleanup(&conn, 0, 0, 1, &dir).unwrap();
        assert_eq!(count_items(&conn), 1);
    }
}
