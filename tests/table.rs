use feathermd::{
    document::SourceLine,
    markdown::{self, Kind},
    table::{self, Align},
    text::{self, Caret},
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
fn plain(texts: &[&str]) -> Vec<SourceLine> {
    lines(0, &texts.iter().map(|t| (*t, false)).collect::<Vec<_>>())
}
fn cell_text(spans: &[markdown::Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}
/// Uniform 10 px pen positions for ASCII tests.
fn pens(units: &[u16]) -> Vec<i32> {
    (1..=units.len() as i32).map(|i| i * 10).collect()
}
fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn rows_split_on_unescaped_pipes_with_optional_outer_pipes() {
    assert_eq!(table::split_row("| a | b |"), ["a", "b"]);
    assert_eq!(table::split_row("a | b"), ["a", "b"]);
    assert_eq!(table::split_row("| a |  |"), ["a", ""]);
    assert_eq!(table::split_row(r"| a \| b | c |"), ["a | b", "c"]);
    assert_eq!(table::split_row("|"), [""]);
    assert_eq!(table::split_row("||"), [""]);
    assert_eq!(table::split_row("a"), ["a"]);
    assert!(table::has_pipe("a | b"));
    assert!(!table::has_pipe(r"a \| b"));
    assert!(!table::has_pipe("plain text"));
}

#[test]
fn delimiter_rows_give_column_alignment() {
    assert_eq!(
        table::delimiter("| :-- | :-: | --: | --- |"),
        Some(vec![Align::Left, Align::Center, Align::Right, Align::Left])
    );
    assert_eq!(
        table::delimiter("-|-"),
        Some(vec![Align::Left, Align::Left])
    );
    for bad in [
        "---", "| a |", "| - - |", "| :-:- |", "| : |", "| :: |", "|  |",
    ] {
        assert_eq!(table::delimiter(bad), None, "{bad}");
    }
    let wide = format!("|{}", "-|".repeat(table::MAX_COLUMNS + 1));
    assert_eq!(table::delimiter(&wide), None);
}

#[test]
fn columns_keep_natural_widths_narrow_the_widest_first_or_scroll() {
    assert_eq!(table::fit_columns(&[100, 50], &[40, 20], 400), [100, 50]);
    // Only the long column gives up width; the short one never wraps.
    assert_eq!(table::fit_columns(&[100, 300], &[50, 60], 250), [100, 150]);
    // Two long columns share a common width above their minimums.
    assert_eq!(
        table::fit_columns(&[120, 400, 80], &[60, 90, 40], 300),
        [110, 110, 80]
    );
    // A column keeps its minimum even when the common width would be narrower.
    assert_eq!(table::fit_columns(&[200, 200], &[150, 20], 250), [150, 100]);
    // Leftover pixels go to columns at the common width, left first.
    assert_eq!(
        table::fit_columns(&[100, 100, 100], &[0, 0, 0], 250),
        [84, 83, 83]
    );
    // Even the minimums do not fit: columns stay readable and the table scrolls.
    assert_eq!(table::fit_columns(&[100, 300], &[80, 90], 100), [80, 90]);
    // A minimum larger than the content never widens a column.
    assert_eq!(table::fit_columns(&[30, 400], &[90, 50], 200), [30, 170]);
    assert!(table::fit_columns(&[], &[], 100).is_empty());
}

#[test]
fn cell_lines_break_at_spaces_and_cjk_and_keep_surrogate_pairs() {
    let ab = utf16("ab cd");
    assert_eq!(table::break_lines(&ab, &pens(&ab), 30), [(0, 2), (3, 5)]);
    assert_eq!(table::break_lines(&ab, &pens(&ab), 100), [(0, 5)]);
    let long = utf16("abcdef");
    assert_eq!(
        table::break_lines(&long, &pens(&long), 25),
        [(0, 2), (2, 4), (4, 6)]
    );
    let cjk = utf16("中文字");
    assert_eq!(table::break_lines(&cjk, &pens(&cjk), 25), [(0, 2), (2, 3)]);
    // CJK text may break before Latin letters; "xyab" would be split mid-word.
    let mixed = utf16("中文ab");
    assert_eq!(
        table::break_lines(&mixed, &pens(&mixed), 35),
        [(0, 2), (2, 4)]
    );
    let latin = utf16("xyab");
    assert_eq!(
        table::break_lines(&latin, &pens(&latin), 35),
        [(0, 3), (3, 4)]
    );
    // Closing punctuation never starts a line and opening punctuation never ends one.
    let comma = utf16("中文，字");
    assert_eq!(
        table::break_lines(&comma, &pens(&comma), 25),
        [(0, 1), (1, 3), (3, 4)]
    );
    let bracket = utf16("中（文");
    assert_eq!(
        table::break_lines(&bracket, &pens(&bracket), 25),
        [(0, 1), (1, 3)]
    );
    assert_eq!(table::widest_token(&comma, &pens(&comma)), 20);
    let emoji = utf16("a😀b");
    assert_eq!(emoji.len(), 4);
    let ends = [10, 20, 20, 30];
    assert_eq!(
        table::break_lines(&emoji, &ends, 15),
        [(0, 1), (1, 3), (3, 4)]
    );
    let lone = utf16("😀b");
    assert_eq!(table::break_lines(&lone, &[10, 10, 20], 5)[0], (0, 2));
    assert!(table::break_lines(&utf16("   "), &[10, 20, 30], 100).is_empty());
    let words = utf16("ab cdef g");
    assert_eq!(table::widest_token(&words, &pens(&words)), 40);
    assert_eq!(table::widest_token(&cjk, &pens(&cjk)), 10);
    assert_eq!(table::widest_token(&[], &[]), 0);
}

#[test]
fn tables_need_a_header_and_delimiter_row_and_end_at_other_blocks() {
    let page = plain(&[
        "| Name | Size |",
        "| :--- | ---: |",
        "| a | 1 |",
        "b | 2 | extra",
        "| c |",
        "",
        "| not | a table |",
        "after",
    ]);
    let blocks = markdown::blocks(&page, false);
    let table: Vec<_> = blocks.iter().filter(|b| b.kind == Kind::Table).collect();
    assert_eq!(table.len(), 4);
    let header = table[0].table.as_ref().unwrap();
    assert!(header.header);
    assert_eq!(header.align, [Align::Left, Align::Right]);
    // The delimiter row belongs to the header row's byte range.
    assert_eq!((table[0].start, table[0].end), (page[0].start, page[1].end));
    let body: Vec<Vec<String>> = table[1..]
        .iter()
        .map(|b| {
            let row = b.table.as_ref().unwrap();
            assert!(!row.header);
            assert_eq!(row.group, page[0].start);
            row.cells.iter().map(|c| cell_text(c)).collect()
        })
        .collect();
    assert_eq!(body, [["a", "1"], ["b", "2"], ["c", ""]]);
    // A `|` line without a delimiter row is an ordinary paragraph.
    let rest = &blocks[blocks.len() - 3..];
    assert_eq!(rest[1].kind, Kind::Paragraph);
    assert_eq!(cell_text(&rest[1].spans), "| not | a table |");
    assert!(rest.iter().all(|b| b.table.is_none()));
}

#[test]
fn tables_stop_before_quotes_lists_and_headings_and_skip_code() {
    let page = plain(&[
        "a | b",
        "--|--",
        "> quote",
        "",
        "```",
        "| x | y |",
        "|---|---|",
        "```",
        "| h | i |",
        "|---|---|",
        "- item",
        "# heading",
    ]);
    let kinds: Vec<Kind> = markdown::blocks(&page, false)
        .iter()
        .map(|b| b.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            Kind::Table,
            Kind::Quote,
            Kind::Paragraph,
            Kind::Fence,
            Kind::Code,
            Kind::Code,
            Kind::Fence,
            Kind::Table,
            Kind::Paragraph,
            Kind::Heading(1),
        ]
    );
    // Source mode never builds tables.
    assert!(
        markdown::blocks(&page, true)
            .iter()
            .all(|b| b.kind == Kind::Code && b.table.is_none())
    );
    // Header and delimiter cell counts must match.
    let uneven = plain(&["| a | b |", "|---|", "| c | d |"]);
    assert!(
        markdown::blocks(&uneven, false)
            .iter()
            .all(|b| b.kind != Kind::Table)
    );
}

#[test]
fn cell_text_stays_inline_and_keeps_formatting() {
    let page = plain(&[
        r"| 1. first | # hash | - | **bold** x | `a\|b` |",
        "|---|---|---|---|---|",
    ]);
    let blocks = markdown::blocks(&page, false);
    let cells = &blocks[0].table.as_ref().unwrap().cells;
    assert_eq!(cell_text(&cells[0]), "1. first");
    assert_eq!(cell_text(&cells[1]), "# hash");
    assert_eq!(cell_text(&cells[2]), "-");
    assert!(cells[3].iter().any(|s| s.text == "bold" && s.bold));
    // `\|` keeps a pipe inside a code span.
    assert!(cells[4].iter().any(|s| s.code && s.text == "a|b"));
    // As in GFM, an unescaped pipe splits cells even inside backticks.
    let split = plain(&["| `a|b` |", "|---|"]);
    assert!(
        markdown::blocks(&split, false)
            .iter()
            .all(|b| b.table.is_none())
    );
}

#[test]
fn a_page_starting_inside_a_table_keeps_its_rows_as_a_table() {
    let page = lines(
        40_000,
        &[
            ("| --- | :-: |", false),
            ("| a | b |", false),
            ("| c | d | e |", false),
            ("", false),
            ("| x |", false),
        ],
    );
    let blocks = markdown::blocks(&page, false);
    let rows: Vec<_> = blocks.iter().filter_map(|b| b.table.as_ref()).collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].align, [Align::Left, Align::Center]);
    assert!(rows.iter().all(|r| !r.header && r.group == page[0].start));
    // The delimiter row belongs to the first body row.
    assert_eq!(blocks[0].start, page[0].start);
    assert_eq!(blocks[0].end, page[1].end);
    assert_eq!(blocks.last().unwrap().kind, Kind::Paragraph);
    // At the start of a file a leading `|` line is not a table on its own.
    assert!(
        markdown::blocks(&plain(&["| a | b |", "text"]), false)
            .iter()
            .all(|b| b.table.is_none())
    );
}

