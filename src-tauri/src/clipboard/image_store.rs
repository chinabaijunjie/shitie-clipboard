//! 图片存储 — 存盘(sha256 分桶)+ 缩略图 + 引用计数删除
//!
//! 技术依据见 m2-research.md §二。混合方案:文本进 DB BLOB,图片进文件系统。
//!
//! 目录结构:
//! ```text
//! <app_data_dir>/images/
//!   ├── <sha256前2位>/<sha256>.png   ← 原图(统一 PNG,即使原是 TIFF)
//!   └── thumb/<sha256>.png           ← 缩略图(200px,列表渲染用)
//! ```
//!
//! 去重:sha256 文件名天然去重,同一张图复制多次磁盘只存一份,
//! 多个 clipboard_formats 行的 file_path 指向同一文件。删历史时引用计数。
//!
//! ⚠️ ClipboardContent::Image.as_bytes() 返回空(源码明确不支持),
//!    图片转字节必须 to_png().get_bytes()。

use std::path::Path;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// 缩略图最长边像素(保比例)。
const THUMB_MAX_SIZE: u32 = 200;

/// 图片存储的元数据(存盘后返回)。
#[derive(Debug, Clone)]
pub struct ImageMeta {
    /// 原图相对路径(相对 images_dir,如 `ab/abcdef....png`)。
    pub rel_path: String,
    /// 缩略图相对路径(如 `thumb/abcdef....png`)。
    /// 前端列表渲染缩略图用;当前 fill_image_fields 从 file_path(rel_path)推导等价值。
    #[allow(dead_code)]
    pub thumb_rel_path: String,
    /// 图片宽度(像素)。
    pub width: u32,
    /// 图片高度(像素)。
    pub height: u32,
}

/// 存图(原图 + 缩略图)。
///
/// `png_bytes` 是已编码的 PNG 字节(调用方负责从 RustImageData::to_png() 转换)。
/// `images_dir` 是图片根目录(`<app_data_dir>/images/`)。
///
/// 返回相对路径 + 尺寸。尺寸从 PNG 字节解码得到(用于标题 + 缩略图比例)。
/// 同一内容(sha256 相同)重复存储是幂等的(覆盖写,不报错)。
pub fn save_image(png_bytes: &[u8], images_dir: &Path) -> Result<ImageMeta, AppError> {
    let hash = sha256_hex(png_bytes);
    let (width, height) = read_png_dimensions(png_bytes).unwrap_or((0, 0));

    let rel_path = relative_image_path(&hash);
    let thumb_rel_path = relative_thumb_path(&hash);

    let orig_full = images_dir.join(&rel_path);
    let thumb_full = images_dir.join(&thumb_rel_path);

    // 原图:若已存在(sha256 去重命中)则跳过写入
    if !orig_full.exists() {
        if let Some(parent) = orig_full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal(format!("创建图片目录 {} 失败: {e}", parent.display()))
            })?;
        }
        std::fs::write(&orig_full, png_bytes).map_err(|e| {
            AppError::internal(format!("写原图 {} 失败: {e}", orig_full.display()))
        })?;
    }

    // 缩略图:若已存在也跳过(同 hash 缩略图一致)
    if !thumb_full.exists() {
        if let Some(parent) = thumb_full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal(format!("创建缩略图目录 {} 失败: {e}", parent.display()))
            })?;
        }
        write_thumbnail(png_bytes, &thumb_full, width, height)?;
    }

    Ok(ImageMeta {
        rel_path,
        thumb_rel_path,
        width,
        height,
    })
}

/// 查某 file_path 在 clipboard_formats 表里的引用数(引用计数删除用)。
pub fn count_references(conn: &Connection, file_path: &str) -> Result<i64, AppError> {
    conn.query_row(
        "SELECT COUNT(*) FROM clipboard_formats WHERE file_path = ?1",
        rusqlite::params![file_path],
        |r| r.get(0),
    )
    .map_err(|e| AppError::internal(format!("查图片引用数失败: {e}")))
}

