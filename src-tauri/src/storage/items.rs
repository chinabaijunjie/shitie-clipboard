//! items 表 CRUD — 剪贴板历史条目的增删查
//!
//! M1 只处理纯文本(uti='public.utf8-plain-text')。每次复制:
//! - 去重:formats 表里已有相同文本 → UPDATE items(last_copied_at + copy_count)
//! - 新增:INSERT items + INSERT formats(一行 text)
//!
//! M2 扩展:`insert_or_update_multi` 支持一次写入多格式(RTF/HTML/图片等),
//! content_kind 分类,并写 search_text(FTS 索引用)。M1 的单文本入口保留(handler 仍在用)。
//!
//! 去重用 data_blob 内容比对(M1 数据量小,全表扫可接受;hash 索引优化经 M4 评估后
//! 决定不做——万条内无感,见 wiki/dev/m4-research.md §3.3 G3)。

use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

use crate::clipboard::classify::{detect_subtype, ContentSubtype};
use crate::clipboard::types::{AbsPath, RelPath};
use crate::error::AppError;
use crate::models::{truncate_title, ClipboardItem, ContentKind, StorageByKind};
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

/// 插入或更新剪贴板条目(去重,纯文本入口)。
///
/// M1 单文本入口,生产路径已改走 `insert_or_update_multi`(handler 用 multi)。
/// 保留供测试/向后兼容。
#[allow(dead_code)]
///
/// 返回最终的 ClipboardItem(新增或更新后的)。调用方据此 emit 前端。
///
/// 去重逻辑:formats 表里查找 uti=PLAIN_TEXT 且 data_blob == text 的 item。
/// - 找到:UPDATE 该 item 的 last_copied_at + copy_count
/// - 没找到:INSERT items(含 search_text)+ INSERT formats(text)
pub fn insert_or_update(
    conn: &Connection,
    text: &str,
    images_dir: &Path,
) -> Result<ClipboardItem, AppError> {
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
            let subtype = detect_subtype(text);
            let subtype_str = if subtype.is_some() { Some(subtype.as_db_str()) } else { None };
            conn.execute(
                "INSERT INTO clipboard_items \
                 (first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype, search_text) \
                 VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![now, now, title, ContentKind::Text.as_db_str(), subtype_str, &search_text],
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

    get_item(conn, item_id, images_dir)
}

/// 去重的判定 key(纯函数,可测)。
///
/// 决策规则(优先级):
/// - 有纯文本格式 → 按文本内容去重(富文本/纯文本场景)
/// - 无纯文本但有图片 → 按图片 file_path 去重(同一张图 sha256 相同,file_path 相同)
/// - 无文本无图片但有文件 URL → 按 URL 字节去重
/// - 都没有 → 不去重(无法判定唯一性)
#[derive(Debug, Clone, PartialEq)]
pub enum DedupKey<'a> {
    /// 按纯文本字节去重。
    Text(&'a [u8]),
    /// 按图片 file_path 去重(相对路径)。
    Image(&'a str),
    /// 按文件 URL 字节去重(public.file-url 的 data)。
    File(&'a [u8]),
}

/// 从 formats 推出去重 key(纯函数,可测)。
pub fn dedup_key<'a>(formats: &'a [FormatInput<'_>]) -> Option<DedupKey<'a>> {
    // 优先纯文本
    if let Some(f) = formats.iter().find(|f| f.uti == UTI_PLAIN_TEXT) {
        return Some(DedupKey::Text(f.data));
    }
    // 其次图片(file_path,sha256 去重)
    if let Some(f) = formats
        .iter()
        .find(|f| f.uti == crate::clipboard::uti::UTI_PNG || f.uti == crate::clipboard::uti::UTI_TIFF)
    {
        if let Some(fp) = f.file_path {
            return Some(DedupKey::Image(fp));
        }
    }
    // 再次文件 URL(按 URL 字节去重)
    if let Some(f) = formats
        .iter()
        .find(|f| f.uti == crate::clipboard::uti::UTI_FILE_URL)
    {
        return Some(DedupKey::File(f.data));
    }
    None
}

/// 插入或更新剪贴板条目(多格式入口,M2 捕获层用)。
///
/// 与 `insert_or_update`(单文本)的区别:
/// - 支持一次写多个 formats(RTF/HTML/text/image/file-url 等)
/// - content_kind 由调用方根据捕获格式分类
/// - 标题由调用方生成(富文本提取纯文本 / 图片尺寸 / 文件名)
///
/// 去重(见 `dedup_key`):
/// - 有纯文本 → 按文本内容去重(富文本/纯文本)
/// - 无纯文本但有图片 → 按图片 file_path 去重(同一张图不重复入库)
/// - 无文本无图片但有文件 URL → 按 URL 去重
/// - 都没有 → 不去重
///
/// 命中去重时,formats 表会整体替换(DELETE 旧行 + INSERT 新行),确保最新格式生效
/// (M4 G1 修复:A(带RTF)→B→A(纯文本) 后不再残留旧 RTF)。
///
/// 返回最终的 ClipboardItem。
pub fn insert_or_update_multi(
    conn: &Connection,
    formats: &[FormatInput<'_>],
    kind: ContentKind,
    title: &str,
    images_dir: &Path,
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

    // 去重:按 dedup_key 查已有 item
    let existing_id: Option<i64> = match dedup_key(formats) {
        Some(DedupKey::Text(bytes)) => conn
            .query_row(
                "SELECT item_id FROM clipboard_formats \
                 WHERE uti = ?1 AND data_blob = ?2 LIMIT 1",
                params![UTI_PLAIN_TEXT, bytes],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::internal(format!("文本去重查询失败: {e}")))?,
        Some(DedupKey::Image(file_path)) => conn
            .query_row(
                "SELECT item_id FROM clipboard_formats \
                 WHERE uti IN ('public.png','public.tiff') AND file_path = ?1 LIMIT 1",
                params![file_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::internal(format!("图片去重查询失败: {e}")))?,
        Some(DedupKey::File(url_bytes)) => conn
            .query_row(
                "SELECT item_id FROM clipboard_formats \
                 WHERE uti = ?1 AND data_blob = ?2 LIMIT 1",
                params![crate::clipboard::uti::UTI_FILE_URL, url_bytes],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::internal(format!("文件 URL 去重查询失败: {e}")))?,
        None => None,
    };

    // 整个写入包进事务:命中替换 formats 或新增都需原子性(M4 G1)
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::internal(format!("开启写入事务失败: {e}")))?;

    let item_id = match existing_id {
        Some(id) => {
            // 命中去重:更新时间 + 次数 + 替换 formats(G1 修复)
            tx.execute(
                "UPDATE clipboard_items SET last_copied_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| AppError::internal(format!("更新 copy_count 失败: {e}")))?;

            // G1 修复:替换 formats 表(DELETE 旧行 + INSERT 新行)
            // 确保最新格式生效(如富文本→纯文本切换时旧 RTF 不残留)
            replace_formats(&tx, id, formats, images_dir)?;

            id
        }
        None => {
            // 内容子类型识别:只对 text/rich 类型识别 link/markdown
            let subtype = match kind {
                ContentKind::Text | ContentKind::Rich => {
                    match plain_text {
                        Some(bytes) => {
                            let text = String::from_utf8_lossy(bytes);
                            detect_subtype(&text)
                        }
                        None => ContentSubtype::None,
                    }
                }
                _ => ContentSubtype::None,
            };
            let subtype_str = if subtype.is_some() { Some(subtype.as_db_str()) } else { None };

            tx.execute(
                "INSERT INTO clipboard_items \
                 (first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype, search_text) \
                 VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![now, now, &title, kind.as_db_str(), subtype_str, &search_text],
            )
            .map_err(|e| AppError::internal(format!("INSERT items 失败: {e}")))?;
            let new_id = tx.last_insert_rowid();

            // INSERT 所有 formats 行(失败则事务回滚)
            insert_formats(&tx, new_id, formats)?;

            new_id
        }
    };

    tx.commit()
        .map_err(|e| AppError::internal(format!("写入事务 commit 失败: {e}")))?;

    get_item(conn, item_id, images_dir)
}

/// 按 id 查单条(返回 ClipboardItem 元数据视图)。
///
/// `images_dir`:图片存储根目录,用于把缩略图相对路径转绝对路径。
pub fn get_item(conn: &Connection, id: i64, images_dir: &Path) -> Result<ClipboardItem, AppError> {
    let mut item = conn
        .query_row(
            "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype \
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
    fill_image_fields(conn, &mut item, images_dir)?;
    fill_file_url(conn, &mut item)?;
    Ok(item)
}

/// 查列表(按 last_copied_at DESC)。
///
/// `kinds`:可选的类型过滤(只返回这些 content_kind 的记录)。None 或空 = 不过滤。
/// `images_dir`:图片存储根目录,用于把缩略图相对路径转绝对路径。
pub fn list(
    conn: &Connection,
    limit: i64,
    images_dir: &Path,
    kinds: Option<&[String]>,
) -> Result<Vec<ClipboardItem>, AppError> {
    match kinds {
        Some(ks) if !ks.is_empty() => {
            // 拆分筛选值:普通值走 content_kind,'subtype:xxx' 走 content_subtype
            let kind_filters: Vec<&String> = ks.iter().filter(|k| !k.starts_with("subtype:")).collect();
            let subtype_filters: Vec<String> = ks.iter()
                .filter_map(|k| k.strip_prefix("subtype:").map(|s| s.to_string()))
                .collect();

            // 动态构建 WHERE 子句(各条件 OR 组合):
            // - content_kind 筛选:匹配 kind 且无子类型(纯文字不含链接/Markdown)
            // - content_subtype 筛选:直接匹配子类型
            let mut clauses = Vec::new();
            let mut param_values: Vec<rusqlite::types::Value> = Vec::new();

            if !kind_filters.is_empty() {
                let placeholders = (0..kind_filters.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
                // 排除有子类型的记录(link/markdown 有独立筛选条件)
                clauses.push(format!("(content_kind IN ({placeholders}) AND content_subtype IS NULL)"));
                for k in &kind_filters {
                    param_values.push(rusqlite::types::Value::Text((**k).clone()));
                }
            }
            if !subtype_filters.is_empty() {
                let placeholders = (0..subtype_filters.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
                clauses.push(format!("content_subtype IN ({placeholders})"));
                for s in &subtype_filters {
                    param_values.push(rusqlite::types::Value::Text(s.clone()));
                }
            }

            let where_clause = clauses.join(" OR ");
            let sql = format!(
                "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype \
                 FROM clipboard_items WHERE {where_clause} ORDER BY last_copied_at DESC LIMIT ?"
            );
            param_values.push(rusqlite::types::Value::from(limit));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| AppError::internal(format!("prepare list(filtered) 失败: {e}")))?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(param_values.iter()), row_to_item)
                .map_err(|e| AppError::internal(format!("query list(filtered) 失败: {e}")))?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| AppError::internal(format!("读 list 行失败: {e}")))?);
            }
            for item in &mut result {
                fill_image_fields(conn, item, images_dir)?;
                fill_file_url(conn, item)?;
            }
            Ok(result)
        }
        _ => {
            // 无过滤:原始查询
            let mut stmt = conn
                .prepare(
                    "SELECT id, first_copied_at, last_copied_at, copy_count, title, content_kind, content_subtype \
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
            for item in &mut result {
                fill_image_fields(conn, item, images_dir)?;
                fill_file_url(conn, item)?;
            }
            Ok(result)
        }
    }
}

/// 读某条的纯文本内容。
///
/// M1 的 copy_back 用此读纯文本;M2 copy_back 改用 `get_formats` 读多格式。
/// 保留供测试/向后兼容。
#[allow(dead_code)]
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
///
/// `file_path` 为 [`RelPath`]:图片格式的磁盘相对路径;文本类格式为 None。
pub type FormatRow = (String, Option<Vec<u8>>, Option<RelPath>);

/// 读某条的所有格式行(给 M2 多格式回填用)。
///
/// 返回每个格式的 (uti, data_blob, file_path)。
/// file_path 从 DB 读出的裸 String 包装成 [`RelPath`]。
pub fn get_formats(conn: &Connection, id: i64) -> Result<Vec<FormatRow>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT uti, data_blob, file_path FROM clipboard_formats WHERE item_id = ?1",
        )
        .map_err(|e| AppError::internal(format!("prepare get_formats 失败: {e}")))?;

    let rows = stmt
        .query_map(params![id], |r| {
            let file_path: Option<String> = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<Vec<u8>>>(1)?,
                file_path.map(RelPath::new),
            ))
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
    let image_paths: Vec<RelPath> = conn
        .prepare("SELECT file_path FROM clipboard_formats WHERE item_id = ?1 AND file_path IS NOT NULL")
        .map_err(|e| AppError::internal(format!("prepare 查图片 file_path 失败: {e}")))?
        .query_map(params![id], |r| {
            let s: String = r.get(0)?;
            Ok(RelPath::new(s))
        })
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

/// 按 contentKind + content_subtype 分组统计条数。
///
/// 返回 HashMap<(kind, subtype), i64>(key = ("text", Some("link")) / ("image", None) 等)。
pub fn count_by_kind(conn: &Connection) -> Result<HashMap<(String, Option<String>), i64>, AppError> {
    let mut map = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT content_kind, content_subtype, COUNT(*) FROM clipboard_items GROUP BY content_kind, content_subtype")
        .map_err(|e| AppError::internal(format!("prepare count_by_kind 失败: {e}")))?;
    let rows = stmt
        .query_map([], |r| {
            let kind: String = r.get(0)?;
            let subtype: Option<String> = r.get(1)?;
            let count: i64 = r.get(2)?;
            Ok((kind, subtype, count))
        })
        .map_err(|e| AppError::internal(format!("query count_by_kind 失败: {e}")))?;
    for row in rows.filter_map(Result::ok) {
        map.insert((row.0, row.1), row.2);
    }
    Ok(map)
}

/// 把 count_by_kind 的 HashMap 转成 StorageByKind。
///
/// link / markdown 从 text+rich 里拆出来(subtype='link'/'markdown' 的不计入 text/rich)。
pub fn storage_by_kind_from_map(map: &HashMap<(String, Option<String>), i64>) -> StorageByKind {
    let get = |kind: &str, subtype: Option<&str>| -> i64 {
        *map.get(&(kind.to_string(), subtype.map(|s| s.to_string()))).unwrap_or(&0)
    };

    StorageByKind {
        // text:无 subtype 或 subtype=None 的 text 类型
        text: get("text", None),
        rich: get("rich", None),
        image: get("image", None),
        file: get("file", None),
        mixed: get("mixed", None),
        // 有 subtype 的(text/rich 里的 link/markdown)
        link: get("text", Some("link")) + get("rich", Some("link")),
        markdown: get("text", Some("markdown")) + get("rich", Some("markdown")),
    }
}

/// 清空所有历史(DB 行 + 图片文件)。
///
/// 先逐条引用计数删除图片,再 `DELETE FROM clipboard_items`(CASCADE 清 formats)。
/// 包 unchecked_transaction,中途失败回滚。
/// 返回删除的条目数。
pub fn delete_all(conn: &Connection, images_dir: &Path) -> Result<usize, AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::internal(format!("开启清空事务失败: {e}")))?;

    // 查所有图片 file_path → 逐个引用计数删除磁盘文件
    let image_paths: Vec<RelPath> = tx
        .prepare("SELECT DISTINCT file_path FROM clipboard_formats WHERE file_path IS NOT NULL")
        .map_err(|e| AppError::internal(format!("prepare 查图片路径失败: {e}")))?
        .query_map([], |r| {
            let s: String = r.get(0)?;
            Ok(RelPath::new(s))
        })
        .map_err(|e| AppError::internal(format!("查图片路径失败: {e}")))?
        .filter_map(Result::ok)
        .collect();

    for fp in &image_paths {
        // 引用计数删除(此时 DB 行还在,每张图至少引用 ≥ 1)
        crate::clipboard::image_store::delete_if_last_reference(&tx, fp, images_dir)?;
    }

    // 统计条数(删之前)
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
        .map_err(|e| AppError::internal(format!("统计条数失败: {e}")))?;

    // 清空 DB(CASCADE 自动清 formats)
    tx.execute("DELETE FROM clipboard_items", [])
        .map_err(|e| AppError::internal(format!("清空 clipboard_items 失败: {e}")))?;

    tx.commit()
        .map_err(|e| AppError::internal(format!("提交清空事务失败: {e}")))?;

    log::info!("[storage] 清空历史:删除了 {} 条记录, {} 个图片文件", count, image_paths.len());
    Ok(count as usize)
}

// ============================================================
// 内部辅助
// ============================================================

/// 向 clipboard_formats 表插入多行(M4 抽出,供 insert_or_update_multi 复用)。
///
/// 返回 Result:任一 INSERT 失败即返回错误,调用方据此回滚事务。
fn insert_formats(
    conn: &Connection,
    item_id: i64,
    formats: &[FormatInput<'_>],
) -> Result<(), AppError> {
    for fmt in formats {
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, data_blob, file_path, size) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item_id,
                fmt.uti,
                fmt.data,
                fmt.file_path,
                fmt.data.len() as i64,
            ],
        )
        .map_err(|e| AppError::internal(format!("INSERT formats ({}) 失败: {e}", fmt.uti)))?;
    }
    Ok(())
}

