use feathermd::{
    document::SourceLine,
    markdown,
    outline::Heading,
    toc::{self, Contents, Line},
};

/// Outline entries (Markdown level, title), 100 bytes apart.
fn entries(list: &[(u8, &str)]) -> Vec<Heading> {
    list.iter()
        .enumerate()
        .map(|(i, &(level, text))| Heading {
            offset: i as u64 * 100,
            level,
            text: text.into(),
            ..Heading::default()
        })
        .collect()
}
fn depths(list: &[(u8, &str)]) -> Vec<u8> {
    let mut e = entries(list);
    toc::assign_depths(&mut e);
    e.iter().map(|h| h.depth).collect()
}

#[test]
fn section_numbers_need_a_title_after_them() {
    for (text, number) in [
        ("1 供应链", Some(vec![1])),
        ("1.1 采购管理", Some(vec![1, 1])),
        ("1.1.12 评估等级", Some(vec![1, 1, 12])),
        ("1.对象整体属性", Some(vec![1])),
        ("1.1权限对象", Some(vec![1, 1])),
        ("2、总则", Some(vec![2])),
        ("3．简介", Some(vec![3])),
        ("  4 Usage", Some(vec![4])),
        ("2024 年度报告", None),
        ("1号线", None),
        ("3D 打印", None),
        ("1:30 meeting", None),
        ("1) Intro", None),
        ("42", None),
        ("v1.2 notes", None),
        ("1.2.3.4.5.6.7.8.9 deep", None),
    ] {
        assert_eq!(toc::section(text), number, "{text:?}");
    }
    for title in [
        "目录",
        "目 录",
        "目錄",
        "目次",
        "Contents",
        "Table of Contents",
        "TOC",
        "目录：",
    ] {
        assert!(toc::contents_title(title), "{title:?}");
    }
    for other in ["目录结构", "Contents of the box", "概述"] {
        assert!(!toc::contents_title(other), "{other:?}");
    }
}

#[test]
fn numbered_headings_at_one_level_nest_by_their_numbers() {
    // A converted specification: every heading is ####/#####, the hierarchy lives
    // in the numbers, and the contents title comes first.
    let spec = [
        (2, "目 录"),
        (4, "1 供应链"),
        (4, "1.1 采购管理"),
        (4, "1.1.1 联系人"),
        (4, "联系人-单据头"),
        (5, "概述"),
        (5, "字段"),
        (4, "联系人-单据头_L"),
        (5, "概述"),
        (4, "1.1.2 材料监控对象"),
        (4, "材料监控对象-单据头"),
        (4, "1.2 库存管理"),
        (4, "2 生产制造"),
    ];
    assert_eq!(depths(&spec), [1, 1, 2, 3, 4, 5, 5, 4, 5, 3, 4, 2, 1]);
    // A title, then contents and numbered chapters one level down.
    assert_eq!(
        depths(&[
            (1, "部门 设计规格说明书"),
            (1, "目 录"),
            (2, "1.对象整体属性"),
            (3, "1.1权限对象"),
            (2, "2.对象实体属性"),
        ]),
        [1, 2, 2, 3, 2]
    );
    // Plain Markdown: levels nest, skipped levels add nothing, a trailing contents
    // title is a sibling.
    assert_eq!(
        depths(&[
            (1, "Project"),
            (3, "Install"),
            (3, "Windows"),
            (2, "Usage"),
            (2, "Contents"),
        ]),
        [1, 2, 2, 2, 2]
    );
    // Numbers that do not extend each other stay siblings (a changelog).
    assert_eq!(
        depths(&[
            (1, "Changes"),
            (2, "1.0"),
            (2, "0.2.0 beta"),
            (2, "0.1.0 first")
        ]),
        [1, 2, 2, 2]
    );
    // An unnumbered heading belongs to the numbered one before it only where the
    // level numbers itself.
    assert_eq!(
        depths(&[(2, "1 Intro"), (2, "1.1 Scope"), (2, "Appendix")]),
        [1, 2, 3]
    );
    assert_eq!(
        depths(&[(2, "1 Intro"), (2, "2 Usage"), (2, "Appendix")]),
        [1, 1, 1]
    );
}

