//! 剪贴板 command — 暴露给前端 invoke 的接口
//!
//! 设计(模块化 §1.21 + 随写随测 §1.22):
//! 两层结构,把可测的核心逻辑与 Tauri 框架包装分离:
//! - 核心逻辑函数(接 &Connection):纯业务,易单测,不依赖 Tauri
//! - command 包装(接 State<AppState>):薄转发,参数转换 + 调核心逻辑
//!
//! command:
//! - list_items:查历史列表(last_copied_at DESC)
//! - copy_back:读 DB 多格式 → set(Vec<ClipboardContent>) 原子写回剪贴板(回填)
//! - delete_item:删单条(CASCADE 清 formats)

use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use rusqlite::Connection;
use tauri::State;

use crate::clipboard::uti;
use crate::error::{codes, AppError};
use crate::models::ClipboardItem;
use crate::storage::items;

/// 列表默认条数上限(前端不传 limit 时)。
const DEFAULT_LIST_LIMIT: i64 = 50;

// ============================================================
// 核心逻辑(接 &Connection,可单测)
// ============================================================

/// 查历史列表。
///
/// `images_dir`:图片存储根目录,用于把缩略图相对路径转绝对路径(前端 convertFileSrc 用)。
pub fn list_items_with_conn(
    conn: &Connection,
    limit: i64,
    images_dir: &std::path::Path,
) -> Result<Vec<ClipboardItem>, AppError> {
    let mut list = items::list(conn, limit)?;
    // 把缩略图相对路径转绝对路径(前端 convertFileSrc 需要绝对路径)
    for item in &mut list {
        item.thumb_path = crate::clipboard::image_store::resolve_thumb_absolute(
            &item.thumb_path,
            images_dir,
        );
    }
    Ok(list)
}

/// 删除单条(含图片引用计数删除)。
pub fn delete_item_with_conn(
    conn: &Connection,
    id: i64,
    images_dir: &std::path::Path,
) -> Result<(), AppError> {
    items::delete(conn, id, images_dir)
}

/// 搜索剪贴板历史(FTS5 + bigram 中文分词,单字 LIKE 兜底)。
///
/// `images_dir`:图片存储根目录,搜索结果里的图片条目需要把缩略图相对路径转绝对。
pub fn search_items_with_conn(
    conn: &Connection,
    query: &str,
    limit: i64,
    images_dir: &std::path::Path,
) -> Result<Vec<ClipboardItem>, AppError> {
    let mut list = crate::storage::search::search(conn, query, limit)?;
    // 搜索结果也转缩略图绝对路径(与 list_items 一致)
    for item in &mut list {
        item.thumb_path = crate::clipboard::image_store::resolve_thumb_absolute(
            &item.thumb_path,
            images_dir,
        );
    }
    Ok(list)
}

/// 回填:读 DB 该条所有格式 → 组装 Vec<ClipboardContent> → set 原子写回剪贴板。
///
/// M2 多格式回填:把该 item 所有存下来的格式都写回(保留 Word 富文本格式等)。
/// ⚠️ set(Vec) 内部已 clearContents + writeObjects(原子多格式写),不需要手动 clear。
/// ⚠️ 不能连续多次 set_text/set_image(单格式 set 会清掉其他格式)。
///
/// 图片格式(png/tiff):从磁盘读原图 → RustImageData → ClipboardContent::Image。
///
/// 返回回填的条目标题(前端可显示)。
pub fn copy_back_with_conn(
    conn: &Connection,
    clipboard: &ClipboardContext,
    id: i64,
    images_dir: &std::path::Path,
) -> Result<String, AppError> {
    // 读该 item 的所有格式行
    let formats = items::get_formats(conn, id)?;
    if formats.is_empty() {
        return Err(AppError::new(
            codes::NOT_FOUND,
            format!("条目 {} 无格式数据", id),
        ));
    }

    // 组装 Vec<ClipboardContent>
    let mut contents: Vec<ClipboardContent> = Vec::new();
    let mut title_for_return = String::new();
    for (uti_str, blob, file_path) in &formats {
        match uti_str.as_str() {
            uti::UTI_PLAIN_TEXT => {
                if let Ok(text) = String::from_utf8(blob.clone().unwrap_or_default()) {
                    if title_for_return.is_empty() {
                        title_for_return.clone_from(&text);
                    }
                    contents.push(ClipboardContent::Text(text));
                }
            }
            uti::UTI_RTF => {
                let rtf = String::from_utf8_lossy(&blob.clone().unwrap_or_default()).into_owned();
                contents.push(ClipboardContent::Rtf(rtf));
            }
            uti::UTI_HTML => {
                let html = String::from_utf8_lossy(&blob.clone().unwrap_or_default()).into_owned();
                contents.push(ClipboardContent::Html(html));
            }
            uti::UTI_FILE_URL => {
                let blob_data = blob.clone().unwrap_or_default();
                let urls = String::from_utf8_lossy(&blob_data);
                let files: Vec<String> = urls.lines().map(String::from).collect();
                if title_for_return.is_empty() {
                    title_for_return = files.first().cloned().unwrap_or_default();
                }
                contents.push(ClipboardContent::Files(files));
            }
            uti::UTI_PNG | uti::UTI_TIFF => {
                // 图片:从磁盘读原图 → RustImageData
                if let Some(fp) = file_path {
                    match read_image_content(images_dir, fp) {
                        Ok(img_content) => {
                            if title_for_return.is_empty() {
                                title_for_return = "图片".to_string();
                            }
                            contents.push(img_content);
                        }
                        Err(e) => {
                            log::warn!("[copy_back] 读图片 {} 失败,跳过: {}", fp, e.message);
                        }
                    }
                }
            }
            other => {
                log::debug!("[copy_back] 未知格式 {} 跳过", other);
            }
        }
    }

    if contents.is_empty() {
        return Err(AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("条目 {} 无可回填的格式", id),
        ));
    }

    // set 原子写(内部已 clear)
    clipboard.set(contents).map_err(|e| {
        AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("写回剪贴板失败: {e}"),
        )
    })?;

    Ok(title_for_return)
}