/// 替换某 item 的所有 formats 行(G1 修复:命中去重时用新格式覆盖旧格式)。
///
/// 步骤:
/// 1. 查旧 formats 里的图片 file_path,对新 formats 中不再出现的做引用计数删除
/// 2. DELETE 旧 formats 行
/// 3. INSERT 新 formats 行
///
/// 图片引用计数安全:去重命中通常新旧图片引用一致(同 file_path),
/// 若新 formats 丢弃了旧图片引用,需删除磁盘文件防止泄露。
fn replace_formats(
    conn: &Connection,
    item_id: i64,
    new_formats: &[FormatInput<'_>],
    images_dir: &Path,
) -> Result<(), AppError> {
    // 收集新 formats 中的图片 file_path(仍在引用的图片)
    let new_image_paths: std::collections::HashSet<&str> = new_formats
        .iter()
        .filter(|f| {
            f.uti == crate::clipboard::uti::UTI_PNG || f.uti == crate::clipboard::uti::UTI_TIFF
        })
        .filter_map(|f| f.file_path)
        .collect();

    // 查旧 formats 里的图片 file_path
    let old_image_paths: Vec<RelPath> = conn
        .prepare(
            "SELECT file_path FROM clipboard_formats \
             WHERE item_id = ?1 AND file_path IS NOT NULL \
             AND uti IN ('public.png','public.tiff')",
        )
        .map_err(|e| AppError::internal(format!("prepare 查旧图片 file_path 失败: {e}")))?
        .query_map(params![item_id], |r| {
            let s: String = r.get(0)?;
            Ok(RelPath::new(s))
        })
        .map_err(|e| AppError::internal(format!("查旧图片 file_path 失败: {e}")))?
        .filter_map(Result::ok)
        .collect();

    // 对新 formats 中不再引用的旧图片做引用计数删除
    for old_fp in &old_image_paths {
        if !new_image_paths.contains(old_fp.as_str()) {
            crate::clipboard::image_store::delete_if_last_reference(conn, old_fp, images_dir)?;
        }
    }

    // DELETE 旧 formats 行
    conn.execute(
        "DELETE FROM clipboard_formats WHERE item_id = ?1",
        params![item_id],
    )
    .map_err(|e| AppError::internal(format!("DELETE 旧 formats 失败: {e}")))?;

    // INSERT 新 formats 行
    for fmt in new_formats {
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, data_blob, file_path, size) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item_id,
                fmt.uti,
                fmt.data,
                fmt.file_path,
                fmt.data.len() as i64,
            ],
        )
        .map_err(|e| AppError::internal(format!("INSERT formats ({}) 失败: {e}", fmt.uti)))?;
    }

    Ok(())
}

