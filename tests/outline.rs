use feathermd::{
    document::Document,
    outline::{self, CHUNK_BYTES, MAX_HEADING_CHARS, MAX_HEADINGS, MAX_SCAN_BYTES, Outline},
    worker::{OutlineRequest, Reply, Request, Workers},
};
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "feathermd-outline-test-{}-{}.md",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn doc(&self) -> Document {
        Document::open(&self.0).unwrap()
    }
    fn outline(&self) -> Outline {
        outline::scan(&self.doc(), &AtomicU64::new(0), 0)
            .unwrap()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn atx_levels_bom_crlf_and_inline_labels() {
    let fixture = Fixture::new(
        "\u{feff}# **中文** [链接](https://example.com) `code` &amp; 😀 ###\r\n\
         ## two\r\n### three\r\n#### four\r\n##### five\r\n###### six\r\n\
         #not a heading\r\n####### too deep\r\n\t# code\r\n"
            .as_bytes(),
    );
    let outline = fixture.outline();
    assert!(outline.complete);
    assert!(!outline.limited);
    assert_eq!(outline.scanned, fixture.doc().size);
    assert_eq!(outline.headings.len(), 6);
    assert_eq!(outline.headings[0].offset, 3);
    assert_eq!(outline.headings[0].text, "中文 链接 code & 😀");
    assert_eq!(
        outline.headings.iter().map(|h| h.level).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6]
    );
    for heading in &outline.headings {
        let bytes = fs::read(&fixture.0).unwrap();
        assert_eq!(bytes[heading.offset as usize], b'#');
    }
}

#[test]
fn fence_length_kind_and_indentation_are_respected() {
    let fixture = Fixture::new(
        b"    # code\n   ## visible\n````rust\n# hidden\n```\n# hidden too\n~~~~\n# hidden still\n```` trailing\n# hidden again\n````\n~~~\n# hidden tilde\n~~~\n# final\n",
    );
    let outline = fixture.outline();
    assert_eq!(
        outline
            .headings
            .iter()
            .map(|h| h.text.as_str())
            .collect::<Vec<_>>(),
        ["visible", "final"]
    );
}

#[test]
fn chunk_boundaries_preserve_physical_lines_and_unicode() {
    for trailing in 0..8 {
        let mut bytes = vec![b'x'; CHUNK_BYTES - trailing - 1];
        bytes.push(b'\n');
        let offset = bytes.len() as u64;
        bytes.extend_from_slice("### 中文 😀 \r\n# 尾行".as_bytes());
        let outline = Fixture::new(&bytes).outline();
        assert_eq!(outline.headings.len(), 2, "trailing={trailing}");
        assert_eq!(outline.headings[0].offset, offset);
        assert_eq!(outline.headings[0].text, "中文 😀");
        assert_eq!(outline.headings[1].text, "尾行");
        assert!(outline.complete);
    }
}

#[test]
fn long_lines_do_not_create_headings_mid_line_and_labels_are_bounded() {
    let mut bytes = vec![b'x'; CHUNK_BYTES];
    bytes.extend_from_slice(b"# fake heading\n# ");
    bytes.extend_from_slice("中😀".repeat(5000).as_bytes());
    bytes.extend_from_slice(b"\n# final\n");
    let outline = Fixture::new(&bytes).outline();
    assert_eq!(outline.headings.len(), 2);
    assert_eq!(outline.headings[0].text.chars().count(), MAX_HEADING_CHARS);
    assert!(!outline.headings[0].text.contains('\u{fffd}'));
    assert_eq!(outline.headings[1].text, "final");
}

#[test]
fn long_fences_keep_exact_marker_length_and_hidden_suffix_rules() {
    let mut text = "`".repeat(6000);
    text.push_str("\n# hidden\n");
    text.push_str(&"`".repeat(5000));
    text.push_str("\n# also hidden\n");
    text.push_str(&"`".repeat(6000));
    text.push_str(&" ".repeat(5000));
    text.push_str("bad\n# still hidden\n");
    text.push_str(&"`".repeat(6000));
    text.push_str("\n# visible\n``` ");
    text.push_str(&"x".repeat(5000));
    text.push_str("`\n# visible after invalid opening\n");
    let outline = Fixture::new(text.as_bytes()).outline();
    assert_eq!(
        outline
            .headings
            .iter()
            .map(|h| h.text.as_str())
            .collect::<Vec<_>>(),
        ["visible", "visible after invalid opening"]
    );
}

#[test]
fn heading_and_scan_limits_are_explicit() {
    let dense = Fixture::new("# heading\n".repeat(MAX_HEADINGS + 100).as_bytes());
    let result = dense.outline();
    assert_eq!(result.headings.len(), MAX_HEADINGS);
    assert!(result.limited);
    assert!(!result.complete);
    assert!(result.scanned < result.size);

    let large = Fixture::new(b"# first\n");
    let mut file = File::options().write(true).open(&large.0).unwrap();
    file.set_len(MAX_SCAN_BYTES + 1024).unwrap();
    file.seek(SeekFrom::Start(MAX_SCAN_BYTES)).unwrap();
    file.write_all(b"\n# beyond scan limit\n").unwrap();
    let result = large.outline();
    assert_eq!(result.scanned, MAX_SCAN_BYTES);
    assert_eq!(result.headings.len(), 1);
    assert!(result.limited);
    assert!(!result.complete);
}

#[test]
fn cancellation_empty_and_external_truncation() {
    let fixture = Fixture::new(b"# original\n");
    let document = fixture.doc();
    assert!(
        outline::scan(&document, &AtomicU64::new(2), 1)
            .unwrap()
            .is_none()
    );
    fs::write(&fixture.0, "# 新\n").unwrap();
    let result = outline::scan(&document, &AtomicU64::new(0), 0)
        .unwrap()
        .unwrap();
    assert!(result.complete);
    assert_eq!(result.size, "# 新\n".len() as u64);
    assert_eq!(result.headings[0].text, "新");
    for bytes in [b"".as_slice(), b"\xef\xbb\xbf"] {
        let result = Fixture::new(bytes).outline();
        assert!(result.complete);
        assert!(result.headings.is_empty());
        assert_eq!(result.scanned, bytes.len() as u64);
    }
}

#[test]
fn worker_returns_only_current_outline_and_keeps_page_io_independent() {
    let fixture = Fixture::new(b"# worker heading\n");
    let workers = Workers::new();
    let document = Arc::new(fixture.doc());
    let ticket = workers.cancel_outline();
    workers
        .outline
        .send(OutlineRequest {
            document: document.clone(),
            ticket: ticket - 1,
        })
        .unwrap();
    workers
        .outline
        .send(OutlineRequest { document, ticket })
        .unwrap();
    workers
        .io
        .send(Request::Open(fixture.0.clone(), 42))
        .unwrap();
    let mut got_page = false;
    let mut got_outline = false;
    for _ in 0..2 {
        match workers
            .replies
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        {
            Reply::Opened(42, Ok(_)) => got_page = true,
            Reply::Outline(received, Ok(outline)) => {
                assert_eq!(received, ticket);
                assert_eq!(outline.headings[0].text, "worker heading");
                got_outline = true;
            }
            _ => panic!("unexpected or stale worker reply"),
        }
    }
    assert!(got_page && got_outline);
    assert!(workers.replies.try_recv().is_err());
}