/// 引用计数删除:若引用数 <= 1(只剩当前这条),删磁盘文件(原图 + 缩略图)。
///
/// 在 delete_item 删 DB 行**之前**调用:此时引用数含当前行。
/// - 引用数 == 1:当前行是唯一引用 → 删 DB 行后无人引用 → 删磁盘文件
/// - 引用数 > 1:还有其他 item 引用 → 只删 DB 行,保留磁盘文件
///
/// `file_path` 是 clipboard_formats.file_path(相对路径)。
/// 删磁盘失败只 log 不阻断(DB 行照删,磁盘文件成孤儿,后续可加清理任务)。
pub fn delete_if_last_reference(
    conn: &Connection,
    file_path: &str,
    images_dir: &Path,
) -> Result<(), AppError> {
    let count = count_references(conn, file_path)?;
    if count <= 1 {
        let orig = images_dir.join(file_path);
        let thumb = images_dir.join(relative_thumb_path_from_image(file_path));
        // 删原图
        if let Err(e) = std::fs::remove_file(&orig) {
            if !matches!(e.kind(), std::io::ErrorKind::NotFound) {
                log::warn!("[image_store] 删原图 {} 失败(成孤儿): {e}", orig.display());
            }
        }
        // 删缩略图
        if let Err(e) = std::fs::remove_file(&thumb) {
            if !matches!(e.kind(), std::io::ErrorKind::NotFound) {
                log::warn!("[image_store] 删缩略图 {} 失败(成孤儿): {e}", thumb.display());
            }
        }
    }
    Ok(())
}

// ============================================================
// 内部辅助(纯函数,可测)
// ============================================================

/// 计算 SHA256 十六进制字符串。
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// 从 sha256 生成原图相对路径:`<前2位>/<完整hash>.png`(分桶防单目录文件膨胀)。
fn relative_image_path(hash: &str) -> String {
    let (bucket, rest) = hash.split_at(2);
    format!("{}/{}.png", bucket, rest)
}

/// 从 sha256 生成缩略图相对路径:`thumb/<完整hash>.png`。
fn relative_thumb_path(hash: &str) -> String {
    format!("thumb/{}.png", hash)
}

/// 从原图相对路径推缩略图相对路径(pub 版本,供 storage 层填 thumb_path 用)。
pub fn relative_thumb_path_from_image_pub(image_rel: &str) -> String {
    relative_thumb_path_from_image(image_rel)
}

/// 把缩略图相对路径转成绝对路径字符串(给前端 convertFileSrc 用)。
///
/// handler emit / command 返回前调用。None 或转失败时返回 None。
pub fn resolve_thumb_absolute(thumb_rel: &Option<String>, images_dir: &Path) -> Option<String> {
    let rel = thumb_rel.as_ref()?;
    let abs = images_dir.join(rel);
    abs.to_str().map(|s| s.to_string())
}

/// 从原图相对路径反推缩略图相对路径。
///
/// 原图:`ab/cdef...png`(分桶 = sha256 前 2 位,rest = 剩余 hash + .png)
/// 缩略图:`thumb/<完整hash>.png`(完整 hash = bucket + rest 不含扩展名)
///
/// 所以要把 bucket 加回到 basename 前面,组成完整 hash。
fn relative_thumb_path_from_image(image_rel: &str) -> String {
    let p = Path::new(image_rel);
    let bucket = p.parent().and_then(|s| s.to_str()).unwrap_or("");
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    // 完整 hash = bucket + stem(如 "ab" + "cdef..." )
    let full_hash = format!("{}{}", bucket, stem);
    format!("thumb/{}.png", full_hash)
}

/// 读 PNG 字节的宽高(用 clipboard-rs 的 RustImageData 解码)。
///
/// clipboard-rs 已开 image feature,复用它的解码能力,不单独引 image crate。
fn read_png_dimensions(png_bytes: &[u8]) -> Option<(u32, u32)> {
    use clipboard_rs::common::RustImage;
    let img = clipboard_rs::RustImageData::from_bytes(png_bytes).ok()?;
    let (w, h) = img.get_size();
    Some((w, h))
}