/// 当前 unix epoch 毫秒。
pub(crate) fn now_epoch_ms() -> i64 {
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
        content_subtype: row.get::<_, Option<String>>(6)?,
        image_path: None,
        file_url: None,
        thumb_path: None,
    })
}

/// 给图片类 item 补充缩略图路径(从 formats 表查 file_path 推导)。
///
/// 非 image/mixed 类型不处理。图片格式(public.png/public.tiff)的 file_path 存的是
/// 原图相对路径,缩略图路径由 image_store::relative_thumb_path_from_image 推导。
/// 推导出相对路径后,用 `images_dir` 转成绝对路径填入 `ClipboardItem.thumb_path`。
///
/// `pub(crate)`:list/get_item(本文件)+ search.rs(搜索结果)都要调用,
/// 保证所有返回前端的路径都填了 thumb_path(否则搜索命中的图片无缩略图)。
pub(crate) fn fill_image_fields(
    conn: &Connection,
    item: &mut ClipboardItem,
    images_dir: &Path,
) -> Result<(), AppError> {
    if !matches!(item.content_kind, ContentKind::Image | ContentKind::Mixed) {
        return Ok(());
    }
    // 查图片格式的 file_path(相对路径)
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
        let image_rel = RelPath::new(fp);
        let thumb_rel = crate::clipboard::image_store::relative_thumb_path_from_image_pub(&image_rel);
        // 缩略图相对路径 → 绝对路径(给前端 convertFileSrc)
        item.thumb_path =
            crate::clipboard::image_store::resolve_thumb_absolute(&Some(thumb_rel), images_dir);
        // 原图相对路径 → 绝对路径(给 UI 显示存储位置)
        item.image_path = Some(AbsPath::new(
            images_dir.join(image_rel.as_str()).to_string_lossy().to_string(),
        ));
    }
    Ok(())
}

