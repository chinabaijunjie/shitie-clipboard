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
use crate::models::{ListPage, StorageStats};
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

/// 搜索剪贴板历史(分页)。中文/混合查询走 FTS5 + bigram;纯 ASCII 查询走拼音直搜合并(FTS + 三路 LIKE)。
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
#[path = "clipboard_tests.rs"]
mod tests;
