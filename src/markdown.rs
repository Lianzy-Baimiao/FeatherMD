//! Window-local Markdown. Never execute HTML or fetch external resources.
use crate::document::SourceLine;
use crate::table::{self, Align};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Paragraph,
    Heading(u8),
    Code,
    Quote,
    Rule,
    Table,
    Fence,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Block {
    pub start: u64,
    pub end: u64,
    pub kind: Kind,
    pub spans: Vec<Span>,
    pub indent: i32,
    /// Cells when `kind` is `Table`; consecutive rows with the same `group` form one table.
    pub table: Option<TableRow>,
}
#[derive(Clone, Debug)]
pub struct TableRow {
    pub group: u64,
    pub header: bool,
    pub align: Vec<Align>,
    /// Exactly `align.len()` cells; missing cells are empty, extra cells are dropped.
    pub cells: Vec<Vec<Span>>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Fence {
    marker: u8,
    len: usize,
}
impl Fence {
    pub fn is_open(self) -> bool {
        self.len > 0
    }
    pub fn feed(&mut self, text: &str) -> bool {
        let trim = text.trim_start_matches(' ');
        if text.len() - trim.len() > 3 {
            return false;
        }
        let bytes = trim.as_bytes();
        let Some(&marker) = bytes.first() else {
            return false;
        };
        if !matches!(marker, b'`' | b'~') {
            return false;
        }
        let n = bytes.iter().take_while(|&&b| b == marker).count();
        if n < 3 {
            return false;
        }
        if self.is_open() {
            if marker != self.marker || n < self.len || !trim[n..].trim().is_empty() {
                return false;
            }
            *self = Self::default();
        } else {
            if marker == b'`' && trim[n..].contains('`') {
                return false;
            }
            *self = Self { marker, len: n };
        }
        true
    }
}
fn plain(text: &str) -> Vec<Span> {
    vec![Span {
        text: text.to_owned(),
        ..Span::default()
    }]
}
/// Starts a block other than a paragraph, so it cannot be a table row.
fn block_start(text: &str) -> bool {
    let t = text.trim_start();
    if text.len() - t.len() > 3 {
        return true;
    }
    let hashes = t.bytes().take_while(|&b| b == b'#').count();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    t.starts_with('>')
        || ((1..=6).contains(&hashes) && t.as_bytes().get(hashes).is_none_or(|b| *b == b' '))
        || ["- ", "* ", "+ "].iter().any(|m| t.starts_with(m))
        || (digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")))
        || (t.len() >= 3
            && ['-', '*', '_']
                .iter()
                .any(|&m| t.chars().all(|c| c == m || c == ' ')))
        || Fence::default().feed(text)
}
/// Column alignments when `lines[i]` and `lines[i + 1]` start a table.
fn table_start(lines: &[SourceLine], i: usize) -> Option<Vec<Align>> {
    let (head, rule) = (lines.get(i)?, lines.get(i + 1)?);
    if head.continued || rule.continued || block_start(&head.text) || !table::has_pipe(&head.text) {
        return None;
    }
    let align = table::delimiter(&rule.text)?;
    (table::split_row(&head.text).len() == align.len()).then_some(align)
}
/// A non-blank line that starts no other block continues the table above it.
fn continues_table(line: &SourceLine) -> bool {
    !line.continued && !line.text.trim().is_empty() && !block_start(&line.text)
}
/// Keeps cell text from being read as a heading, list, quote, rule or fence.
fn cell_spans(text: &str) -> Vec<Span> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let rest = text.get(1..).unwrap_or("");
    let marker = |m: char| text.starts_with(m) && (rest.is_empty() || rest.starts_with(' '));
    let rule = text.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|&m| text.chars().all(|c| c == m || c == ' '));
    if text.starts_with(['#', '>'])
        || marker('-')
        || marker('*')
        || marker('+')
        || rule
        || text.starts_with("```")
        || text.starts_with("~~~")
    {
        inline(&format!("\\{text}"))
    } else if digits > 0 && digits <= 9 && text[digits..].starts_with(['.', ')']) {
        inline(&format!("{}\\{}", &text[..digits], &text[digits..]))
    } else {
        inline(text)
    }
}
fn table_row(line: &SourceLine, end: u64, group: u64, header: bool, align: &[Align]) -> Block {
    let mut cells: Vec<Vec<Span>> = table::split_row(&line.text)
        .iter()
        .take(align.len())
        .map(|cell| cell_spans(cell))
        .collect();
    cells.resize(align.len(), Vec::new());
    Block {
        start: line.start,
        end,
        kind: Kind::Table,
        spans: Vec::new(),
        indent: 0,
        table: Some(TableRow {
            group,
            header,
            align: align.to_vec(),
            cells,
        }),
    }
}
pub fn blocks(lines: &[SourceLine], raw: bool) -> Vec<Block> {
    let mut fence = Fence::default();
    let mut continued = false;
    let mut out = Vec::with_capacity(lines.len());
    // Columns and group of the table being continued.
    let mut table: Option<(Vec<Align>, u64)> = None;
    let mut i = 0;
    // A page that starts inside a table (its header is above the window) keeps the
    // rows starting with `|` as one table without a header row.
    if !raw && lines.first().is_some_and(|l| l.start > 3) {
        let mut body = 0;
        let mut align = None;
        if let Some(rule) = lines.first().and_then(|l| table::delimiter(&l.text))
            && lines.get(1).is_some_and(continues_table)
        {
            align = Some(rule);
            body = 1;
        }
        let run = lines[body..]
            .iter()
            .take_while(|l| continues_table(l) && l.text.trim_start().starts_with('|'))
            .count();
        if run > 0 {
            let columns = align.as_ref().map_or_else(
                || {
                    lines[body..body + run]
                        .iter()
                        .map(|l| table::split_row(&l.text).len())
                        .max()
                        .unwrap_or(1)
                        .min(table::MAX_COLUMNS)
                },
                Vec::len,
            );
            let align = align.unwrap_or_else(|| vec![Align::Left; columns]);
            let group = lines[0].start;
            for (n, line) in lines[body..body + run].iter().enumerate() {
                // The delimiter row belongs to the first body row.
                let mut block = table_row(line, line.end, group, false, &align);
                if n == 0 {
                    block.start = lines[0].start;
                }
                out.push(block);
            }
            table = Some((align, group));
            i = body + run;
        }
    }
    while i < lines.len() {
        let line = &lines[i];
        if !raw && !fence.is_open() && !continued {
            if let Some((align, group)) = &table
                && continues_table(line)
            {
                out.push(table_row(line, line.end, *group, false, align));
                i += 1;
                continue;
            }
            table = None;
            if let Some(align) = table_start(lines, i) {
                // The delimiter row is part of the header row's range.
                out.push(table_row(line, lines[i + 1].end, line.start, true, &align));
                table = Some((align, line.start));
                i += 2;
                continue;
            }
        }
        out.push(line_block(line, raw, &mut fence, &mut continued));
        i += 1;
    }
    out
}
/// What a boxed run of blocks holds, which decides what its copy button copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    /// Fenced code: the lines between the fences, without them.
    Fenced,
    /// Indented code: its lines with one level of indentation removed.
    Indented,
    Quote,
    Table,
}
/// Consecutive blocks of a preview page drawn as one box (code, quote or table).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxRange {
    pub kind: BoxKind,
    /// Inclusive block indices of the whole box, fences included.
    pub first: usize,
    pub last: usize,
    /// Inclusive block indices to copy; None when there is nothing to copy (an
    /// empty fence).
    pub content: Option<(usize, usize)>,
    /// The box runs past the loaded page, so a copy holds only its loaded part.
    pub partial: bool,
}
/// Boxes of preview blocks from `blocks`. `before` and `after` say whether the file
/// has bytes outside the page, so a box touching that edge may be cut off.
pub fn boxes(blocks: &[Block], before: bool, after: bool) -> Vec<BoxRange> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < blocks.len() {
        let block = &blocks[i];
        let (kind, last) = match block.kind {
            // Fence blocks alternate opening and closing; everything between them is
            // Code, so the box ends at the next Fence block or at the page end.
            Kind::Fence => {
                let close = blocks[i + 1..].iter().position(|b| b.kind == Kind::Fence);
                (
                    BoxKind::Fenced,
                    close.map_or(blocks.len() - 1, |n| i + 1 + n),
                )
            }
            Kind::Code => (BoxKind::Indented, run(blocks, i, |b| b.kind == Kind::Code)),
            Kind::Quote => (BoxKind::Quote, run(blocks, i, |b| b.kind == Kind::Quote)),
            Kind::Table => {
                let group = block.table.as_ref().map(|t| t.group);
                (
                    BoxKind::Table,
                    run(blocks, i, |b| b.table.as_ref().map(|t| t.group) == group),
                )
            }
            _ => {
                i += 1;
                continue;
            }
        };
        let closed = kind == BoxKind::Fenced && last > i && blocks[last].kind == Kind::Fence;
        let content = match kind {
            BoxKind::Fenced => {
                let end = if closed { last - 1 } else { last };
                (i < end).then_some((i + 1, end))
            }
            _ => Some((i, last)),
        };
        out.push(BoxRange {
            kind,
            first: i,
            last,
            content,
            // An opening fence on the page shows where its box starts.
            partial: (i == 0 && before && kind != BoxKind::Fenced)
                || (last == blocks.len() - 1 && after && !closed),
        });
        i = last + 1;
    }
    out
}
/// Last index of the run of blocks from `from` that satisfy `same`.
fn run(blocks: &[Block], from: usize, same: impl Fn(&Block) -> bool) -> usize {
    from + blocks[from..].iter().take_while(|b| same(b)).count() - 1
}
fn line_block(line: &SourceLine, raw: bool, fence: &mut Fence, continued: &mut bool) -> Block {
    let text = line.text.as_str();
    let mut kind = Kind::Paragraph;
    let mut indent = 0;
    let spans = if raw {
        kind = Kind::Code;
        plain(text)
    } else if !*continued && fence.feed(text) {
        kind = Kind::Fence;
        plain(if fence.is_open() {
            text.trim().trim_start_matches(['`', '~']).trim()
        } else {
            ""
        })
    } else if fence.is_open() || *continued {
        kind = Kind::Code;
        plain(text)
    } else {
        let t = text.trim_start();
        let hashes = t.bytes().take_while(|&b| b == b'#').count();
        let mut content = text;
        if (1..=6).contains(&hashes)
            && t.as_bytes()
                .get(hashes)
                .is_some_and(u8::is_ascii_whitespace)
        {
            kind = Kind::Heading(hashes as u8);
            content = t[hashes..].trim();
        } else if t.starts_with('>') {
            kind = Kind::Quote;
            content = t.trim_start_matches('>').trim_start();
            indent = 16;
        } else if t.len() >= 3
            && (t.chars().all(|c| c == '-' || c == ' ')
                || t.chars().all(|c| c == '*' || c == ' ')
                || t.chars().all(|c| c == '_' || c == ' '))
        {
            kind = Kind::Rule;
        } else if text.starts_with("    ") || text.starts_with('\t') {
            kind = Kind::Code;
        }
        if kind == Kind::Code {
            plain(content)
        } else if kind == Kind::Rule {
            vec![]
        } else {
            let mut prefix = None;
            if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
                prefix = Some("• ".to_owned());
                content = &t[2..];
                indent += 14;
            } else {
                let digits = t.bytes().take_while(u8::is_ascii_digit).count();
                if digits > 0
                    && digits <= 9
                    && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))
                {
                    prefix = Some(t[..digits + 2].to_owned());
                    content = &t[digits + 2..];
                    indent += 14;
                }
            }
            if let Some(rest) = content.strip_prefix("[ ] ") {
                prefix = Some("☐ ".into());
                content = rest;
            } else if let Some(rest) = content
                .strip_prefix("[x] ")
                .or_else(|| content.strip_prefix("[X] "))
            {
                prefix = Some("☑ ".into());
                content = rest;
            }
            let mut spans = inline(content);
            if let Some(b) = prefix {
                spans.insert(
                    0,
                    Span {
                        text: b,
                        ..Span::default()
                    },
                );
            }
            spans
        }
    };
    *continued = line.continued;
    Block {
        start: line.start,
        end: line.end,
        kind,
        spans,
        indent,
        table: None,
    }
}
pub fn inline(text: &str) -> Vec<Span> {
    let mut style = Span::default();
    let mut out = Vec::new();
    let mut bold = 0;
    let mut italic = 0;
    let mut strike = 0;
    for event in Parser::new_ext(
        text,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    ) {
        match event {
            Event::Start(Tag::Strong) => {
                bold += 1;
                style.bold = true;
            }
            Event::End(TagEnd::Strong) => {
                bold -= 1;
                style.bold = bold > 0;
            }
            Event::Start(Tag::Emphasis) => {
                italic += 1;
                style.italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                italic -= 1;
                style.italic = italic > 0;
            }
            Event::Start(Tag::Strikethrough) => {
                strike += 1;
                style.strike = true;
            }
            Event::End(TagEnd::Strikethrough) => {
                strike -= 1;
                style.strike = strike > 0;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                style.link = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                style.link = None;
            }
            Event::Start(Tag::Image { .. }) => {
                out.push(Span {
                    text: crate::i18n::tr("[图片: ").into(),
                    ..style.clone()
                });
            }
            Event::End(TagEnd::Image) => {
                out.push(Span {
                    text: "]".into(),
                    ..style.clone()
                });
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => {
                out.push(Span {
                    text: t.to_string(),
                    ..style.clone()
                });
            }
            Event::Code(t) => {
                out.push(Span {
                    text: t.to_string(),
                    code: true,
                    ..style.clone()
                });
            }
            Event::SoftBreak | Event::HardBreak => out.push(Span {
                text: " ".into(),
                ..style.clone()
            }),
            Event::TaskListMarker(done) => out.push(Span {
                text: if done { "☑ " } else { "☐ " }.into(),
                ..style.clone()
            }),
            _ => {}
        }
    }
    if out.is_empty() && !text.is_empty() {
        return plain(text);
    }
    out
}
pub fn safe_web_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !url.chars().any(char::is_control)
}
