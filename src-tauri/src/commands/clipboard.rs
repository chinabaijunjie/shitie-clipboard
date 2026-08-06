//! 剪贴板 command — 暴露给前端 invoke 的接口
//!
//! 设计(模块化 §1.21 + 随写随测 §1.22):
//! 两层结构,把可测的核心逻辑与 Tauri 框架包装分离:
//! - 核心逻辑函数(接 &Connection):纯业务,易单测,不依赖 Tauri
//! - command 包装(接 State<AppState>):薄转发,参数转换 + 调核心逻辑
//!
//! M1 command:
//! - list_items:查历史列表(last_copied_at DESC)
//! - copy_back:读 DB → set_text 写回剪贴板(回填)
//! - delete_item:删单条(CASCADE 清 formats)

use clipboard_rs::{Clipboard, ClipboardContext};
use rusqlite::Connection;
use tauri::State;

use crate::error::{codes, AppError};
use crate::models::ClipboardItem;
use crate::storage::items;

/// 列表默认条数上限(前端不传 limit 时)。
const DEFAULT_LIST_LIMIT: i64 = 50;

// ============================================================
// 核心逻辑(接 &Connection,可单测)
// ============================================================

/// 查历史列表。
pub fn list_items_with_conn(conn: &Connection, limit: i64) -> Result<Vec<ClipboardItem>, AppError> {
    items::list(conn, limit)
}

/// 删除单条。
pub fn delete_item_with_conn(conn: &Connection, id: i64) -> Result<(), AppError> {
    items::delete(conn, id)
}

/// 回填:读 DB 取文本 → 写回剪贴板。
///
/// 返回回填的文本(前端可显示/调试用)。
/// 需要 ClipboardContext 参数(测试可传独立实例,生产用全局)。
pub fn copy_back_with_conn(
    conn: &Connection,
    clipboard: &ClipboardContext,
    id: i64,
) -> Result<String, AppError> {
    let text = items::get_text(conn, id)?;
    clipboard
        .set_text(text.clone())
        .map_err(|e| AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("写回剪贴板失败: {e}"),
        ))?;
    Ok(text)
}

// ============================================================
// Tauri command 包装(接 State,薄转发)
// ============================================================

/// 查历史列表。`limit` 默认 50。
#[tauri::command]
pub fn list_items(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, AppError> {
    let conn = state.db();
    list_items_with_conn(&conn, limit.unwrap_or(DEFAULT_LIST_LIMIT))
}

/// 删除单条。
#[tauri::command]
pub fn delete_item(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<(), AppError> {
    let conn = state.db();
    delete_item_with_conn(&conn, id)
}

/// 回填:读 DB 取文本 → 写回剪贴板。
///
/// ⚠️ ClipboardContext 在 command 里现造一个(短生命周期,用完即弃)。
/// M1 每次回填新建 Context 无性能问题(复制操作低频)。
/// ⚠️ 写剪贴板前置 self_write 标志,watcher 会跳过这次自身写入(防反馈循环)。
#[tauri::command]
pub fn copy_back(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<String, AppError> {
    // 先置标志(必须在 set_text 前;watcher 500ms 后轮询到变化时消费此标志)
    state.mark_self_write();

    let conn = state.db();
    let clipboard = ClipboardContext::new()
        .map_err(|e| AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("创建 ClipboardContext 失败: {e}"),
        ))?;
    copy_back_with_conn(&conn, &clipboard, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_in_memory;

    fn db_with_sample_data() -> Connection {
        let conn = init_db_in_memory().expect("内存 DB");
        items::insert_or_update(&conn, "第一条").unwrap();
        // 每条插入间隔 sleep,确保 last_copied_at 毫秒级时间戳递增
        // (否则同毫秒插入导致 DESC 排序不确定)
        std::thread::sleep(std::time::Duration::from_millis(10));
        items::insert_or_update(&conn, "第二条").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        items::insert_or_update(&conn, "第三条").unwrap();
        conn
    }

    #[test]
    fn list_items_returns_all_ordered() {
        let conn = db_with_sample_data();
        let list = list_items_with_conn(&conn, 50).unwrap();
        assert_eq!(list.len(), 3);
        // 按 last_copied_at DESC,第三条最后插入应排第一
        assert_eq!(list[0].title, "第三条");
    }

    #[test]
    fn list_items_respects_limit() {
        let conn = db_with_sample_data();
        let list = list_items_with_conn(&conn, 2).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn delete_item_removes_it() {
        let conn = db_with_sample_data();
        let before = list_items_with_conn(&conn, 50).unwrap();
        assert_eq!(before.len(), 3);

        delete_item_with_conn(&conn, before[1].id).unwrap();

        let after = list_items_with_conn(&conn, 50).unwrap();
        assert_eq!(after.len(), 2);
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        // DELETE 不存在的 id 在 SQLite 是 Ok(无匹配行,影响 0 行,不报错)
        let conn = init_db_in_memory().unwrap();
        let result = delete_item_with_conn(&conn, 9999);
        assert!(result.is_ok(), "删除不存在的 id 应返回 Ok(无副作用)");
    }

    #[test]
    fn copy_back_nonexistent_returns_not_found() {
        let conn = init_db_in_memory().unwrap();
        let clipboard = ClipboardContext::new().expect("ClipboardContext");
        let err = copy_back_with_conn(&conn, &clipboard, 9999).unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }
}
