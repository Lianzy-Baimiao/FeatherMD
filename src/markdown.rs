//! Window-local Markdown with a safe HTML subset. Never execute HTML or fetch
//! external resources.
use crate::document::SourceLine;
use crate::highlight::{self, Syntax, Token};
use crate::html::{self, Html};
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
    /// A line of only HTML tags, comments or hidden content: a thin gap.
    Spacer,
    /// A line of plain text, as written (leading spaces kept).
    Text,
    /// A log line: monospace, leading spaces kept, timestamp and level coloured.
    Log,
}
/// An image reference. FeatherMD shows local files only, and only when the
/// reader enables images; otherwise the alternative text stands in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub src: String,
    /// Size attributes of an HTML `<img>`, in CSS pixels.
    pub width: Option<u32>,
    pub height: Option<u32>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
    pub underline: bool,
    /// A keyboard key (`<kbd>`).
    pub kbd: bool,
    /// Highlighted text (`<mark>`).
    pub mark: bool,
    /// 1 superscript, -1 subscript.
    pub script: i8,
    /// Colour of a piece of highlighted code.
    pub token: Option<Token>,
    /// An image; `text` is its alternative text.
    pub image: Option<Image>,
    /// A task checkbox and whether it is done; `text` is its copy text.
    pub check: Option<bool>,
    /// The open marker of a `<summary>`; `text` is its copy text.
    pub disclosure: bool,
    /// Ends the visual line here (`<br>`).
    pub line_break: bool,
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
    pub align: Align,
    /// The first span is a list marker: wrapped lines align after it.
    pub marker: bool,
    /// Left edge of the quote bar in logical pixels, when the block is quoted.
    pub bar: Option<i32>,
}
#[derive(Clone, Debug)]
pub struct TableRow {
    pub group: u64,
    pub header: bool,
    pub align: Vec<Align>,
    /// Exactly `align.len()` cells; missing cells are empty, extra cells are dropped.
    pub cells: Vec<Vec<Span>>,
}
/// Text shown for an image that is not drawn: its alternative text (else its file
/// name) and, when images are on, why it is not shown.
pub fn image_placeholder(image: &Image, alt: &str, reason: Option<&str>) -> String {
    let name = crate::image::label(alt, &image.src);
    // A long alternative text stays one short chip.
    let name: String = if name.chars().count() > 60 {
        name.chars().take(59).chain(['…']).collect()
    } else {
        name
    };
    match reason {
        Some(reason) => format!("{}{name} · {reason}]", crate::i18n::tr("[图片: ")),
        None => format!("{}{name}]", crate::i18n::tr("[图片: ")),
    }
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
        // HTML blocks end a table too.
        || t.starts_with('<') && html::claims_start(text)
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
        align: Align::Left,
        marker: false,
        bar: None,
    }
}
pub fn blocks(lines: &[SourceLine], raw: bool) -> Vec<Block> {
    let mut state = Parse::default();
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
        // HTML blocks, then tables; neither inside code or a long line's tail.
        if !raw && !state.fence.is_open() && !state.continued {
            if state.html.claims(&line.text) {
                out.push(html_block(line, state.html.line(&line.text)));
                state.continued = line.continued;
                state.list = false;
                table = None;
                i += 1;
                continue;
            }
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
        let mut block = line_block(line, raw, &mut state);
        if !raw {
            // Markdown inside open HTML containers takes their alignment and indent.
            let context = state.html.context();
            block.indent += context.indent;
            if let Some(bar) = &mut block.bar {
                *bar += context.indent;
            }
            if block.align == Align::Left {
                block.align = context.align;
            }
            if context.quote && block.table.is_none() {
                block.bar = block.bar.or(Some(context.indent - 16));
                if block.kind == Kind::Paragraph {
                    block.kind = Kind::Quote;
                }
            }
        }
        out.push(block);
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
/// What one page's lines carry to the next line.
#[derive(Default)]
struct Parse {
    fence: Fence,
    /// Indent of the open fence's line, removed from its code lines.
    fence_indent: usize,
    syntax: Option<&'static Syntax>,
    colour: highlight::State,
    /// The previous line was a segment of a long line that continues.
    continued: bool,
    /// Inside a list: indented lines continue items instead of starting code.
    list: bool,
    /// Indents of the open list items, outermost first: their count is the depth.
    items: Vec<usize>,
    html: Html,
}
/// Leading spaces of `text`, a tab counting as four.
fn leading(text: &str) -> usize {
    let mut column = 0;
    for c in text.chars() {
        match c {
            ' ' => column += 1,
            '\t' => column += 4 - column % 4,
            _ => break,
        }
    }
    column
}
/// `text` without up to `columns` leading spaces.
fn strip_columns(text: &str, columns: usize) -> &str {
    let mut used = 0;
    for (i, c) in text.char_indices() {
        if used >= columns {
            return &text[i..];
        }
        match c {
            ' ' => used += 1,
            '\t' => used += 4 - used % 4,
            _ => return &text[i..],
        }
    }
    ""
}
/// One line of fenced code, coloured when its language is known.
fn code_spans(state: &mut Parse, text: &str) -> Vec<Span> {
    let Some(syntax) = state.syntax else {
        return plain(text);
    };
    let pieces = highlight::line(syntax, &mut state.colour, text);
    if pieces.is_empty() {
        return plain(text);
    }
    pieces
        .into_iter()
        .map(|(a, b, token)| Span {
            text: text[a..b].to_owned(),
            token,
            ..Span::default()
        })
        .collect()
}
pub(crate) fn block(line: &SourceLine, kind: Kind, spans: Vec<Span>, indent: i32) -> Block {
    Block {
        start: line.start,
        end: line.end,
        kind,
        spans,
        indent,
        table: None,
        align: Align::Left,
        marker: false,
        bar: None,
    }
}
/// A line drawn by the HTML path.
fn html_block(line: &SourceLine, drawn: html::Line) -> Block {
    let kind = match drawn.shape {
        html::Shape::Empty => Kind::Spacer,
        html::Shape::Paragraph => Kind::Paragraph,
        html::Shape::Heading(level) => Kind::Heading(level),
        html::Shape::Code => Kind::Code,
        html::Shape::Quote => Kind::Quote,
        html::Shape::Rule => Kind::Rule,
    };
    let mut spans = drawn.spans;
    if kind == Kind::Code {
        // Preformatted text keeps its spaces; inline tags only style it.
        for span in &mut spans {
            span.code = false;
        }
    }
    let mut out = block(line, kind, spans, drawn.context.indent);
    out.align = drawn.context.align;
    out.marker = drawn.marker;
    if drawn.context.quote && kind != Kind::Code {
        out.bar = Some(drawn.context.indent - 16);
    }
    out
}
fn line_block(line: &SourceLine, raw: bool, state: &mut Parse) -> Block {
    let text = line.text.as_str();
    if raw {
        state.continued = line.continued;
        return block(line, Kind::Code, plain(text), 0);
    }
    // A long line's later segments continue whatever its first segment was.
    if state.continued {
        state.continued = line.continued;
        let spans = if state.fence.is_open() {
            code_spans(state, text)
        } else {
            plain(text)
        };
        return block(line, Kind::Code, spans, 0);
    }
    state.continued = line.continued;
    let indent = leading(text);
    // A fence may be indented under a list item; its closing fence and code lines
    // lose the opening line's indent. Elsewhere four spaces still mean content.
    let probe = if state.fence.is_open() {
        strip_columns(text, state.fence_indent.min(indent))
    } else if state.list {
        strip_columns(text, indent.saturating_sub(3))
    } else {
        text
    };
    if state.fence.feed(probe) {
        let label = if state.fence.is_open() {
            let info = probe.trim().trim_start_matches(['`', '~']).trim();
            state.syntax = highlight::find(info);
            state.colour = highlight::State::default();
            // Code lines lose the fence's own indent (CommonMark), at most three
            // spaces at top level, the item's indent inside a list.
            state.fence_indent = indent;
            info.split_whitespace().next().unwrap_or("")
        } else {
            state.syntax = None;
            ""
        };
        return block(line, Kind::Fence, plain(label), 0);
    }
    if state.fence.is_open() {
        let content = strip_columns(text, state.fence_indent);
        let spans = code_spans(state, content);
        return block(line, Kind::Code, spans, 0);
    }
    let t = text.trim_start();
    if t.is_empty() {
        // Lists continue across blank lines until a line that is not indented.
        return block(line, Kind::Paragraph, Vec::new(), 0);
    }
    if indent >= 4 && !state.list {
        return block(line, Kind::Code, plain(strip_columns(text, 4)), 0);
    }
    // Quote markers, then the quoted content (which may hold a list or heading).
    let mut content = t;
    let mut quoted = false;
    while let Some(rest) = content.strip_prefix('>') {
        quoted = true;
        content = rest.strip_prefix(' ').unwrap_or(rest);
    }
    let mut kind = if quoted { Kind::Quote } else { Kind::Paragraph };
    let mut pad = if quoted { 18 } else { 0 };
    let hashes = content.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes)
        && content
            .as_bytes()
            .get(hashes)
            .is_none_or(u8::is_ascii_whitespace)
    {
        let mut title = content[hashes..].trim();
        // Optional closing sequence: `## Title ##`.
        let closed = title.trim_end_matches('#');
        if closed.is_empty() || closed.ends_with([' ', '\t']) {
            title = closed.trim_end();
        }
        state.list = false;
        state.items.clear();
        let mut out = block(line, Kind::Heading(hashes as u8), inline(title), pad);
        out.bar = quoted.then_some(0);
        return out;
    }
    let rule = |c: char| content.chars().all(|x| x == c || x == ' ' || x == '\t');
    if content.chars().filter(|c| !c.is_whitespace()).count() >= 3
        && (rule('-') || rule('*') || rule('_'))
    {
        state.list = false;
        state.items.clear();
        return block(line, Kind::Rule, Vec::new(), pad);
    }
    // List items: bullets change with depth; numbers keep their own text.
    let mut prefix = None;
    let mut check = None;
    let bullet = ["- ", "* ", "+ "].iter().any(|m| content.starts_with(m));
    let digits = content.bytes().take_while(u8::is_ascii_digit).count();
    let item = bullet
        || digits > 0
            && digits <= 9
            && (content[digits..].starts_with(". ") || content[digits..].starts_with(") "));
    // Depth follows the indents of the open items, whatever their width.
    if item {
        while state.items.last().is_some_and(|&open| open >= indent) {
            state.items.pop();
        }
        if state.items.len() < 8 {
            state.items.push(indent);
        }
    }
    let depth = state.items.len().saturating_sub(1) as i32;
    if bullet {
        prefix = Some(["• ", "◦ ", "▪ "][(depth as usize).min(2)].to_owned());
        content = &content[2..];
    } else if digits > 0
        && digits <= 9
        && (content[digits..].starts_with(". ") || content[digits..].starts_with(") "))
    {
        prefix = Some(content[..digits + 2].to_owned());
        content = &content[digits + 2..];
    }
    if prefix.is_some() {
        state.list = true;
        pad += 12 + depth * 24;
        // Task items replace their bullet with a checkbox.
        if let Some(rest) = content.strip_prefix("[ ] ") {
            check = Some(false);
            content = rest;
        } else if let Some(rest) = content
            .strip_prefix("[x] ")
            .or_else(|| content.strip_prefix("[X] "))
        {
            check = Some(true);
            content = rest;
        }
    } else if state.list && indent >= 2 {
        // A continuation paragraph of the item above.
        pad += 12 + depth * 24 + 16;
    } else {
        state.list = false;
        state.items.clear();
    }
    let mut spans = inline(content);
    let marker = prefix.is_some();
    if let Some(done) = check {
        spans.insert(
            0,
            Span {
                text: if done { "☑ " } else { "☐ " }.into(),
                check: Some(done),
                ..Span::default()
            },
        );
    } else if let Some(text) = prefix {
        spans.insert(
            0,
            Span {
                text,
                ..Span::default()
            },
        );
    }
    if quoted && kind == Kind::Paragraph {
        kind = Kind::Quote;
    }
    let mut out = block(line, kind, spans, pad);
    out.marker = marker;
    out.bar = quoted.then_some(0);
    out
}
pub fn inline(text: &str) -> Vec<Span> {
    let mut style = Span::default();
    let mut out = Vec::new();
    let mut bold = 0;
    let mut italic = 0;
    let mut strike = 0;
    let mut tags = html::Inline::default();
    let mut comment = false;
    // Alternative text of the image being read, and the image.
    let mut image: Option<(Image, String)> = None;
    let mut markup = false;
    let push = |out: &mut Vec<Span>, tags: &html::Inline, mut span: Span| {
        if tags.hidden() {
            return;
        }
        tags.style(&mut span);
        out.push(span);
    };
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
            Event::Start(Tag::Image { dest_url, .. }) => {
                image = Some((
                    Image {
                        src: dest_url.to_string(),
                        ..Image::default()
                    },
                    String::new(),
                ));
            }
            Event::End(TagEnd::Image) => {
                if let Some((picture, alt)) = image.take() {
                    push(
                        &mut out,
                        &tags,
                        Span {
                            text: alt,
                            image: Some(picture),
                            ..style.clone()
                        },
                    );
                }
            }
            Event::Text(t) | Event::Code(t) if image.is_some() => {
                if let Some((_, alt)) = &mut image {
                    alt.push_str(&t);
                }
            }
            Event::Text(t) => push(
                &mut out,
                &tags,
                Span {
                    text: t.to_string(),
                    ..style.clone()
                },
            ),
            Event::Code(t) => push(
                &mut out,
                &tags,
                Span {
                    text: t.to_string(),
                    code: true,
                    ..style.clone()
                },
            ),
            Event::Html(t) | Event::InlineHtml(t) => {
                markup = true;
                for token in html::tokens(&t, &mut comment) {
                    match token {
                        html::Token::Skip => {}
                        html::Token::Text(piece) => push(
                            &mut out,
                            &tags,
                            Span {
                                text: html::decode(piece),
                                ..style.clone()
                            },
                        ),
                        html::Token::Tag(tag) => {
                            if let Some(span) = tags.apply(&tag) {
                                push(
                                    &mut out,
                                    &tags,
                                    Span {
                                        link: span.link.clone().or_else(|| style.link.clone()),
                                        bold: span.bold || style.bold,
                                        italic: span.italic || style.italic,
                                        ..span
                                    },
                                );
                            }
                        }
                    }
                }
            }
            Event::SoftBreak => push(
                &mut out,
                &tags,
                Span {
                    text: " ".into(),
                    ..style.clone()
                },
            ),
            Event::HardBreak => push(
                &mut out,
                &tags,
                Span {
                    text: " ".into(),
                    line_break: true,
                    ..style.clone()
                },
            ),
            Event::TaskListMarker(done) => push(
                &mut out,
                &tags,
                Span {
                    text: if done { "☑ " } else { "☐ " }.into(),
                    check: Some(done),
                    ..Span::default()
                },
            ),
            _ => {}
        }
    }
    // Nothing parsed (a link reference definition, say): show the text as written,
    // unless it was only hidden HTML.
    if out.is_empty() && !text.is_empty() && !markup {
        return plain(text);
    }
    out
}
pub fn safe_web_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !url.chars().any(char::is_control)
}
