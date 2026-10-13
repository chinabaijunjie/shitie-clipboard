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
//! - get_item_text:读条目完整文本(预览面板用,title 截断 1000 字符以外的全文)

use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use rusqlite::Connection;
use tauri::State;

use crate::clipboard::{title, uti};
use crate::error::{codes, AppError};
use crate::models::{ClipboardItem, ListPage, StorageStats};
use crate::storage::{cleanup, items};

/// 列表默认条数上限(前端不传 limit 时)。
const DEFAULT_LIST_LIMIT: i64 = 50;

/// 更新指定条目的 last_copied_at 为当前时间(M3:回填后移到最前)。
///
/// ⚠️ 不加 copy_count:copy_count 反映「被复制次数」,回填不是新复制。
fn update_last_copied_at(conn: &Connection, id: i64) -> Result<(), AppError> {
    let now = items::now_epoch_ms();
    conn.execute(
        "UPDATE clipboard_items SET last_copied_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| AppError::internal(format!("更新 last_copied_at 失败: {e}")))?;
    Ok(())
}

// ============================================================
// 核心逻辑(接 &Connection,可单测)
// ============================================================

/// 查历史列表(不分页辅助,offset=0;命令分页路径走 items::list + count)。
///
/// `images_dir`:图片存储根目录,用于把缩略图相对路径转绝对路径(前端 convertFileSrc 用)。
pub fn list_items_with_conn(
    conn: &Connection,
    limit: i64,
    images_dir: &std::path::Path,
    kinds: Option<&[String]>,
) -> Result<Vec<ClipboardItem>, AppError> {
    // items::list 已通过 fill_image_fields 把 thumb_path 填为绝对路径
    items::list(conn, limit, 0, images_dir, kinds)
}

/// 分页查历史列表(命令层核心逻辑:当前页 + 过滤口径下的命中总数)。
pub fn list_page_with_conn(
    conn: &Connection,
    limit: i64,
    offset: i64,
    images_dir: &std::path::Path,
    kinds: Option<&[String]>,
) -> Result<ListPage, AppError> {
    let items = items::list(conn, limit, offset, images_dir, kinds)?;
    let total = items::count(conn, kinds)?;
    Ok(ListPage { items, total })
}

/// 删除单条(含图片引用计数删除)。
pub fn delete_item_with_conn(
    conn: &Connection,
    id: i64,
    images_dir: &std::path::Path,
) -> Result<(), AppError> {
    items::delete(conn, id, images_dir)
}

/// 搜索剪贴板历史(不分页辅助,offset=0)。
pub fn search_items_with_conn(
    conn: &Connection,
    query: &str,
    limit: i64,
    images_dir: &std::path::Path,
) -> Result<Vec<ClipboardItem>, AppError> {
    crate::storage::search::search(conn, query, limit, 0, images_dir)
}

/// 分页搜索(命令层核心逻辑:当前页 + 命中总数)。
pub fn search_page_with_conn(
    conn: &Connection,
    query: &str,
    limit: i64,
    offset: i64,
    images_dir: &std::path::Path,
) -> Result<ListPage, AppError> {
    let items = crate::storage::search::search(conn, query, limit, offset, images_dir)?;
    let total = crate::storage::search::count(conn, query)?;
    Ok(ListPage { items, total })
}

/// 在格式行中找指定 UTI 的 blob,按 UTF-8(lossy)解码为文本。
///
/// lossy 而非 strict:预览是尽力展示,坏字节替换 U+FFFD 优于整段报错
/// (与 M2 rich/html 捕获时的 lossy 策略一致)。
fn find_blob_text(formats: &[items::FormatRow], want_uti: &str) -> Option<String> {
    formats
        .iter()
        .find(|(u, blob, _)| u == want_uti && blob.is_some())
        .map(|(_, blob, _)| String::from_utf8_lossy(blob.as_ref().unwrap()).into_owned())
}

/// 读条目完整文本(右侧预览面板用)。
///
/// 格式优先级与 generate_title 一致:plain-text 直读 → RTF 粗提取 → HTML 粗提取。
/// 条目不存在、或无任何文本格式(纯图片/纯文件)→ NOT_FOUND。
pub fn get_item_text_with_conn(conn: &Connection, id: i64) -> Result<String, AppError> {
    let formats = items::get_formats(conn, id)?;
    if formats.is_empty() {
        return Err(AppError::new(codes::NOT_FOUND, format!("条目 {id} 不存在")));
    }
    if let Some(text) = find_blob_text(&formats, uti::UTI_PLAIN_TEXT) {
        return Ok(text);
    }
    if let Some(rtf) = find_blob_text(&formats, uti::UTI_RTF) {
        return Ok(title::extract_text_from_rtf(&rtf));
    }
    if let Some(html) = find_blob_text(&formats, uti::UTI_HTML) {
        return Ok(title::extract_text_from_html(&html));
    }
    Err(AppError::new(codes::NOT_FOUND, format!("条目 {id} 无文本格式")))
}

/// 日志预览用安全截断:按**字符数**取前 `n` 个字符。
///
/// ⚠️ 禁止改回 `&s[..n]` 字节切片:UTF-8 多字节字符(中文 3 字节/emoji 4 字节)的
/// 第 n 字节可能落在字符中间,切片直接 panic。v2.0.0 曾因此在 copy_back 崩溃
/// (纯中文标题 ≥14 字即触发,主线程 panic → 进程 abort,用户机四份崩溃报告同签名)。
fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
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

    // 组装 Vec<ClipboardContent>(纯函数,可单测)
    let assembled = assemble_contents(&formats, images_dir);

    // 诊断日志:回填了哪些格式(排障关键)
    let format_summary: Vec<&str> = assembled.contents.iter().map(|c| match c {
        ClipboardContent::Text(_) => "Text",
        ClipboardContent::Rtf(_) => "Rtf",
        ClipboardContent::Html(_) => "Html",
        ClipboardContent::Image(_) => "Image",
        ClipboardContent::Files(_) => "Files",
        ClipboardContent::Other(_, _) => "Other",
    }).collect();
    log::info!("[copy_back] 条目 {} 回填格式: {:?}, 标题: {:?}",
        id, format_summary, &truncate_chars(&assembled.title, 40));

    if assembled.contents.is_empty() {
        // 若曾有图片读盘失败,用更明确的错误(告知磁盘图可能损坏),而非笼统「无可回填格式」
        let msg = assembled
            .image_read_error
            .unwrap_or_else(|| format!("条目 {} 无可回填的格式", id));
        return Err(AppError::new(codes::CLIPBOARD_WRITE_FAILED, msg));
    }

    // set 原子写(内部已 clear)
    clipboard.set(assembled.contents).map_err(|e| {
        AppError::new(
            codes::CLIPBOARD_WRITE_FAILED,
            format!("写回剪贴板失败: {e}"),
        )
    })?;
    log::info!("[copy_back] 条目 {} 写回剪贴板成功", id);

    // M3:更新 last_copied_at(回填后将该条移到最前)
    // ⚠️ 不加 copy_count:copy_count 反映「被复制次数」,回填不是新复制
    update_last_copied_at(conn, id)?;

    Ok(assembled.title)
}