/// 给文件类型 item 补充 file_url(从 formats 表读 public.file-url 的 data_blob)。
///
/// 仅 file/mixed 类型处理。data_blob 是原始文件路径(可能多个用 \n 拼接)。
pub(crate) fn fill_file_url(conn: &Connection, item: &mut ClipboardItem) -> Result<(), AppError> {
    if !matches!(item.content_kind, ContentKind::File | ContentKind::Mixed) {
        return Ok(());
    }
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT data_blob FROM clipboard_formats \
             WHERE item_id = ?1 AND uti = ?2 AND data_blob IS NOT NULL \
             LIMIT 1",
            params![item.id, crate::clipboard::uti::UTI_FILE_URL],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::internal(format!("查 file-url 失败: {e}")))?;
    if let Some(bytes) = blob {
        item.file_url = Some(String::from_utf8_lossy(&bytes).to_string());
    }
    Ok(())
}
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
        let item = insert_or_update(&conn, "第一条", &test_images_dir()).expect("插入应成功");
        assert_eq!(item.title, "第一条");
        assert_eq!(item.copy_count, 1);
        assert_eq!(item.content_kind, ContentKind::Text);
    }

    #[test]
    fn duplicate_updates_count() {
        let conn = db();
        let first = insert_or_update(&conn, "重复内容", &test_images_dir()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second = insert_or_update(&conn, "重复内容", &test_images_dir()).unwrap();

        assert_eq!(first.id, second.id, "相同文本应命中同一条");
        assert_eq!(second.copy_count, 2);
        assert!(second.last_copied_at >= first.last_copied_at);
    }

    #[test]
    fn different_text_creates_separate_items() {
        let conn = db();
        let a = insert_or_update(&conn, "AAA", &test_images_dir()).unwrap();
        let b = insert_or_update(&conn, "BBB", &test_images_dir()).unwrap();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn list_orders_by_last_copied_desc() {
        let conn = db();
        insert_or_update(&conn, "旧", &test_images_dir()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        insert_or_update(&conn, "新", &test_images_dir()).unwrap();

        let list = list(&conn, 10, &test_images_dir(), None).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "新", "最新复制应排第一");
    }

    #[test]
    fn get_text_returns_stored_content() {
        let conn = db();
        let item = insert_or_update(&conn, "要回填的内容", &test_images_dir()).unwrap();
        let text = get_text(&conn, item.id).unwrap();
        assert_eq!(text, "要回填的内容");
    }

    #[test]
    fn delete_removes_item_and_cascades_formats() {
        let conn = db();
        let item = insert_or_update(&conn, "待删除", &test_images_dir()).unwrap();

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
        let err = get_item(&conn, 9999, &test_images_dir()).unwrap_err();
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
        let item = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "Hello world", &test_images_dir())
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
        let item = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "你好世界", &test_images_dir()).unwrap();

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
        let first = insert_or_update_multi(&conn, &formats, ContentKind::Text, "重复内容", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::Text, "重复内容", &test_images_dir()).unwrap();

        assert_eq!(first.id, second.id, "相同纯文本应命中同一条");
        assert_eq!(second.copy_count, 2);
    }

    #[test]
    fn insert_multi_image_dedup_by_file_path() {
        // 纯图片(无纯文本格式)按 file_path 去重:同一张图(同 sha256 → 同 file_path)不重复入库
        let conn = db();
        let formats = vec![
            FormatInput { uti: "public.png", data: &[], file_path: Some("ab/1234.png") },
        ];
        let first = insert_or_update_multi(&conn, &formats, ContentKind::Image, "图片 100×100", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::Image, "图片 100×100", &test_images_dir()).unwrap();

        assert_eq!(first.id, second.id, "相同图片(file_path 相同)应去重命中同一条");
        assert_eq!(second.copy_count, 2);
    }

    #[test]
    fn insert_multi_different_images_no_dedup() {
        // 不同图片(file_path 不同)不去重
        let conn = db();
        let f1 = vec![FormatInput { uti: "public.png", data: &[], file_path: Some("ab/1111.png") }];
        let f2 = vec![FormatInput { uti: "public.png", data: &[], file_path: Some("cd/2222.png") }];
        let first = insert_or_update_multi(&conn, &f1, ContentKind::Image, "图1", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &f2, ContentKind::Image, "图2", &test_images_dir()).unwrap();
        assert_ne!(first.id, second.id, "不同图片应各自入库");
    }

    #[test]
    fn dedup_key_prefers_text() {
        // 有纯文本 → 按文本去重(即使还有图片)
        let text = b"hello".to_vec();
        let formats = vec![
            FormatInput { uti: UTI_PLAIN_TEXT, data: &text, file_path: None },
            FormatInput { uti: "public.png", data: &[], file_path: Some("ab/x.png") },
        ];
        assert_eq!(dedup_key(&formats), Some(DedupKey::Text(&text)));
    }

    #[test]
    fn dedup_key_image_when_no_text() {
        // 无纯文本 + 有图片 → 按图片 file_path
        let formats = vec![
            FormatInput { uti: "public.png", data: &[], file_path: Some("ab/x.png") },
        ];
        assert_eq!(dedup_key(&formats), Some(DedupKey::Image("ab/x.png")));
    }

    #[test]
    fn dedup_key_none_when_unknown_uti() {
        // 非白名单 UTI(无文本无图片无文件URL)→ 不去重
        let data = b"random".to_vec();
        let formats = vec![
            FormatInput { uti: "dyn.some-random", data: &data, file_path: None },
        ];
        assert_eq!(dedup_key(&formats), None);
    }

    #[test]
    fn dedup_key_file_url_when_no_text_no_image() {
        // 无文本无图片 + 有文件 URL → 按文件 URL 去重
        let url = b"file:///Users/test/report.pdf".to_vec();
        let formats = vec![
            FormatInput { uti: "public.file-url", data: &url, file_path: None },
        ];
        assert_eq!(dedup_key(&formats), Some(DedupKey::File(&url)));
    }

    #[test]
    fn insert_multi_dedup_by_file_url() {
        // 文件 URL 去重:相同 URL 复制两次 → 命中同一 item
        let conn = db();
        let url = b"file:///Users/test/report.pdf".to_vec();
        let formats = vec![
            FormatInput { uti: "public.file-url", data: &url, file_path: None },
        ];
        let first = insert_or_update_multi(&conn, &formats, ContentKind::File, "report.pdf", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::File, "report.pdf", &test_images_dir()).unwrap();

        assert_eq!(first.id, second.id, "相同文件 URL 应命中同一条");
        assert_eq!(second.copy_count, 2);
    }

    #[test]
    fn insert_multi_different_file_urls_no_dedup() {
        // 不同文件 URL 不去重
        let conn = db();
        let url1 = b"file:///a.pdf".to_vec();
        let url2 = b"file:///b.pdf".to_vec();
        let f1 = vec![FormatInput { uti: "public.file-url", data: &url1, file_path: None }];
        let f2 = vec![FormatInput { uti: "public.file-url", data: &url2, file_path: None }];
        let first = insert_or_update_multi(&conn, &f1, ContentKind::File, "a.pdf", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &f2, ContentKind::File, "b.pdf", &test_images_dir()).unwrap();
        assert_ne!(first.id, second.id, "不同文件 URL 应各自入库");
    }

    #[test]
    fn dedup_replaces_formats_on_hit() {
        // G1 修复:A(带 RTF)→B→A(纯文本) 后,旧 RTF 不残留
        let conn = db();
        let text = b"hello".to_vec();
        let rtf = b"{\\rtf1 hello}".to_vec();

        // 第一次:text + rtf
        let formats_with_rtf = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
            FormatInput { uti: "public.rtf", data: &rtf, file_path: None },
        ];
        let first = insert_or_update_multi(&conn, &formats_with_rtf, ContentKind::Rich, "hello", &test_images_dir()).unwrap();

        // 确认第一次有 2 个 formats
        let fmt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_formats WHERE item_id = ?1",
                params![first.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fmt_count, 2, "首次插入应有 2 个格式行");

        // 第二次:相同文本但只有纯文本(模拟从记事本复制)
        let formats_text_only = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
        ];
        let second = insert_or_update_multi(&conn, &formats_text_only, ContentKind::Text, "hello", &test_images_dir()).unwrap();

        // 应命中同一条
        assert_eq!(first.id, second.id, "相同文本应命中同一条");
        assert_eq!(second.copy_count, 2);

        // formats 应只有 1 行(纯文本),RTF 已被替换掉
        let read_back = get_formats(&conn, second.id).unwrap();
        assert_eq!(read_back.len(), 1, "G1:旧 RTF 应被替换,只剩纯文本");
        assert_eq!(read_back[0].0, "public.utf8-plain-text");
    }

    #[test]
    fn dedup_replaces_formats_keeps_same_formats() {
        // G1 边界:命中去重时新旧 formats 相同 → 替换后格式不变
        let conn = db();
        let text = b"hello".to_vec();
        let rtf = b"{\\rtf1 hello}".to_vec();
        let formats = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
            FormatInput { uti: "public.rtf", data: &rtf, file_path: None },
        ];

        let first = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "hello", &test_images_dir()).unwrap();
        let second = insert_or_update_multi(&conn, &formats, ContentKind::Rich, "hello", &test_images_dir()).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.copy_count, 2);

        // formats 应仍然是 2 行(相同内容替换)
        let read_back = get_formats(&conn, second.id).unwrap();
        assert_eq!(read_back.len(), 2, "相同 formats 替换后行数不变");
    }

    #[test]
    fn dedup_replaces_formats_drops_old_image() {
        // G1 + 图片引用计数:图片 item 命中去重,新 formats 无图片 → 旧图片磁盘文件应删
        let conn = db();
        let images_dir = test_images_dir();
        let png = test_png_bytes();

        // 存图
        let meta = crate::clipboard::image_store::save_image(&png, &images_dir).unwrap();
        let orig_path = images_dir.join(meta.rel_path.as_str());
        assert!(orig_path.exists(), "原图应存在");

        // 第一次:文本 + 图片(mixed)
        let text = b"screenshot".to_vec();
        let rel_path_str = meta.rel_path.as_str().to_string();
        let formats_with_image = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
            FormatInput { uti: crate::clipboard::uti::UTI_PNG, data: &[], file_path: Some(&rel_path_str) },
        ];
        let first = insert_or_update_multi(
            &conn, &formats_with_image, ContentKind::Mixed, "screenshot", &images_dir,
        ).unwrap();

        // 确认引用数为 1
        assert_eq!(
            crate::clipboard::image_store::count_references(&conn, &meta.rel_path).unwrap(),
            1
        );

        // 第二次:相同文本但只有纯文本(图片被丢弃)
        let formats_text_only = vec![
            FormatInput { uti: "public.utf8-plain-text", data: &text, file_path: None },
        ];
        let second = insert_or_update_multi(
            &conn, &formats_text_only, ContentKind::Text, "screenshot", &images_dir,
        ).unwrap();

        // 命中同一条
        assert_eq!(first.id, second.id);
        assert_eq!(second.copy_count, 2);

        // formats 应只有 1 行(纯文本),PNG 已被替换掉
        let read_back = get_formats(&conn, second.id).unwrap();
        assert_eq!(read_back.len(), 1, "图片 format 应被替换掉");
        assert_eq!(read_back[0].0, "public.utf8-plain-text");

        // 旧图片磁盘文件应被删(引用计数归 0)
        assert!(!orig_path.exists(), "旧图片应被引用计数删除");
    }

    #[test]
    fn single_text_insert_indexes_fts() {
        // M1 单文本入口(insert_or_update)也应写 search_text + 索引 FTS
        let conn = db();
        let item = insert_or_update(&conn, "搜索测试文本", &test_images_dir()).unwrap();

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

    // ===== PR review 补:图片引用计数 delete 端到端 =====

    /// 测试用 PNG(2x2 红色,通过 image crate 生成,可靠)。
    fn test_png_bytes() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2, 2, |_, _| Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn delete_image_end_to_end_reference_counting() {
        // 端到端:两张图共享同一 file_path(去重命中)→ 删一个保留磁盘,删第二个清盘。
        // test-analyzer 评 8/10 的关键缺口:items::delete 有图片时从未测过(只测纯文本)。
        let conn = db();
        let images_dir = test_images_dir();
        let png = test_png_bytes();

        // 存图(sha256 决定 file_path)
        let meta = crate::clipboard::image_store::save_image(&png, &images_dir).unwrap();
        let orig_path = images_dir.join(meta.rel_path.as_str());
        assert!(orig_path.exists(), "原图应存在");

        // 两个 item 都引用同一图片 file_path(模拟同一张图被两次复制)
        let rel_path_str = meta.rel_path.as_str();
        let formats = vec![FormatInput {
            uti: crate::clipboard::uti::UTI_PNG,
            data: &[],
            file_path: Some(rel_path_str),
        }];
        let item1 = insert_or_update_multi(&conn, &formats, ContentKind::Image, "图1", &images_dir).unwrap();
        // 第二个 item:insert_or_update_multi 会按 Image file_path 去重命中 item1,
        // 所以直接插第二个真实 item + formats 行(模拟两条历史引用同一张图)。
        let item2_id: i64 = conn
            .query_row(
                "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) \
                 VALUES (2,2,'图2','image') RETURNING id",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, file_path, size) VALUES (?1,'public.png',?2,0)",
            params![item2_id, meta.rel_path.as_str()],
        )
        .unwrap();

        // 引用数应为 2
        assert_eq!(
            crate::clipboard::image_store::count_references(&conn, &meta.rel_path).unwrap(),
            2
        );

        // 删 item1:引用数 2 → 1,磁盘文件保留
        delete(&conn, item1.id, &images_dir).unwrap();
        assert!(orig_path.exists(), "引用数仍 1,磁盘文件应保留");

        // 删 item2:引用数 1 → 0,磁盘文件清除
        delete(&conn, item2_id, &images_dir).unwrap();
        assert!(!orig_path.exists(), "引用数归 0,磁盘文件应清除");
    }

    #[test]
    fn delete_image_missing_disk_file_is_ok() {
        // 磁盘文件已不存在(孤儿/手动删)时,delete 不应报错(NotFound 豁免)
        let conn = db();
        let images_dir = test_images_dir();
        // 插一个指向不存在文件的图片 item
        let formats = vec![FormatInput {
            uti: crate::clipboard::uti::UTI_PNG,
            data: &[],
            file_path: Some("ab/nonexistent.png"),
        }];
        let item = insert_or_update_multi(&conn, &formats, ContentKind::Image, "孤儿图", &images_dir).unwrap();
        // delete 应成功(磁盘文件不存在不算错误)
        delete(&conn, item.id, &images_dir).unwrap();
    }

    // ===== count_by_kind / delete_all 测试 =====

    #[test]
    fn count_by_kind_groups_correctly() {
        let conn = db();
        let dir = test_images_dir();

        // 插入 2 条纯文本
        insert_or_update(&conn, "text1", &dir).unwrap();
        insert_or_update(&conn, "text2", &dir).unwrap();
        // 插入 1 条链接(会被识别为 text+link)
        insert_or_update(&conn, "https://github.com", &dir).unwrap();
        // 插入 1 条 file
        let file_url = b"file:///tmp/test.pdf".to_vec();
        insert_or_update_multi(
            &conn,
            &[FormatInput {
                uti: crate::clipboard::uti::UTI_FILE_URL,
                data: &file_url,
                file_path: None,
            }],
            ContentKind::File,
            "test.pdf",
            &dir,
        )
        .unwrap();

        let map = count_by_kind(&conn).unwrap();
        // text 无 subtype:2 条
        assert_eq!(*map.get(&("text".to_string(), None)).unwrap_or(&0), 2);
        // text + link:1 条
        assert_eq!(*map.get(&("text".to_string(), Some("link".to_string()))).unwrap_or(&0), 1);
        // file:1 条
        assert_eq!(*map.get(&("file".to_string(), None)).unwrap_or(&0), 1);
    }

    #[test]
    fn count_by_kind_empty_db() {
        let conn = db();
        let map = count_by_kind(&conn).unwrap();
        assert!(map.is_empty(), "空 DB 应返回空 map");
    }

    #[test]
    fn storage_by_kind_splits_subtypes() {
        let mut map = HashMap::new();
        map.insert(("text".to_string(), None), 5);
        map.insert(("text".to_string(), Some("link".to_string())), 3);
        map.insert(("text".to_string(), Some("markdown".to_string())), 2);
        map.insert(("rich".to_string(), Some("link".to_string())), 1);
        map.insert(("image".to_string(), None), 4);

        let bk = storage_by_kind_from_map(&map);
        assert_eq!(bk.text, 5, "纯 text(无 subtype)");
        assert_eq!(bk.link, 4, "text+rich 的 link 合计: 3+1");
        assert_eq!(bk.markdown, 2, "text 的 markdown");
        assert_eq!(bk.image, 4);
        assert_eq!(bk.rich, 0, "无纯 rich(有 link 的被拆走了)");
    }

    #[test]
    fn delete_all_clears_everything() {
        let conn = db();
        let dir = test_images_dir();

        // 插入若干条
        insert_or_update(&conn, "text1", &dir).unwrap();
        insert_or_update(&conn, "text2", &dir).unwrap();
        let file_url = b"file:///tmp/a.pdf".to_vec();
        insert_or_update_multi(
            &conn,
            &[FormatInput {
                uti: crate::clipboard::uti::UTI_FILE_URL,
                data: &file_url,
                file_path: None,
            }],
            ContentKind::File,
            "a.pdf",
            &dir,
        )
        .unwrap();

        let deleted = delete_all(&conn, &dir).unwrap();
        assert_eq!(deleted, 3);

        // DB 应完全清空
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(item_count, 0);
        let fmt_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_formats", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fmt_count, 0, "formats 也应被 CASCADE 清空");
    }

    #[test]
    fn delete_all_on_empty_db() {
        let conn = db();
        let dir = test_images_dir();
        let deleted = delete_all(&conn, &dir).unwrap();
        assert_eq!(deleted, 0, "空 DB 清空应返回 0");
    }
}
