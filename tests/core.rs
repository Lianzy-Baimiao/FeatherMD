use feathermd::{
    document::{Document, PAGE_BYTES, SEGMENT_BYTES, SearchResult},
    markdown::{self, Fence, Kind},
    settings::{Editor, Settings, Theme},
};
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn system_theme_tracks_os_while_explicit_choices_stay_fixed() {
    assert_eq!(Settings::default().theme, Theme::System);
    for theme in Theme::ALL {
        let settings = Settings {
            theme,
            ..Settings::default()
        };
        assert_eq!(Settings::parse(&settings.serialize()), settings);
        for dark in [false, true] {
            let expected = if theme == Theme::System {
                if dark { Theme::Dark } else { Theme::Light }
            } else {
                theme
            };
            assert_eq!(theme.resolve(dark), expected);
        }
    }
    assert_eq!(Settings::parse("theme=light").theme, Theme::Light);
}
#[test]
fn skins_opacity_and_editor_roundtrip_with_safe_fallbacks() {
    for (index, theme) in Theme::ALL.into_iter().enumerate() {
        assert_eq!(theme.index(), index);
        assert_eq!(Theme::from_key(theme.key()), Some(theme));
        let parsed = Settings::parse(&format!("theme={}", theme.key()));
        assert_eq!(parsed.theme, theme);
    }
    assert_eq!(Settings::parse("theme=neon").theme, Theme::System);
    let dark = [
        Theme::Dark,
        Theme::Midnight,
        Theme::Black,
        Theme::Dusk,
        Theme::Polar,
        Theme::DeepSea,
        Theme::Violet,
        Theme::Mocha,
    ];
    for theme in Theme::ALL {
        assert_eq!(theme.is_dark(), dark.contains(&theme), "{}", theme.key());
    }
    // Saved indices of the original twelve skins never move.
    assert_eq!(Theme::Black.index(), 11);
    assert_eq!(Theme::Mocha.index(), 19);
    // The gallery shows every skin exactly once: light ones first, then dark ones.
    for theme in Theme::ALL {
        assert_eq!(Theme::GALLERY.iter().filter(|t| **t == theme).count(), 1);
        assert_eq!(Theme::GALLERY[theme.gallery_index()], theme);
    }
    let first_dark = Theme::GALLERY.iter().position(|t| t.is_dark()).unwrap();
    assert!(Theme::GALLERY[first_dark..].iter().all(|t| t.is_dark()));

    assert_eq!(Settings::default().opacity, 100);
    for (text, expected) in [
        ("100", 100),
        ("85", 85),
        ("87", 85),
        ("88", 90),
        ("33", 35),
        ("5", Settings::MIN_OPACITY),
        ("-40", Settings::MIN_OPACITY),
        ("250", 100),
        ("70%", 70),
    ] {
        assert_eq!(
            Settings::parse(&format!("opacity={text}")).opacity,
            expected
        );
    }
    assert_eq!(Settings::parse("opacity=half").opacity, 100);
    // Update checks are opt-in; bookkeeping survives a round trip and rejects injection.
    let defaults = Settings::default();
    assert!(
        !defaults.update_check && defaults.update_checked == 0 && defaults.update_latest.is_empty()
    );
    let checked =
        Settings::parse("update_check=true\nupdate_checked=1800000000\nupdate_latest=v0.2.0");
    assert!(checked.update_check);
    assert_eq!(
        (checked.update_checked, checked.update_latest.as_str()),
        (1_800_000_000, "v0.2.0")
    );
    assert_eq!(Settings::parse(&checked.serialize()), checked);
    for bad in [
        "update_check=yes",
        "update_checked=-1",
        "update_latest=v1/../x",
        "update_latest=v1 2",
    ] {
        let s = Settings::parse(bad);
        assert!(
            !s.update_check && s.update_checked == 0 && s.update_latest.is_empty(),
            "{bad}"
        );
    }
    let injected = Settings {
        update_latest: "v1.0.0\nraw=true".into(),
        ..Settings::default()
    };
    assert!(!Settings::parse(&injected.serialize()).raw);
    // A UTF-8 BOM must not hide the first setting.
    let bom = Settings::parse("\u{feff}theme=light\nfont_size=20");
    assert_eq!((bom.theme, bom.font_size), (Theme::Light, 20));

    assert_eq!(Settings::default().editor, Editor::System);
    assert_eq!(Settings::parse("editor=notepad").editor, Editor::Notepad);
    let program = r"C:\Program Files\Microsoft VS Code\Code.exe";
    let s = Settings::parse(&format!("theme=ancient\nopacity=75\neditor={program}"));
    assert_eq!(s.editor, Editor::Program(program.into()));
    assert_eq!(Settings::parse(&s.serialize()), s);
    let unicode = r"D:\工具 箱\编辑器=1\Edit.EXE";
    assert_eq!(Editor::parse(unicode), Editor::Program(unicode.into()));
    for bad in [
        "Code.exe",
        r"\\server\share\edit.exe",
        r"C:relative\edit.exe",
        r"C:\tools\edit.bat",
        r"C:\tools\edit.exe:stream.exe",
        "C:\\tools\\\"edit\".exe",
        "C:\\tools\\edit\n.exe",
        r"C:\tools\*.exe",
        "C:/tools/edit.exe",
        r"C:\.exe",
    ] {
        assert_eq!(Editor::parse(bad), Editor::System, "{bad:?}");
    }
    // A program set in code is validated again before it reaches the INI.
    let injected = Settings {
        editor: Editor::Program("C:\\a.exe\nraw=true".into()),
        ..Settings::default()
    };
    let text = injected.serialize();
    assert!(text.contains("\neditor=system\n"));
    assert!(!Settings::parse(&text).raw);
}
struct Fixture(PathBuf);
impl Fixture {
    fn new(data: &[u8]) -> Self {
        let p = std::env::temp_dir().join(format!(
            "feathermd-test-{}-{}.md",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&p, data).unwrap();
        Self(p)
    }
    fn doc(&self) -> Document {
        Document::open(&self.0).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[test]
fn empty_and_bom() {
    for text in [b"".as_slice(), b"\xef\xbb\xbf"] {
        let f = Fixture::new(text);
        assert!(f.doc().page(0).unwrap().lines.is_empty());
    }
}
#[test]
fn chinese_crlf_offsets() {
    let f = Fixture::new("\u{feff}# 中文\r\nhello 😀\n尾行".as_bytes());
    let p = f.doc().page(0).unwrap();
    assert_eq!(p.lines.len(), 3);
    assert_eq!(p.lines[0].start, 3);
    assert_eq!(p.lines[0].text, "# 中文");
    assert_eq!(p.lines[1].text, "hello 😀");
    assert_eq!(p.end, f.doc().size);
    assert!(!p.invalid_utf8);
    for pair in p.lines.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
}
#[test]
fn long_line_is_bounded() {
    let f = Fixture::new(&vec![b'x'; 4 * 1024 * 1024]);
    for anchor in [0, 50000, 1_000_000, 4 * 1024 * 1024 - 1] {
        let p = f.doc().page(anchor).unwrap();
        assert!(p.end - p.start <= PAGE_BYTES as u64 + 4);
        assert!(p.start <= anchor && p.end > anchor);
        assert!(p.lines.iter().all(|l| l.text.len() <= SEGMENT_BYTES));
        assert!(p.lines.len() <= 2048);
    }
}
#[test]
fn dense_newlines_keep_anchor() {
    let f = Fixture::new(&vec![b'\n'; 1_000_000]);
    for anchor in [0, 20_000, 50_000, 999_999] {
        let p = f.doc().page(anchor).unwrap();
        assert!(p.lines.len() <= 2048);
        assert!(p.start <= anchor && p.end > anchor);
    }
}
#[test]
fn multibyte_boundaries_never_corrupt_valid_utf8() {
    for prefix in 0..4 {
        let text = format!("{}{}", "a".repeat(prefix), "中😀文é".repeat(30000));
        let f = Fixture::new(text.as_bytes());
        for anchor in [0, 32769, 50000, 150003, text.len() as u64 - 1] {
            let p = f.doc().page(anchor).unwrap();
            assert!(!p.invalid_utf8, "prefix={prefix} anchor={anchor}");
            assert!(p.lines.iter().all(|l| !l.text.contains('�')));
        }
    }
}
#[test]
fn invalid_bytes_are_flagged() {
    // UTF-8 text with one stray byte stays UTF-8 on every system code page, so the
    // byte is replaced and flagged. (Text with no UTF-8 at all may be read as the
    // system's ANSI page instead: see tests/encoding.rs.)
    let f = Fixture::new(
        "中文 abc\u{e9}\n"
            .as_bytes()
            .iter()
            .copied()
            .chain(*b"\xff\n")
            .collect::<Vec<_>>()
            .as_slice(),
    );
    assert!(f.doc().page(0).unwrap().invalid_utf8);
}
#[test]
fn utf16_is_rejected_clearly() {
    let f = Fixture::new(b"\xff\xfeh\0i\0");
    assert!(
        Document::open(&f.0)
            .unwrap_err()
            .to_string()
            .contains("UTF-16")
    );
}
#[test]
fn file_can_be_truncated_while_open() {
    let f = Fixture::new(&vec![b'a'; 500000]);
    let d = f.doc();
    fs::write(&f.0, b"new\n").unwrap();
    let p = d.page(499999).unwrap();
    assert_eq!(p.size, 4);
    assert_eq!(p.lines[0].text, "new");
}
#[test]
fn search_crosses_io_boundary() {
    let mut data = vec![b'x'; 256 * 1024 - 2];
    let position = data.len() as u64;
    data.extend_from_slice("中文 needle 😀".as_bytes());
    let f = Fixture::new(&data);
    assert_eq!(
        f.doc()
            .search("中文 needle", 0, &AtomicU64::new(1), 1)
            .unwrap(),
        SearchResult::Found(position)
    );
}
#[test]
fn search_wraps_and_overlaps() {
    let f = Fixture::new(b"aaaaa xyz");
    let d = f.doc();
    let epoch = AtomicU64::new(1);
    assert_eq!(
        d.search("aaa", 1, &epoch, 1).unwrap(),
        SearchResult::Found(1)
    );
    assert_eq!(
        d.search("aaaaa", 1, &epoch, 1).unwrap(),
        SearchResult::Found(0)
    );
    assert_eq!(
        d.search("xyz", d.size, &epoch, 1).unwrap(),
        SearchResult::Found(6)
    );
}
#[test]
fn search_cancel_empty_and_not_found() {
    let f = Fixture::new(b"abc");
    let d = f.doc();
    let epoch = AtomicU64::new(3);
    assert_eq!(
        d.search("a", 0, &epoch, 2).unwrap(),
        SearchResult::Cancelled
    );
    assert_eq!(
        d.search("nope", 0, &epoch, 3).unwrap(),
        SearchResult::NotFound
    );
    assert!(d.search("", 0, &epoch, 3).is_err());
    assert!(d.search(&"x".repeat(17000), 0, &epoch, 3).is_err());
}
#[test]
fn fence_marker_length_and_indent() {
    let mut f = Fence::default();
    assert!(f.feed("````rust"));
    assert!(f.is_open());
    assert!(!f.feed("```"));
    assert!(!f.feed("~~~~"));
    assert!(!f.feed("```` nope"));
    assert!(f.feed("````"));
    assert!(!f.is_open());
    assert!(!f.feed("    ```"));
    assert!(f.feed("~~~"));
}
#[test]
fn markdown_styles_and_safe_links() {
    let spans = markdown::inline("**粗体** *斜体* ~~删除~~ `code` [网页](https://example.com)");
    assert!(spans.iter().any(|s| s.bold && s.text == "粗体"));
    assert!(spans.iter().any(|s| s.italic));
    assert!(spans.iter().any(|s| s.strike));
    assert!(spans.iter().any(|s| s.code));
    assert!(
        spans
            .iter()
            .any(|s| s.link.as_deref() == Some("https://example.com"))
    );
    for link in [
        "file:///C:/test.exe",
        "javascript:alert(1)",
        "https://site\narg",
    ] {
        assert!(!markdown::safe_web_link(link));
    }
    assert!(markdown::safe_web_link("HTTPS://example.com"));
}
#[test]
fn block_kinds_lists_and_raw() {
    let f = Fixture::new(b"# Heading\n> quote\n- [x] done\n1. first\n```rs\n# code\n```\n---\n");
    let p = f.doc().page(0).unwrap();
    let b = markdown::blocks(&p.lines, false);
    assert_eq!(b[0].kind, Kind::Heading(1));
    assert_eq!(b[1].kind, Kind::Quote);
    assert_eq!(b[2].spans[0].text, "☑ ");
    assert_eq!(b[3].spans[0].text, "1. ");
    assert_eq!(b[5].kind, Kind::Code);
    assert_eq!(b[7].kind, Kind::Rule);
    assert!(
        markdown::blocks(&p.lines, true)
            .iter()
            .all(|b| b.kind == Kind::Code)
    );
}
#[test]
fn settings_validate_roundtrip_and_atomic_save() {
    let s =
        Settings::parse("font_size=999\ntheme=dark\nwrap=false\nraw=true\nwide=true\nunknown=1");
    assert_eq!(s.font_size, 36);
    assert_eq!(s.theme, Theme::Dark);
    assert!(!s.wrap);
    assert_eq!(Settings::parse(&s.serialize()), s);
    let file = Fixture::new(b"old");
    s.save(&file.0).unwrap();
    assert_eq!(Settings::load(&file.0), s);
    assert_eq!(Settings::parse("font_size=-2").font_size, 12);
}
#[test]
fn random_binary_pages_do_not_panic() {
    let mut seed = 93u64;
    let mut data = vec![0; 400000];
    for b in &mut data {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *b = (seed >> 32) as u8;
    }
    let f = Fixture::new(&data);
    for anchor in (0..400000).step_by(7919) {
        let p = f.doc().page(anchor).unwrap();
        assert!(p.lines.len() <= 2048);
        let _ = markdown::blocks(&p.lines, false);
    }
}
#[test]
fn sparse_100_gib_reads_only_a_window() {
    let f = Fixture::new(b"# Huge\n");
    let mut file = File::options().write(true).open(&f.0).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let mut returned = 0;
        assert_ne!(
            unsafe {
                windows_sys::Win32::System::IO::DeviceIoControl(
                    file.as_raw_handle(),
                    0x900c4,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
    }
    let size = 100u64 * 1024 * 1024 * 1024;
    file.set_len(size).unwrap();
    file.seek(SeekFrom::Start(size - 20)).unwrap();
    file.write_all(b"\n# end of document\n").unwrap();
    let d = f.doc();
    let p = d.page(size - 1).unwrap();
    assert!(p.end - p.start <= PAGE_BYTES as u64 + 4);
    assert!(p.lines.iter().any(|l| l.text == "# end of document"));
    assert_eq!(d.size, size);
}

#[test]
fn custom_font_settings_validate_and_survive_roundtrip() {
    let s = Settings::parse("font_family=宋体\nfont_size=22");
    assert_eq!(s.font_family, "宋体");
    assert_eq!(s.font_size, 22);
    assert_eq!(Settings::parse(&s.serialize()), s);
    assert_eq!(
        Settings::parse("font_size=20").font_family,
        Settings::default().font_family
    );
    for bad in [
        "",
        "@SimSun",
        "bad\0font",
        "Segoe\tUI",
        "Injected\nraw=true",
    ] {
        assert!(Settings::validate_font_family(bad).is_none());
    }
    assert!(Settings::validate_font_family(&"a".repeat(32)).is_none());
    assert!(Settings::validate_font_family(&"a".repeat(31)).is_some());
    assert!(Settings::validate_font_family(&"😀".repeat(16)).is_none());
    let mut injected = s.clone();
    injected.font_family = "Bad\nraw=true".into();
    assert!(!Settings::parse(&injected.serialize()).raw);
    let file = Fixture::new(b"old");
    s.save(&file.0).unwrap();
    assert_eq!(Settings::load(&file.0), s);
}