/// [`assemble_contents`] 的返回(组装结果)。
struct AssembledContents {
    /// 组装出的剪贴板内容列表(给 clipboard.set 用)。
    contents: Vec<ClipboardContent>,
    /// 回填标题(给前端显示,取第一个有效格式的文本/文件名/「图片」)。
    title: String,
    /// 图片读盘失败信息(若图片读盘失败导致 contents 为空,调用方用此生成明确报错)。
    image_read_error: Option<String>,
}

/// 把 DB 格式行组装成 `Vec<ClipboardContent>`(纯函数,可单测)。
///
/// 逐行解析格式:
/// - 文本/富文本/HTML → 对应 `ClipboardContent` 变体
/// - 文件 URL → `ClipboardContent::Files`(按行 split)
/// - 图片(PNG/TIFF)→ 从磁盘读原图 → `ClipboardContent::Image`
/// - 非白名单 UTI → 跳过(debug 日志)
///
/// `title` 取第一个有效格式的值(纯文本内容 / 首个文件 URL / 「图片」)。
/// 图片读盘失败不阻断,记录到 `image_read_error`(若导致 contents 为空,调用方据此报错)。
fn assemble_contents(
    formats: &[items::FormatRow],
    images_dir: &std::path::Path,
) -> AssembledContents {
    let mut contents: Vec<ClipboardContent> = Vec::new();
    let mut title = String::new();
    let mut image_read_error: Option<String> = None;

    for (uti_str, blob, file_path) in formats {
        // WhitelistUti::from_str 把 UTI 字符串解析成穷尽 enum(非白名单 → None → 跳过)
        let Some(white_uti) = crate::clipboard::types::WhitelistUti::from_str(uti_str.as_str()) else {
            log::debug!("[copy_back] 未知格式 {} 跳过", uti_str);
            continue;
        };
        match white_uti {
            crate::clipboard::types::WhitelistUti::PlainText => {
                if let Ok(text) = String::from_utf8(blob.clone().unwrap_or_default()) {
                    if title.is_empty() {
                        title.clone_from(&text);
                    }
                    contents.push(ClipboardContent::Text(text));
                }
            }
            crate::clipboard::types::WhitelistUti::Rich => {
                let rtf = String::from_utf8_lossy(&blob.clone().unwrap_or_default()).into_owned();
                contents.push(ClipboardContent::Rtf(rtf));
            }
            crate::clipboard::types::WhitelistUti::Html => {
                let html = String::from_utf8_lossy(&blob.clone().unwrap_or_default()).into_owned();
                contents.push(ClipboardContent::Html(html));
            }
            crate::clipboard::types::WhitelistUti::FileUrl => {
                let blob_data = blob.clone().unwrap_or_default();
                let urls = String::from_utf8_lossy(&blob_data);
                let files: Vec<String> = urls.lines().map(String::from).collect();
                if title.is_empty() {
                    title = files.first().cloned().unwrap_or_default();
                }
                contents.push(ClipboardContent::Files(files));
            }
            crate::clipboard::types::WhitelistUti::Png | crate::clipboard::types::WhitelistUti::Tiff => {
                // 图片:从磁盘读原图 → RustImageData
                if let Some(fp) = file_path {
                    match read_image_content(images_dir, fp) {
                        Ok(img_content) => {
                            if title.is_empty() {
                                title = "图片".to_string();
                            }
                            contents.push(img_content);
                        }
                        Err(e) => {
                            log::warn!("[copy_back] 读图片 {} 失败,跳过: {}", fp, e.message);
                            image_read_error = Some(format!(
                                "图片文件 {} 读取失败: {}(磁盘图片可能已损坏/被清理)",
                                fp, e.message
                            ));
                        }
                    }
                }
            }
        }
    }

    AssembledContents {
        contents,
        title,
        image_read_error,
    }
}