#[test]
fn folds_keep_the_outline_short_and_filters_keep_parents() {
    let mut e = entries(&[
        (1, "a"),
        (2, "a1"),
        (3, "a1x"),
        (3, "a1y"),
        (2, "a2"),
        (1, "b"),
        (2, "b1"),
    ]);
    toc::assign_depths(&mut e);
    assert!(toc::has_children(&e, 0) && toc::has_children(&e, 1));
    assert!(!toc::has_children(&e, 2) && !toc::has_children(&e, 6));
    // 2 entries at depth 1, 5 to depth 2, 7 to depth 3.
    assert_eq!(toc::fold_depth(&e, 7), 3);
    assert_eq!(toc::fold_depth(&e, 6), 2);
    assert_eq!(toc::fold_depth(&e, 1), 1);
    assert_eq!(
        toc::initial_folds(&e, 5),
        [false, true, false, false, false, false, false]
    );
    let lower: Vec<String> = e.iter().map(|h| h.text.to_lowercase()).collect();
    let listed = |folded: &[bool], words: &[&str]| -> Vec<(usize, bool)> {
        let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        toc::rows(&e, folded, &lower, &words)
            .into_iter()
            .map(|Line { entry, context }| (entry, context))
            .collect()
    };
    let open = vec![false; e.len()];
    assert_eq!(listed(&open, &[]).len(), 7);
    let mut folded = open.clone();
    folded[1] = true;
    let rows = toc::rows(&e, &folded, &lower, &[]);
    assert_eq!(
        rows.iter().map(|r| r.entry).collect::<Vec<_>>(),
        [0, 1, 4, 5, 6]
    );
    // A hidden entry is shown by its folded parent; a listed one by itself.
    assert_eq!(toc::active_row(&e, &rows, 3), Some(1));
    assert_eq!(toc::active_row(&e, &rows, 4), Some(2));
    // Filters ignore folds, list each needed parent once, and need every word.
    assert_eq!(listed(&folded, &["1y"]), [(0, true), (1, true), (3, false)]);
    assert_eq!(
        listed(&folded, &["1"]),
        [
            (0, true),
            (1, false),
            (2, false),
            (3, false),
            (5, true),
            (6, false)
        ]
    );
    assert_eq!(
        listed(&open, &["a", "y"]),
        [(0, true), (1, true), (3, false)]
    );
    assert!(listed(&open, &["zzz"]).is_empty());
}

#[test]
fn fragments_resolve_like_github_anchors() {
    assert_eq!(toc::slug("Getting Started!"), "getting-started");
    assert_eq!(toc::slug("1.1.1 联系人"), "111-联系人");
    assert_eq!(toc::slug("C++ & Rust_2"), "c--rust_2");
    let mut e = entries(&[
        (1, "Intro"),
        (2, "Intro"),
        (2, "1.1 采购管理"),
        (2, "Getting Started"),
    ]);
    toc::assign_depths(&mut e);
    let contents = Contents::new(&e);
    assert_eq!(contents.anchor(1), Some("intro-1"));
    assert_eq!(contents.resolve("#intro"), Some(0));
    assert_eq!(contents.resolve("#intro-1"), Some(1));
    assert_eq!(contents.resolve("#getting-started"), Some(3));
    assert_eq!(contents.resolve("#Getting-Started"), Some(3));
    assert_eq!(contents.resolve("#11-采购管理"), Some(2));
    assert_eq!(
        contents.resolve("#11-%E9%87%87%E8%B4%AD%E7%AE%A1%E7%90%86"),
        Some(2)
    );
    // The heading's own text, as some converters write it.
    assert_eq!(contents.resolve("#1.1 采购管理"), Some(2));
    assert_eq!(contents.resolve("#missing"), None);
    assert_eq!(contents.resolve("#"), None);
}

/// Preview blocks of Markdown `text`, lines 10 bytes apart from byte 10000.
fn blocks(text: &str) -> Vec<markdown::Block> {
    let lines: Vec<SourceLine> = text
        .lines()
        .enumerate()
        .map(|(i, line)| SourceLine {
            start: 10_000 + i as u64 * 10,
            end: 10_000 + i as u64 * 10 + 10,
            text: line.into(),
            continued: false,
        })
        .collect();
    markdown::blocks(&lines, false)
}
fn spec() -> Vec<Heading> {
    // The contents title sits before the list (offset 0), the headings after it.
    let mut e = entries(&[
        (2, "目 录"),
        (4, "1 供应链"),
        (4, "1.1 采购管理"),
        (4, "1.1.1 联系人"),
        (4, "1.1.2 材料监控对象"),
    ]);
    for heading in e.iter_mut().skip(1) {
        heading.offset += 20_000;
    }
    toc::assign_depths(&mut e);
    e
}

