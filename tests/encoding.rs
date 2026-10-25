use feathermd::encoding::{self, Codec, Encoding};

#[test]
fn gb18030_boundaries_are_walked_from_a_known_start() {
    let gb = Codec::gb18030();
    // ASCII, a two-byte character (中 = D6 D0) and a four-byte one (81 30 81 30).
    let bytes = [b'a', 0xd6, 0xd0, 0x81, 0x30, 0x81, 0x30, b'z'];
    assert_eq!(gb.char_len(&bytes), 1);
    assert_eq!(gb.char_len(&bytes[1..]), 2);
    assert_eq!(gb.char_len(&bytes[3..]), 4);
    // Cuts never split a character.
    assert_eq!(gb.floor(&bytes, 2), 1);
    assert_eq!(gb.floor(&bytes, 3), 3);
    assert_eq!(gb.floor(&bytes, 6), 3);
    assert_eq!(gb.floor(&bytes, 8), 8);
    // A lone lead byte at the end is left for the next read.
    assert_eq!(gb.floor(&[b'a', 0xd6], 2), 1);
    // A trail byte that looks like ASCII ('a' = 0x61 after a lead) is not a start.
    let trail = [0xb0, 0x61, b'a'];
    assert!(!gb.aligned(&trail, 1));
    assert!(gb.aligned(&trail, 2));
    // Newlines and most punctuation end a character; digits may continue one.
    assert!(gb.sync(b'\n') && gb.sync(b' ') && !gb.sync(b'5') && !gb.sync(0xd6));
}

#[test]
fn files_are_utf8_unless_clearly_not() {
    let gb = Some(Codec::gb18030());
    assert_eq!(encoding::detect(b"plain ascii\n", gb), Encoding::Utf8);
    assert_eq!(
        encoding::detect("中文 UTF-8 文本".as_bytes(), gb),
        Encoding::Utf8
    );
    assert_eq!(
        encoding::detect(b"\xef\xbb\xbf\xd6\xd0", gb),
        Encoding::Utf8
    );
    // NUL bytes: binary or UTF-16 without a BOM, never a legacy text page.
    assert_eq!(encoding::detect(b"a\0b\0", gb), Encoding::Utf8);
    // One stray byte in UTF-8 text does not make it legacy.
    let mut mostly = "中文内容很多很多很多很多".repeat(4).into_bytes();
    mostly.push(0xff);
    assert_eq!(encoding::detect(&mostly, gb), Encoding::Utf8);
    // Without a legacy page (UTF-8 systems, other platforms) everything is UTF-8.
    assert_eq!(encoding::detect(b"\xd6\xd0\xce\xc4", None), Encoding::Utf8);
    assert_eq!(Encoding::Utf8.name(), "UTF-8");
    assert_eq!(Codec::gb18030().name(), "GB18030");
}

#[cfg(windows)]
mod windows {
    use feathermd::{
        document::{Document, SearchResult},
        encoding::{self, Codec, Encoding},
    };
    use std::sync::atomic::AtomicU64;

    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn file(bytes: &[u8]) -> (Temp, Document) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("feathermd-gbk-{}-{n}.txt", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let mut document = Document::open(&path).unwrap();
        // The test does not depend on this computer's code page.
        document.encoding = Encoding::Legacy(Codec::gb18030());
        (Temp(path), document)
    }

    #[test]
    fn gbk_text_is_decoded_and_found() {
        let gb = Codec::gb18030();
        assert_eq!(
            gb.encode("中文").as_deref(),
            Some(&[0xd6, 0xd0, 0xce, 0xc4][..])
        );
        assert_eq!(
            gb.decode(&[0xd6, 0xd0, 0xce, 0xc4]),
            ("中文".to_owned(), false)
        );
        assert!(gb.decode(&[0xd6]).1);
        let novel = "第一章 少年\r\n　　他走了。\r\nend";
        let bytes = gb.encode(novel).unwrap();
        assert_eq!(encoding::detect(&bytes, Some(gb)), Encoding::Legacy(gb));
        let (_temp, document) = file(&bytes);
        let page = document.page(0).unwrap();
        let lines: Vec<&str> = page.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(lines, ["第一章 少年", "　　他走了。", "end"]);
        assert!(!page.invalid_utf8);
        assert_eq!(page.encoding.name(), "GB18030");
        let epoch = AtomicU64::new(0);
        let at = gb.encode("第一章 少年\r\n　　").unwrap().len() as u64;
        assert_eq!(
            document.search("他走了", 0, &epoch, 0).unwrap(),
            SearchResult::Found(at)
        );
        assert_eq!(
            document.search("缺失", 0, &epoch, 0).unwrap(),
            SearchResult::NotFound
        );
    }

    #[test]
    fn long_gbk_lines_split_and_resume_at_whole_characters() {
        let gb = Codec::gb18030();
        // About 74 KiB on one line: Chinese with a space now and then, as in prose.
        let line: String = (0..40_000)
            .map(|i| if i % 7 == 6 { ' ' } else { '字' })
            .collect();
        let bytes = gb.encode(&format!("{line}\nnext")).unwrap();
        let (_temp, document) = file(&bytes);
        let page = document.page(0).unwrap();
        assert!(!page.invalid_utf8);
        assert!(page.lines.iter().all(|l| l.text.len() <= 3 * 4096));
        let segments: String = page
            .lines
            .iter()
            .take_while(|l| l.continued)
            .map(|l| l.text.as_str())
            .collect();
        assert!(line.starts_with(&segments) && segments.chars().count() > 4096);
        // A page that starts inside the line (beyond the 32 KiB look-behind) resumes
        // at a whole character after a space.
        let page = document.page(50_000).unwrap();
        assert!(
            !page.invalid_utf8,
            "first segment: {:?}",
            page.lines[0].text
        );
        assert!(page.lines[0].text.chars().all(|c| c == ' ' || c == '字'));
        assert!(
            page.lines
                .iter()
                .any(|l| l.start <= 50_000 && 50_000 < l.end)
        );
    }

    #[test]
    fn an_ascii_query_never_matches_the_trail_byte_of_a_character() {
        // 0x81 0x40 is one character whose trail byte is '@'; then a real '@'.
        let bytes = [0x81, 0x40, b'@'];
        let (text, bad) = Codec::gb18030().decode(&bytes);
        assert!(!bad);
        assert_eq!(text.chars().nth(1), Some('@'));
        let (_temp, document) = file(&bytes);
        let epoch = AtomicU64::new(0);
        assert_eq!(
            document.search("@", 0, &epoch, 0).unwrap(),
            SearchResult::Found(2)
        );
    }
}