/// 从磁盘读图片 → ClipboardContent::Image。
fn read_image_content(
    images_dir: &std::path::Path,
    rel_path: &crate::clipboard::types::RelPath,
) -> Result<ClipboardContent, AppError> {
    use clipboard_rs::common::RustImage;
    let full = images_dir.join(rel_path.as_str());
    // 路径非 UTF-8 时显式报错(而非 unwrap_or("") 用空串兜底,误导排查)
    let path_str = full.to_str().ok_or_else(|| {
        AppError::internal(format!("图片路径含非 UTF-8 字符: {}", full.display()))
    })?;
    let img = clipboard_rs::RustImageData::from_path(path_str)
        .map_err(|e| AppError::internal(format!("从盘读图片失败 {}: {e}", full.display())))?;
    Ok(ClipboardContent::Image(img))
}

// ============================================================
// Tauri command 包装(接 State,薄转发)
// ============================================================

/// 查历史列表(分页)。`limit` 默认从设置读(用户配置的 max_items);`offset` 分页偏移。
#[tauri::command]
pub fn list_items(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    kinds: Option<Vec<String>>,
) -> Result<ListPage, AppError> {
    let effective_limit = match limit {
        Some(l) => l,
        None => {
            let guard = state.settings();
            let s = guard.lock().expect("Settings Mutex 中毒");
            let max = s.max_items;
            if max > 0 { max } else { DEFAULT_LIST_LIMIT }
        }
    };
    // kinds: 空 Vec 或 None = 不过滤
    let kinds_ref = kinds.filter(|ks| !ks.is_empty());
    let conn = state.db();
    list_page_with_conn(
        &conn,
        effective_limit,
        offset.unwrap_or(0),
        state.images_dir(),
        kinds_ref.as_deref(),
    )
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

/// 切换置顶状态(置顶/取消置顶)。返回切换后的新状态(true=已置顶)。
#[tauri::command]
pub fn toggle_pin(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<bool, AppError> {
    let conn = state.db();
    items::toggle_pin(&conn, id)
}

/// 搜索剪贴板历史(分页,FTS5 + bigram)。
#[tauri::command]
pub fn search_items(
    state: State<'_, crate::AppState>,
    query: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<ListPage, AppError> {
    let conn = state.db();
    search_page_with_conn(
        &conn,
        &query,
        limit.unwrap_or(DEFAULT_LIST_LIMIT),
        offset.unwrap_or(0),
        state.images_dir(),
    )
}

/// 读条目完整文本(右侧预览面板用)。
///
/// title 截断在 1000 字符(TITLE_MAX_CHARS),超长条目以此命令取全文。
#[tauri::command]
pub fn get_item_text(
    state: State<'_, crate::AppState>,
    id: i64,
) -> Result<String, AppError> {
    let conn = state.db();
    get_item_text_with_conn(&conn, id)
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

/// 存储统计(设置面板「存储」模块用)。
#[tauri::command]
pub fn get_storage_stats(state: State<'_, crate::AppState>) -> Result<StorageStats, AppError> {
    let conn = state.db();
    let kind_map = items::count_by_kind(&conn)?;
    let by_kind = items::storage_by_kind_from_map(&kind_map);
    let total: i64 = by_kind.text + by_kind.rich + by_kind.image + by_kind.file + by_kind.mixed;
    let storage_bytes = cleanup::total_storage_bytes(&conn)?;
    Ok(StorageStats {
        total,
        by_kind,
        storage_bytes,
    })
}

/// 清空所有历史(DB + 图片文件)。
#[tauri::command]
pub fn clear_history(state: State<'_, crate::AppState>) -> Result<(), AppError> {
    let conn = state.db();
    let count = items::delete_all(&conn, state.images_dir())?;
    log::info!("[commands] 用户手动清空历史,删除 {} 条", count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::capture::CapturedFormat;
    use crate::clipboard::uti;
    use crate::models::ContentKind;
    use crate::storage::init_db_in_memory;

    fn db_with_sample_data() -> Connection {
        let conn = init_db_in_memory().expect("内存 DB");
        let dir = test_images_dir();
        items::insert_or_update(&conn, "第一条", &dir).unwrap();
        // 每条插入间隔 sleep,确保 last_copied_at 毫秒级时间戳递增
        // (否则同毫秒插入导致 DESC 排序不确定)
        std::thread::sleep(std::time::Duration::from_millis(10));
        items::insert_or_update(&conn, "第二条", &dir).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        items::insert_or_update(&conn, "第三条", &dir).unwrap();
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
        let list = list_items_with_conn(&conn, 50, &test_images_dir(), None).unwrap();
        assert_eq!(list.len(), 3);
        // 按 last_copied_at DESC,第三条最后插入应排第一
        assert_eq!(list[0].title, "第三条");
    }

    #[test]
    fn list_items_respects_limit() {
        let conn = db_with_sample_data();
        let list = list_items_with_conn(&conn, 2, &test_images_dir(), None).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn delete_item_removes_it() {
        let conn = db_with_sample_data();
        let before = list_items_with_conn(&conn, 50, &test_images_dir(), None).unwrap();
        assert_eq!(before.len(), 3);

        delete_item_with_conn(&conn, before[1].id, &test_images_dir()).unwrap();

        let after = list_items_with_conn(&conn, 50, &test_images_dir(), None).unwrap();
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

    // ============================================================
    // get_item_text(预览面板全文)
    // ============================================================

    /// 构造单条多格式条目,返回其 id。
    fn insert_with_formats(
        conn: &Connection,
        formats: &[(&str, &[u8])],
        kind: ContentKind,
        title: &str,
    ) -> i64 {
        let captured: Vec<CapturedFormat> = formats
            .iter()
            .map(|(u, d)| CapturedFormat {
                uti: u.to_string(),
                data: d.to_vec(),
                file_path: None,
            })
            .collect();
        let inputs: Vec<items::FormatInput<'_>> = captured
            .iter()
            .map(|f| items::FormatInput {
                uti: &f.uti,
                data: &f.data,
                file_path: None,
            })
            .collect();
        items::insert_or_update_multi(conn, &inputs, kind, title, &test_images_dir())
            .unwrap()
            .id
    }

    #[test]
    fn get_item_text_plain_direct() {
        let conn = init_db_in_memory().unwrap();
        // 超过 TITLE_MAX_CHARS(1000)的长文本,预览面板需要全文而非截断 title
        let long = "长".repeat(1500);
        let id = insert_with_formats(
            &conn,
            &[(uti::UTI_PLAIN_TEXT, long.as_bytes())],
            ContentKind::Text,
            "长文本",
        );
        assert_eq!(get_item_text_with_conn(&conn, id).unwrap(), long);
    }

    #[test]
    fn get_item_text_rtf_fallback() {
        let conn = init_db_in_memory().unwrap();
        let id = insert_with_formats(
            &conn,
            &[(uti::UTI_RTF, "{\\rtf1\\ansi RTF 内容}".as_bytes())],
            ContentKind::Rich,
            "RTF 条目",
        );
        let text = get_item_text_with_conn(&conn, id).unwrap();
        assert!(text.contains("RTF 内容"), "应剥离控制词,实际: {text}");
    }

    #[test]
    fn get_item_text_html_fallback() {
        let conn = init_db_in_memory().unwrap();
        let id = insert_with_formats(
            &conn,
            &[(uti::UTI_HTML, b"<p>Hello <b>world</b></p>".as_slice())],
            ContentKind::Rich,
            "HTML 条目",
        );
        assert_eq!(get_item_text_with_conn(&conn, id).unwrap(), "Hello world");
    }

    #[test]
    fn get_item_text_nonexistent_not_found() {
        let conn = init_db_in_memory().unwrap();
        let err = get_item_text_with_conn(&conn, 9999).unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    #[test]
    fn get_item_text_no_text_format_not_found() {
        let conn = init_db_in_memory().unwrap();
        // 仅文件 URL 格式(无任何文本格式)→ NOT_FOUND
        let id = insert_with_formats(
            &conn,
            &[(uti::UTI_FILE_URL, b"file:///tmp/a.txt".as_slice())],
            ContentKind::File,
            "a.txt",
        );
        let err = get_item_text_with_conn(&conn, id).unwrap_err();
        assert_eq!(err.code, codes::NOT_FOUND);
    }

    // ============================================================
    // 分页(offset + total)
    // ============================================================

    #[test]
    fn list_offset_pagination_covers_all_without_overlap() {
        let conn = db_with_sample_data(); // 3 条
        let page0 = list_page_with_conn(&conn, 2, 0, &test_images_dir(), None).unwrap();
        assert_eq!(page0.total, 3);
        assert_eq!(page0.items.len(), 2);
        let page1 = list_page_with_conn(&conn, 2, 2, &test_images_dir(), None).unwrap();
        assert_eq!(page1.items.len(), 1);
        // 两页无重叠,并集 = 全集(id DESC tiebreaker 保证翻页稳定)
        let mut page_ids: Vec<i64> = page0.items.iter().chain(&page1.items).map(|i| i.id).collect();
        let mut full_ids: Vec<i64> =
            list_items_with_conn(&conn, 50, &test_images_dir(), None).unwrap().iter().map(|i| i.id).collect();
        page_ids.sort_unstable();
        full_ids.sort_unstable();
        assert_eq!(page_ids, full_ids);
    }

    #[test]
    fn list_count_matches_kinds_filter() {
        let conn = db_with_sample_data(); // 3 条纯文字
        let image_page = list_page_with_conn(
            &conn, 50, 0, &test_images_dir(), Some(&["image".to_string()]),
        ).unwrap();
        assert_eq!(image_page.total, 0);
        assert!(image_page.items.is_empty());
        let all = list_page_with_conn(&conn, 50, 0, &test_images_dir(), None).unwrap();
        assert_eq!(all.total, 3);
    }

    #[test]
    fn search_offset_and_count_pages_disjoint() {
        let conn = db_with_sample_data(); // 第一条/第二条/第三条,单字「条」LIKE 全命中
        let page0 = search_page_with_conn(&conn, "条", 2, 0, &test_images_dir()).unwrap();
        assert_eq!(page0.total, 3);
        assert_eq!(page0.items.len(), 2);
        let page1 = search_page_with_conn(&conn, "条", 2, 2, &test_images_dir()).unwrap();
        assert_eq!(page1.items.len(), 1);
        let mut page_ids: Vec<i64> = page0.items.iter().chain(&page1.items).map(|i| i.id).collect();
        let mut full_ids: Vec<i64> =
            search_items_with_conn(&conn, "条", 50, &test_images_dir()).unwrap().iter().map(|i| i.id).collect();
        page_ids.sort_unstable();
        full_ids.sort_unstable();
        assert_eq!(page_ids, full_ids);
        // 多字查询走 FTS,count 口径一致
        let fts_page = search_page_with_conn(&conn, "第二", 10, 0, &test_images_dir()).unwrap();
        assert_eq!(fts_page.total, 1);
        assert_eq!(fts_page.items.len(), 1);
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
            items::insert_or_update_multi(&conn, &format_inputs, ContentKind::Rich, "Hello", &test_images_dir()).unwrap();

        // 验证 DB 里确实有 3 个格式行(copy_back 会读这些)
        let read_formats = items::get_formats(&conn, item.id).unwrap();
        assert_eq!(read_formats.len(), 3, "应有 3 个格式行供 copy_back 读");
    }

    // ===== assemble_contents 纯函数测试(不依赖真实剪贴板) =====

    /// 构造一个文本类格式行(uti + bytes,无 file_path)。
    fn text_row(uti: &str, data: &[u8]) -> items::FormatRow {
        (
            uti.to_string(),
            Some(data.to_vec()),
            None,
        )
    }

    /// 构造一个图片格式行(uti + file_path,data 为空)。
    fn image_row(uti: &str, file_path: &str) -> items::FormatRow {
        (
            uti.to_string(),
            None,
            Some(crate::clipboard::types::RelPath::from_str(file_path)),
        )
    }

    #[test]
    fn assemble_text_only() {
        // 纯文本格式 → 1 个 Text content + title 取文本内容
        let formats = [text_row(uti::UTI_PLAIN_TEXT, b"Hello world")];
        let result = assemble_contents(&formats, &test_images_dir());

        assert_eq!(result.contents.len(), 1, "应组装 1 个 content");
        assert!(matches!(result.contents[0], ClipboardContent::Text(_)), "应为 Text");
        assert_eq!(result.title, "Hello world");
        assert!(result.image_read_error.is_none());
    }

    #[test]
    fn assemble_rich_text() {
        // text + rtf + html → 3 个 content
        let formats = [
            text_row(uti::UTI_PLAIN_TEXT, b"Hello"),
            text_row(uti::UTI_RTF, b"{\\rtf1 Hello}"),
            text_row(uti::UTI_HTML, b"<p>Hello</p>"),
        ];
        let result = assemble_contents(&formats, &test_images_dir());

        assert_eq!(result.contents.len(), 3, "应组装 3 个 content");
        assert_eq!(result.title, "Hello", "title 取首个(纯文本)");
    }

    #[test]
    fn assemble_file_url() {
        // file-url → Files content + title 取首个文件 URL
        let formats = [text_row(uti::UTI_FILE_URL, b"file:///Users/foo/report.pdf")];
        let result = assemble_contents(&formats, &test_images_dir());

        assert_eq!(result.contents.len(), 1);
        match &result.contents[0] {
            ClipboardContent::Files(paths) => {
                assert_eq!(paths.len(), 1);
                assert_eq!(paths[0], "file:///Users/foo/report.pdf");
            }
            _ => panic!("应为 Files"),
        }
        assert_eq!(result.title, "file:///Users/foo/report.pdf");
    }

    #[test]
    fn assemble_file_url_multiple() {
        // 多文件 URL(按行 split)
        let data = b"file:///a.png\nfile:///b.png\nfile:///c.png";
        let formats = [text_row(uti::UTI_FILE_URL, data)];
        let result = assemble_contents(&formats, &test_images_dir());

        match &result.contents[0] {
            ClipboardContent::Files(paths) => assert_eq!(paths.len(), 3),
            _ => panic!("应为 Files"),
        }
    }

    #[test]
    fn assemble_unknown_format_skipped() {
        // 非白名单 UTI → 跳过,不报错
        let formats = [
            text_row("com.apple.linkpresentation.metadata", b"noise"),
            text_row(uti::UTI_PLAIN_TEXT, "有效文本".as_bytes()),
        ];
        let result = assemble_contents(&formats, &test_images_dir());

        assert_eq!(result.contents.len(), 1, "噪音格式应跳过,只留 1 个");
        assert_eq!(result.title, "有效文本");
    }

    #[test]
    fn assemble_image_missing_file() {
        // 图片格式但 file_path 指向不存在的文件 → image_read_error 有值 + contents 空
        let formats = [image_row(uti::UTI_PNG, "ab/nonexistent.png")];
        let result = assemble_contents(&formats, &test_images_dir());

        assert!(result.contents.is_empty(), "图片读盘失败 → contents 应空");
        assert!(result.image_read_error.is_some(), "应有图片读盘错误信息");
        // title 为空(图片没成功加入)
        assert!(result.title.is_empty());
    }

    #[test]
    fn assemble_empty_formats() {
        // 空格式列表 → 空结果
        let result = assemble_contents(&[], &test_images_dir());
        assert!(result.contents.is_empty());
        assert!(result.title.is_empty());
        assert!(result.image_read_error.is_none());
    }

    #[test]
    fn assemble_title_priority_text_over_file() {
        // 纯文本 + file-url:title 取纯文本(先出现)
        let formats = [
            text_row(uti::UTI_PLAIN_TEXT, "文本内容".as_bytes()),
            text_row(uti::UTI_FILE_URL, b"file:///x.pdf"),
        ];
        let result = assemble_contents(&formats, &test_images_dir());
        assert_eq!(result.title, "文本内容", "title 取首个有效格式的值");
    }

    // ===== truncate_chars 测试(v2.0.1 崩溃修复回归) =====

    /// 回归用例:v2.0.0 的 `&s[..40]` 在此输入上 panic(32 个汉字 = 96 字节,
    /// 字节 40 落在第 14 个汉字中间,非 UTF-8 字符边界)。
    /// 字符数不足 n 时按实际长度返回(32 < 40,不截断也不补齐)。
    #[test]
    fn truncate_chars_multibyte_never_panics() {
        let title = "生成的彩条图比真实视频帧好认真实标题测试数据一二三四五六七八九十";
        let preview = truncate_chars(title, 40);
        assert_eq!(preview.chars().count(), 32);
        assert_eq!(preview, title, "不足 n 个字符时原样返回");
    }

    /// 字符数超过 n 时截到恰好 n 个字符(>14 字纯中文,触发 v2.0.0 崩溃的长度区间)。
    #[test]
    fn truncate_chars_truncates_to_n_chars() {
        let title = "拾贴".repeat(25); // 50 个汉字 = 150 字节
        let preview = truncate_chars(&title, 40);
        assert_eq!(preview.chars().count(), 40);
        assert!(title.starts_with(&preview), "截断结果应是原串前缀");
    }

    #[test]
    fn truncate_chars_edge_cases() {
        // 短串原样返回(不足 n 个字符,不补不报错)
        assert_eq!(truncate_chars("短标题", 40), "短标题");
        assert_eq!(truncate_chars("", 40), "");
        // n=0
        assert_eq!(truncate_chars("任意", 0), "");
        // 4 字节 emoji 切边界同样安全
        assert_eq!(truncate_chars("🎉🎉🎉🎉", 2), "🎉🎉");
        // ASCII 与中文按同等字符数计数(真实标题常见形态)
        assert_eq!(truncate_chars("abc拾贴拾贴", 4), "abc拾");
    }

    // ===== update_last_copied_at 测试 =====

    #[test]
    fn update_last_copied_at_moves_item_to_front() {
        // copy_back 后 UPDATE last_copied_at → 列表排序变化
        let conn = db_with_sample_data();
        let dir = test_images_dir();

        // 排序应该是:第三条(最新) / 第二条 / 第一条
        let before = list_items_with_conn(&conn, 50, &dir, None).unwrap();
        assert_eq!(before[0].title, "第三条");

        // 回填第一条(最旧的)→ 更新 last_copied_at → 应排到最前
        let first_id = before[2].id; // 第一条
        update_last_copied_at(&conn, first_id).unwrap();

        let after = list_items_with_conn(&conn, 50, &dir, None).unwrap();
        assert_eq!(after[0].title, "第一条", "回填后应排到最前");
    }

    #[test]
    fn update_last_copied_at_does_not_change_copy_count() {
        // copy_count 不应因回填变化(回填不是新复制)
        let conn = db_with_sample_data();
        let dir = test_images_dir();
        let before = list_items_with_conn(&conn, 50, &dir, None).unwrap();

        let target = &before[2]; // 第一条,copy_count=1
        assert_eq!(target.copy_count, 1);
        let target_id = target.id;

        update_last_copied_at(&conn, target_id).unwrap();

        let after = list_items_with_conn(&conn, 50, &dir, None).unwrap();
        let updated = after.iter().find(|i| i.id == target_id).unwrap();
        assert_eq!(updated.copy_count, 1, "copy_count 不应变(回填不加 count)");
    }
}