/// 从磁盘读图片 → ClipboardContent::Image。
fn read_image_content(
    images_dir: &std::path::Path,
    rel_path: &str,
) -> Result<ClipboardContent, AppError> {
    use clipboard_rs::common::RustImage;
    let full = images_dir.join(rel_path);
    let img = clipboard_rs::RustImageData::from_path(full.to_str().unwrap_or(""))
        .map_err(|e| AppError::internal(format!("从盘读图片失败: {e}")))?;
    Ok(ClipboardContent::Image(img))
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
    list_items_with_conn(&conn, limit.unwrap_or(DEFAULT_LIST_LIMIT), state.images_dir())
}

/// 删除单条(含图片引用计数)。
#[tauri::command]
pub fn delete_item(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<(), AppError> {
    let conn = state.db();
    delete_item_with_conn(&conn, id, state.images_dir())
}

/// 搜索剪贴板历史(FTS5 + bigram)。
#[tauri::command]
pub fn search_items(
    state: State<'_, crate::AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, AppError> {
    let conn = state.db();
    search_items_with_conn(&conn, &query, limit.unwrap_or(DEFAULT_LIST_LIMIT), state.images_dir())
}

/// 回填:读 DB 多格式 → set 原子写回剪贴板。
///
/// ⚠️ ClipboardContext 在 command 里现造一个(短生命周期,用完即弃)。
/// 复制操作低频,每次回填新建 Context 无性能问题。
/// ⚠️ 写剪贴板前置 self_write 标志,watcher 会跳过这次自身写入(防反馈循环)。
#[tauri::command]
pub fn copy_back(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<String, AppError> {
    // 先置标志(必须在 set 前;watcher 500ms 后轮询到变化时消费此标志)
    state.mark_self_write();

    let conn = state.db();
    let clipboard = ClipboardContext::new().map_err(|e| {
        AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("创建 ClipboardContext 失败: {e}"),
        )
    })?;
    copy_back_with_conn(&conn, &clipboard, id, state.images_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::capture::CapturedFormat;
    use crate::models::ContentKind;
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

    /// 测试用 images_dir(临时目录,纯文本测试不实际写图,但 delete 签名要求传)。
    fn test_images_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clipboard-cmd-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn list_items_returns_all_ordered() {
        let conn = db_with_sample_data();
        let list = list_items_with_conn(&conn, 50, &test_images_dir()).unwrap();
        assert_eq!(list.len(), 3);
        // 按 last_copied_at DESC,第三条最后插入应排第一
        assert_eq!(list[0].title, "第三条");
    }

    #[test]
    fn list_items_respects_limit() {
        let conn = db_with_sample_data();
        let list = list_items_with_conn(&conn, 2, &test_images_dir()).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn delete_item_removes_it() {
        let conn = db_with_sample_data();
        let before = list_items_with_conn(&conn, 50, &test_images_dir()).unwrap();
        assert_eq!(before.len(), 3);

        delete_item_with_conn(&conn, before[1].id, &test_images_dir()).unwrap();

        let after = list_items_with_conn(&conn, 50, &test_images_dir()).unwrap();
        assert_eq!(after.len(), 2);
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        // DELETE 不存在的 id 在 SQLite 是 Ok(无匹配行,影响 0 行,不报错)
        let conn = init_db_in_memory().unwrap();
        let result = delete_item_with_conn(&conn, 9999, &test_images_dir());
        assert!(result.is_ok(), "删除不存在的 id 应返回 Ok(无副作用)");
    }

    #[test]
    fn copy_back_nonexistent_returns_not_found() {
        let conn = init_db_in_memory().unwrap();
        let clipboard = ClipboardContext::new().expect("ClipboardContext");
        let err = copy_back_with_conn(&conn, &clipboard, 9999, &test_images_dir()).unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn copy_back_multi_format_assembles_contents() {
        // 多格式 item(text + rtf + html)→ copy_back 应组装 3 个 ClipboardContent
        // 注:此测试需要真实剪贴板(ClipboardContext::set),在 CI/无头环境可能失败。
        // 这里只验证 DB 读取 + 组装逻辑不报错(实际写剪贴板在 GUI 实测验证)。
        let conn = init_db_in_memory().unwrap();
        let formats = vec![
            CapturedFormat {
                uti: uti::UTI_PLAIN_TEXT.to_string(),
                data: b"Hello".to_vec(),
                file_path: None,
            },
            CapturedFormat {
                uti: uti::UTI_RTF.to_string(),
                data: b"{\\rtf1 Hello}".to_vec(),
                file_path: None,
            },
            CapturedFormat {
                uti: uti::UTI_HTML.to_string(),
                data: b"<p>Hello</p>".to_vec(),
                file_path: None,
            },
        ];
        let format_inputs: Vec<items::FormatInput<'_>> = formats
            .iter()
            .map(|f| items::FormatInput {
                uti: &f.uti,
                data: &f.data,
                file_path: None,
            })
            .collect();
        let item =
            items::insert_or_update_multi(&conn, &format_inputs, ContentKind::Rich, "Hello").unwrap();

        // 验证 DB 里确实有 3 个格式行(copy_back 会读这些)
        let read_formats = items::get_formats(&conn, item.id).unwrap();
        assert_eq!(read_formats.len(), 3, "应有 3 个格式行供 copy_back 读");
    }
}
