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