/// 生成并写缩略图。
///
/// 用 clipboard-rs 的 thumbnail(保比例,最长边 THUMB_MAX_SIZE)。
/// 极小图(小于 THUMB_MAX_SIZE)thumbnail 不放大。
fn write_thumbnail(
    png_bytes: &[u8],
    thumb_full: &Path,
    orig_w: u32,
    orig_h: u32,
) -> Result<(), AppError> {
    use clipboard_rs::common::RustImage;

    // 极小图直接拷贝原图当缩略图(thumbnail 不放大但仍会重新编码,直接拷贝更省)
    if orig_w <= THUMB_MAX_SIZE && orig_h <= THUMB_MAX_SIZE {
        std::fs::write(thumb_full, png_bytes).map_err(|e| {
            AppError::internal(format!("写缩略图 {} 失败: {e}", thumb_full.display()))
        })?;
        return Ok(());
    }

    let img = clipboard_rs::RustImageData::from_bytes(png_bytes)
        .map_err(|e| AppError::internal(format!("解码图片生成缩略图失败: {e}")))?;
    let thumb = img
        .thumbnail(THUMB_MAX_SIZE, THUMB_MAX_SIZE)
        .map_err(|e| AppError::internal(format!("生成缩略图失败: {e}")))?;
    let thumb_png = thumb
        .to_png()
        .map_err(|e| AppError::internal(format!("缩略图编码 PNG 失败: {e}")))?;
    std::fs::write(thumb_full, thumb_png.get_bytes()).map_err(|e| {
        AppError::internal(format!("写缩略图 {} 失败: {e}", thumb_full.display()))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_in_memory;

    #[test]
    fn sha256_hex_is_stable() {
        let h1 = sha256_hex(b"abc");
        let h2 = sha256_hex(b"abc");
        assert_eq!(h1, h2, "相同内容 sha256 应一致");
        assert_eq!(h1.len(), 64, "SHA256 hex 应 64 字符");
        // abc 的 SHA256 已知值
        assert_eq!(h1, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn sha256_hex_different_input() {
        assert_ne!(sha256_hex(b"abc"), sha256_hex(b"abd"));
    }

    #[test]
    fn relative_image_path_uses_bucket() {
        // sha256 前 2 位分桶
        let path = relative_image_path("abcdef1234567890");
        assert_eq!(path, "ab/cdef1234567890.png");
    }

    #[test]
    fn relative_thumb_path_format() {
        let path = relative_thumb_path("abcdef1234567890");
        assert_eq!(path, "thumb/abcdef1234567890.png");
    }

    #[test]
    fn thumb_path_from_image_reconstructs_full_hash() {
        // 原图 ab/cdef...png(bucket=ab, stem=cdef...) → 缩略图 thumb/abcdef...png(完整 hash)
        let thumb = relative_thumb_path_from_image("ab/cdef1234567890.png");
        assert_eq!(thumb, "thumb/abcdef1234567890.png");
    }

    #[test]
    fn thumb_path_from_image_no_bucket() {
        // 容错:无分桶目录(bucket 为空)→ 缩略图 thumb/<stem>.png
        let thumb = relative_thumb_path_from_image("cdef.png");
        assert_eq!(thumb, "thumb/cdef.png");
    }

    #[test]
    fn count_references_empty_db() {
        let conn = init_db_in_memory().unwrap();
        assert_eq!(count_references(&conn, "ab/xyz.png").unwrap(), 0);
    }

    #[test]
    fn count_references_counts_correctly() {
        // 插两条 formats 行指向同一 file_path → 引用数 2
        let conn = init_db_in_memory().unwrap();
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) \
             VALUES (1,1,'a','image')",
            [],
        )
        .unwrap();
        let id1 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) \
             VALUES (2,2,'b','image')",
            [],
        )
        .unwrap();
        let id2 = conn.last_insert_rowid();

        let fp = "ab/shared.png";
        for id in [id1, id2] {
            conn.execute(
                "INSERT INTO clipboard_formats (item_id, uti, file_path, size) VALUES (?1,'public.png',?2,0)",
                rusqlite::params![id, fp],
            )
            .unwrap();
        }
        assert_eq!(count_references(&conn, fp).unwrap(), 2, "两行指向同一文件应计数 2");
    }

    #[test]
    fn save_image_creates_files() {
        // 用一张极小的测试 PNG(1x1 红点)
        let png = minimal_test_png();
        let tmp = tempdir_for_test();
        let images_dir = tmp.join("images");

        let meta = save_image(&png, &images_dir).expect("存图应成功");

        // 原图文件存在
        assert!(images_dir.join(&meta.rel_path).exists(), "原图应存在");
        // 缩略图文件存在
        assert!(images_dir.join(&meta.thumb_rel_path).exists(), "缩略图应存在");
        // 尺寸 2x2(TEST_PNG_BASE64 是 2x2 红色图)
        assert_eq!(meta.width, 2);
        assert_eq!(meta.height, 2);
    }

    #[test]
    fn save_image_dedup_is_idempotent() {
        // 同一内容存两次:文件仍只一份(覆盖写),不报错
        let png = minimal_test_png();
        let tmp = tempdir_for_test();
        let images_dir = tmp.join("images");

        let meta1 = save_image(&png, &images_dir).unwrap();
        let meta2 = save_image(&png, &images_dir).unwrap();

        // 相同内容 → 相同路径
        assert_eq!(meta1.rel_path, meta2.rel_path);
        assert_eq!(meta1.thumb_rel_path, meta2.thumb_rel_path);
    }

    #[test]
    fn delete_if_last_reference_removes_when_count_one() {
        let conn = init_db_in_memory().unwrap();
        let png = minimal_test_png();
        let tmp = tempdir_for_test();
        let images_dir = tmp.join("images");

        let meta = save_image(&png, &images_dir).unwrap();
        // 插一条引用
        conn.execute(
            "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) VALUES (1,1,'img','image')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO clipboard_formats (item_id, uti, file_path, size) VALUES (?1,'public.png',?2,0)",
            rusqlite::params![id, &meta.rel_path],
        )
        .unwrap();

        // 引用数 1 → 删除应移除磁盘文件
        assert!(images_dir.join(&meta.rel_path).exists());
        delete_if_last_reference(&conn, &meta.rel_path, &images_dir).unwrap();
        assert!(!images_dir.join(&meta.rel_path).exists(), "引用数 1 应删磁盘文件");
        assert!(!images_dir.join(&meta.thumb_rel_path).exists(), "缩略图也应删");
    }

    #[test]
    fn delete_if_last_reference_keeps_when_count_two() {
        let conn = init_db_in_memory().unwrap();
        let png = minimal_test_png();
        let tmp = tempdir_for_test();
        let images_dir = tmp.join("images");

        let meta = save_image(&png, &images_dir).unwrap();
        // 插两条引用
        for ts in [1, 2] {
            conn.execute(
                "INSERT INTO clipboard_items (first_copied_at, last_copied_at, title, content_kind) VALUES (?1,?1,'img','image')",
                rusqlite::params![ts],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO clipboard_formats (item_id, uti, file_path, size) VALUES (?1,'public.png',?2,0)",
                rusqlite::params![id, &meta.rel_path],
            )
            .unwrap();
        }

        // 引用数 2 → 删除应保留磁盘文件
        delete_if_last_reference(&conn, &meta.rel_path, &images_dir).unwrap();
        assert!(images_dir.join(&meta.rel_path).exists(), "引用数 2 应保留磁盘文件");
    }

    // ===== 测试辅助 =====

    /// 用 image crate 生成合法的 2x2 红色 PNG(可靠,避免手写/解码字节出错)。
    fn minimal_test_png() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2, 2, |_, _| Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    /// 测试用临时目录(每个测试独立,自动清理)。
    fn tempdir_for_test() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "clipboard-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