#[test]
fn flat_contents_lists_link_to_headings_and_indent_by_depth() {
    let e = spec();
    let contents = Contents::new(&e);
    let mut b =
        blocks("* 1 供应链\n* 1.1 采购管理\n* 1.1.1 联系人\n\n* 1.1.2 材料监控对象\n* 附录 A");
    let indents: Vec<i32> = b.iter().map(|x| x.indent).collect();
    toc::link(&mut b, &e, &contents);
    let linked: Vec<Option<&str>> = b
        .iter()
        .map(|x| x.spans.get(1).and_then(|s| s.link.as_deref()))
        .collect();
    assert_eq!(
        linked,
        [
            Some("#1-供应链"),
            Some("#11-采购管理"),
            Some("#111-联系人"),
            None,
            Some("#112-材料监控对象"),
            None
        ]
    );
    // The marker is not a link; items indent one step per level.
    assert!(
        b.iter()
            .all(|x| x.spans.first().is_none_or(|s| s.link.is_none()))
    );
    assert_eq!(
        b.iter().map(|x| x.indent).collect::<Vec<_>>(),
        [
            indents[0],
            indents[1] + 24,
            indents[2] + 48,
            indents[3],
            indents[4] + 48,
            indents[5]
        ]
    );
    assert_eq!(b[1].spans[0].text, "◦ ");
    assert_eq!(b[2].spans[0].text, "▪ ");
    // Each link resolves back to its heading.
    assert_eq!(contents.resolve("#111-联系人"), Some(3));
}

#[test]
fn numbered_headings_read_in_sizes_by_their_depth() {
    let mut e = entries(&[
        (2, "目 录"),
        (4, "1 供应链"),
        (4, "1.1 采购管理"),
        (4, "1.1.1 联系人"),
        (4, "联系人-单据头"),
        (5, "概述"),
    ]);
    assert!(toc::assign_depths(&mut e));
    let mut b: Vec<markdown::Block> = e
        .iter()
        .map(|h| {
            let line = SourceLine {
                start: h.offset,
                end: h.offset + 10,
                text: format!("{} {}", "#".repeat(h.level.into()), h.text),
                continued: false,
            };
            markdown::blocks(std::slice::from_ref(&line), false).remove(0)
        })
        .collect();
    toc::levels(&mut b, &e);
    use markdown::Kind::Heading;
    let kinds: Vec<markdown::Kind> = b.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [
            Heading(1),
            Heading(1),
            Heading(2),
            Heading(3),
            Heading(4),
            Heading(5)
        ]
    );
    // Plain Markdown outlines do not number their levels: sizes stay as written.
    let mut plain = entries(&[(1, "Title"), (2, "Usage")]);
    assert!(!toc::assign_depths(&mut plain));
}

#[test]
fn short_nested_or_linked_lists_are_left_alone() {
    let e = spec();
    let contents = Contents::new(&e);
    let mut two = blocks("* 1 供应链\n* 1.1 采购管理\n* something else");
    toc::link(&mut two, &e, &contents);
    assert!(two.iter().all(|x| x.spans.iter().all(|s| s.link.is_none())));
    // A hand-written nested list keeps its indents, but gets its links.
    let mut nested = blocks("* 1 供应链\n  * 1.1 采购管理\n    * 1.1.1 联系人");
    let indents: Vec<i32> = nested.iter().map(|x| x.indent).collect();
    toc::link(&mut nested, &e, &contents);
    assert_eq!(nested.iter().map(|x| x.indent).collect::<Vec<_>>(), indents);
    assert_eq!(nested[2].spans[1].link.as_deref(), Some("#111-联系人"));
    // Items that already link somewhere keep their own links.
    let mut linked = blocks("* [1 供应链](#x)\n* [1.1 采购管理](#y)\n* [1.1.1 联系人](#z)");
    toc::link(&mut linked, &e, &contents);
    assert_eq!(linked[0].spans[1].link.as_deref(), Some("#x"));
    // Without an outline nothing changes.
    let mut none = blocks("* 1 供应链\n* 1.1 采购管理\n* 1.1.1 联系人");
    toc::link(&mut none, &[], &Contents::default());
    assert!(
        none.iter()
            .all(|x| x.spans.iter().all(|s| s.link.is_none()))
    );
}