#[test]
fn displayed_units_map_back_to_source_bytes() {
    assert_eq!(text::display("a\tb\u{7}"), "a    b\u{fffd}");
    let line = "a\t😀b";
    assert_eq!(
        [0, 1, 2, 3, 6, 7].map(|i| text::units_before(line, i)),
        [0, 1, 5, 5, 7, 8]
    );
    assert_eq!(
        [0, 1, 4, 5, 6, 7, 8].map(|u| text::index_at_unit(line, u)),
        [0, 1, 1, 2, 2, 6, 7]
    );
    assert_eq!(text::word_at("foo bar, 中文", 1), (0, 3));
    assert_eq!(text::word_at("foo bar, 中文", 3), (3, 4));
    assert_eq!(text::word_at("foo bar, 中文", 7), (7, 8));
    assert_eq!(text::word_at("foo bar, 中文", 10), (9, 15));
    assert_eq!(text::word_at("abc", 3), (3, 3));
}

#[test]
fn selections_copy_exact_source_with_crlf_between_lines() {
    let page = lines(
        0,
        &[("abc", true), ("def", false), ("g\th", false), ("", false)],
    );
    let at = |line: usize, index: usize| Caret {
        line: page[line].start,
        index,
    };
    // A long line split into segments has no break; real lines get CRLF; tabs survive.
    assert_eq!(
        text::selected(&page, at(0, 1), at(2, 2)),
        ("bcdef\r\ng\t".to_owned(), false)
    );
    assert_eq!(
        text::selected(&page, at(2, 2), at(0, 1)),
        text::selected(&page, at(0, 1), at(2, 2))
    );
    assert_eq!(text::selected(&page, at(1, 1), at(1, 1)).0, "");
    assert_eq!(text::selected(&page, at(2, 0), at(3, 0)).0, "g\th\r\n");
    // Carets outside the loaded lines copy what is loaded and say so.
    let outside = Caret {
        line: 999,
        index: 0,
    };
    assert_eq!(
        text::selected(&page, at(2, 2), outside),
        ("h\r\n".to_owned(), true)
    );
    // An index inside a multi-byte character is floored to a boundary.
    let cjk = lines(0, &[("中文", false)]);
    let c = |index| Caret { line: 0, index };
    assert_eq!(text::selected(&cjk, c(1), c(4)).0, "中");
}
