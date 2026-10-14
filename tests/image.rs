//! Image references: only local raster files resolve; nothing remote is ever a path.
use feathermd::{
    i18n,
    image::{self, Source},
    markdown, table,
};
use std::path::{Path, PathBuf};

fn folder() -> &'static Path {
    Path::new(r"D:\Docs\notes")
}
fn local(path: &str) -> Source {
    Source::Local(PathBuf::from(path))
}

#[test]
fn local_files_resolve_next_to_the_document() {
    for (src, expected) in [
        ("a.png", r"D:\Docs\notes\a.png"),
        ("./img/b.JPG", r"D:\Docs\notes\img\b.JPG"),
        ("../shared/c.gif", r"D:\Docs\shared\c.gif"),
        ("/assets/d.png", r"D:\Docs\notes\assets\d.png"),
        (r"img\e.bmp", r"D:\Docs\notes\img\e.bmp"),
        ("my%20photo.jpeg", r"D:\Docs\notes\my photo.jpeg"),
        ("图片/截图.png?raw=1#top", r"D:\Docs\notes\图片\截图.png"),
        (r"C:\Pictures\f.tiff", r"C:\Pictures\f.tiff"),
        ("c:/Pictures/g.ico", r"C:\Pictures\g.ico"),
        ("file:///C:/Pictures/h.png", r"C:\Pictures\h.png"),
        ("file://localhost/C:/Pictures/i.png", r"C:\Pictures\i.png"),
        // `..` never climbs above the drive root.
        ("../../../../j.png", r"D:\j.png"),
    ] {
        assert_eq!(image::resolve(src, folder()), local(expected), "{src}");
    }
}

#[test]
fn network_addresses_and_unsafe_references_never_become_paths() {
    for src in [
        "https://example.com/a.png",
        "HTTP://example.com/a.png",
        "//cdn.example.com/a.png",
        "ftp://example.com/a.png",
        "file://server/share/a.png",
    ] {
        assert_eq!(image::resolve(src, folder()), Source::Remote, "{src}");
    }
    for src in [
        r"\\server\share\a.png",
        "//?/C:/a.png",
        r"\\?\C:\a.png",
        r"\\.\pipe\a.png",
        "data:image/png;base64,AAAA",
        "javascript:alert(1)",
        "C:relative.png",
        "file:relative.png",
        "con.png",
        "img/NUL.png",
        "COM1.jpg",
        "a.png:stream",
        "bad%zz.png",
        "nul%00.png",
        "trailing.png.",
        "",
        "   ",
    ] {
        assert_eq!(image::resolve(src, folder()), Source::Refused, "{src:?}");
    }
    for src in ["logo.svg", "photo.webp", "noext", "archive.png.zip"] {
        assert_eq!(image::resolve(src, folder()), Source::Unsupported, "{src}");
    }
    // A document on a share cannot pull in relative images from it.
    assert_eq!(
        image::resolve("a.png", Path::new(r"\\server\share\docs")),
        Source::Refused
    );
}

#[test]
fn display_size_scales_keeps_aspect_and_only_shrinks_to_fit() {
    // 1:1 at 96 DPI, scaled at 120 DPI, never enlarged by the fit.
    assert_eq!(
        image::display_size((200, 100), (None, None), 1.0, (800, 600)),
        (200, 100)
    );
    assert_eq!(
        image::display_size((200, 100), (None, None), 1.25, (800, 600)),
        (250, 125)
    );
    assert_eq!(
        image::display_size((4000, 1000), (None, None), 1.0, (800, 600)),
        (800, 200)
    );
    assert_eq!(
        image::display_size((1000, 4000), (None, None), 1.0, (800, 600)),
        (150, 600)
    );
    // One size attribute keeps the ratio; both set both.
    assert_eq!(
        image::display_size((400, 200), (Some(100), None), 1.0, (800, 600)),
        (100, 50)
    );
    assert_eq!(
        image::display_size((400, 200), (None, Some(50)), 2.0, (800, 600)),
        (200, 100)
    );
    assert_eq!(
        image::display_size((400, 200), (Some(30), Some(30)), 1.0, (800, 600)),
        (30, 30)
    );
    // Degenerate sizes stay drawable.
    assert_eq!(
        image::display_size((0, 0), (None, None), 1.0, (0, 0)),
        (1, 1)
    );
}

#[test]
fn placeholders_name_the_image() {
    assert_eq!(image::label(" Logo ", "a.png"), "Logo");
    assert_eq!(
        image::label("", "docs/img/my%20shot.png?x=1"),
        "my shot.png"
    );
    assert_eq!(image::label("", r"C:\a\b.gif"), "b.gif");
    // The placeholder names the image and, when images are on, the reason.
    let picture = markdown::Image {
        src: "assets/shot.png".into(),
        ..markdown::Image::default()
    };
    i18n::set(i18n::Lang::En);
    assert_eq!(
        markdown::image_placeholder(&picture, "", None),
        "[Image: shot.png]"
    );
    assert_eq!(
        markdown::image_placeholder(&picture, "Shot", Some("file not found")),
        "[Image: Shot · file not found]"
    );
    let long = "x".repeat(80);
    assert_eq!(
        markdown::image_placeholder(&picture, &long, None)
            .chars()
            .count(),
        "[Image: ]".len() + 60
    );
}

#[test]
fn only_raster_signatures_reach_a_decoder() {
    for bytes in [
        &b"\x89PNG\r\n\x1a\n...."[..],
        b"\xff\xd8\xff\xe0",
        b"GIF89a..",
        b"GIF87a..",
        b"BM......",
        b"II*\0....",
        b"MM\0*....",
        b"\0\0\x01\0\x01\0",
    ] {
        assert!(image::raster(bytes), "{bytes:?}");
    }
    for bytes in [
        &b""[..],
        b"\x89PN",
        b"<svg",
        b"<?xml version",
        b"\xd7\xcd\xc6\x9a",
        b"\x01\0\0\0",
        b"RIFF\0\0\0\0WEBP",
        b"%PDF-1.7",
    ] {
        assert!(!image::raster(bytes), "{bytes:?}");
    }
}

#[test]
fn image_markers_break_on_both_sides_but_not_before_closing_punctuation() {
    // "abcd▣▣，ef" with ▣ = U+FFFC, every unit 10 px wide.
    let units: Vec<u16> = "abcd\u{fffc}\u{fffc}，ef".encode_utf16().collect();
    let ends: Vec<i32> = (1..=units.len() as i32).map(|i| i * 10).collect();
    let lines = |width| table::break_range(&units, &ends, 0, units.len(), width, width);
    // Latin text may break right before an image, with no space; the comma stays
    // with the image it follows.
    assert_eq!(lines(45), [(0, 4), (4, 7), (7, 9)]);
    // Two images next to each other may wrap apart.
    assert_eq!(lines(25), [(0, 2), (2, 4), (4, 5), (5, 7), (7, 9)]);
}
