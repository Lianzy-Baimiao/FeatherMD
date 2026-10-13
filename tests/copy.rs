//! Copy buttons: which preview blocks form one box and what each box copies.
use feathermd::{
    document::SourceLine,
    markdown::{self, BoxKind, BoxRange},
    text,
};

/// Loaded lines starting at byte `start`, each ending with `\n` unless `continued`.
fn lines(start: u64, texts: &[(&str, bool)]) -> Vec<SourceLine> {
    let mut at = start;
    texts
        .iter()
        .map(|&(text, continued)| {
            let end = at + text.len() as u64 + u64::from(!continued);
            let line = SourceLine {
                start: at,
                end,
                text: text.into(),
                continued,
            };
            at = end;
            line
        })
        .collect()
}
fn plain(start: u64, texts: &[&str]) -> Vec<SourceLine> {
    lines(
        start,
        &texts.iter().map(|t| (*t, false)).collect::<Vec<_>>(),
    )
}
fn boxed(kind: BoxKind, first: usize, last: usize, content: Option<(usize, usize)>) -> BoxRange {
    BoxRange {
        kind,
        first,
        last,
        content,
        partial: false,
    }
}

#[test]
fn code_quotes_and_tables_each_form_one_box() {
    let page = plain(
        0,
        &[
            "# Title",
            "```rust",
            "fn main() {",
            "",
            "    println!(\"hi\");",
            "}",
            "```",
            "",
            "    indented",
            "\tsecond",
            "",
            "> quoted",
            "> more",
            "",
            "| a | b |",
            "| - | - |",
            "| 1 | 2 |",
            "",
            "```",
            "```",
            "~~~",
            "```",
            "~~~",
            "> ```",
        ],
    );
    let blocks = markdown::blocks(&page, false);
    // The delimiter row belongs to the header block, so later indices shift by one.
    assert_eq!(
        markdown::boxes(&blocks, false, false),
        [
            boxed(BoxKind::Fenced, 1, 6, Some((2, 5))),
            boxed(BoxKind::Indented, 8, 9, Some((8, 9))),
            boxed(BoxKind::Quote, 11, 12, Some((11, 12))),
            boxed(BoxKind::Table, 14, 15, Some((14, 15))),
            // An empty fence has nothing to copy.
            boxed(BoxKind::Fenced, 17, 18, None),
            // A ``` line inside a ~~~ fence is code, not a closing fence.
            boxed(BoxKind::Fenced, 19, 21, Some((20, 20))),
            // A fence marker after `>` is quoted text.
            boxed(BoxKind::Quote, 22, 22, Some((22, 22))),
        ]
    );
    let copy = |(a, b): (usize, usize), dedent| {
        text::source(&page, blocks[a].start, blocks[b].start, dedent)
    };
    assert_eq!(
        copy((2, 5), false),
        "fn main() {\r\n\r\n    println!(\"hi\");\r\n}"
    );
    assert_eq!(copy((8, 9), true), "indented\r\nsecond");
    assert_eq!(copy((20, 20), false), "```");
}

#[test]
fn boxes_cut_off_by_the_page_edge_are_partial() {
    let partial = |b: &BoxRange| b.partial;
    // An unterminated fence copies to the page end; it is partial only when the
    // file continues.
    let open = plain(0, &["text", "```", "a", "b"]);
    let blocks = markdown::blocks(&open, false);
    for after in [false, true] {
        let boxes = markdown::boxes(&blocks, false, after);
        assert_eq!(boxes.len(), 1);
        assert_eq!((boxes[0].first, boxes[0].last), (1, 3));
        assert_eq!(boxes[0].content, Some((2, 3)));
        assert_eq!(partial(&boxes[0]), after);
    }
    // A closed fence at the page end is complete even when the file continues.
    let closed = plain(0, &["```", "a", "```"]);
    let boxes = markdown::boxes(&markdown::blocks(&closed, false), true, true);
    assert!(!partial(&boxes[0]));
    // A quote or table touching the top of a later page may have started above it.
    let quote = plain(100, &["> a", "> b", "", "text"]);
    let blocks = markdown::blocks(&quote, false);
    assert!(partial(&markdown::boxes(&blocks, true, false)[0]));
    assert!(!partial(&markdown::boxes(&blocks, false, false)[0]));
    // A page that starts inside a table keeps its rows as one headless table.
    let headless = plain(200, &["| 1 | 2 |", "| 3 | 4 |", "", "text"]);
    let blocks = markdown::blocks(&headless, false);
    let boxes = markdown::boxes(&blocks, true, false);
    assert_eq!(
        (boxes[0].kind, boxes[0].first, boxes[0].last),
        (BoxKind::Table, 0, 1)
    );
    assert!(partial(&boxes[0]));
    // A table running to the bottom of the page continues below it.
    let table = plain(0, &["text", "| a |", "| - |", "| 1 |"]);
    let blocks = markdown::blocks(&table, false);
    assert!(partial(&markdown::boxes(&blocks, false, true)[0]));
    assert!(!partial(&markdown::boxes(&blocks, false, false)[0]));
}

#[test]
fn copied_source_keeps_long_lines_whole_and_removes_one_indent() {
    let page = lines(
        0,
        &[
            ("    ab", true),
            ("cd", false),
            ("  x", false),
            ("  \tz", false),
            ("\t\tdeep", false),
            ("      six", false),
            ("", false),
            ("tail", false),
        ],
    );
    let all = |dedent| text::source(&page, 0, page[7].start, dedent);
    // The second segment continues the first line: no break and no dedent inside.
    assert_eq!(all(true), "abcd\r\nx\r\nz\r\n\tdeep\r\n  six\r\n\r\ntail");
    assert_eq!(
        all(false),
        "    abcd\r\n  x\r\n  \tz\r\n\t\tdeep\r\n      six\r\n\r\ntail"
    );
    // Only lines starting inside the byte range are copied.
    assert_eq!(
        text::source(&page, page[2].start, page[3].start, false),
        "  x\r\n  \tz"
    );
    assert_eq!(text::source(&page, page[7].end, u64::MAX, false), "");
}
