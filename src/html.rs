//! A safe HTML subset inside Markdown. Tags only shape the text: headings,
//! paragraphs, emphasis, lists, quotes, alignment, line breaks and image
//! placeholders. Nothing is executed or fetched; script, style and embedded
//! content stay hidden. Like the rest of the preview it works line by line, with
//! a small bounded state carried across the lines of one page.
use crate::markdown::{Image, Span};
use crate::table::Align;

/// Containers and inline styles are capped so hostile input stays bounded.
const MAX_DEPTH: usize = 32;
/// Longest tag the scanner looks at.
const MAX_TAG: usize = 2048;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    /// Lower-case element name.
    pub name: String,
    pub close: bool,
    pub attrs: Vec<(String, String)>,
}
impl Tag {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Text(&'a str),
    Tag(Tag),
    /// A comment, declaration or processing instruction: shown as nothing.
    Skip,
}

/// Elements whose content is never shown.
const HIDDEN: &[&str] = &[
    "script", "style", "template", "iframe", "object", "embed", "applet", "noscript", "svg",
    "math", "canvas", "video", "audio", "select", "textarea", "head", "title", "frameset",
    "noembed", "button", "form",
];
/// Elements dropped without a trace (their content, if any, shows).
const SILENT: &[&str] = &[
    "html",
    "body",
    "meta",
    "link",
    "base",
    "source",
    "track",
    "wbr",
    "picture",
    "col",
    "colgroup",
    "thead",
    "tbody",
    "tfoot",
    "caption",
    "map",
    "area",
    "label",
    "fieldset",
    "legend",
    "nav",
    "header",
    "footer",
    "main",
    "section",
    "article",
    "aside",
    "figure",
    "figcaption",
    "address",
    "hgroup",
    "dl",
    "dt",
    "dd",
    "menu",
    "optgroup",
    "option",
    "param",
    "frame",
    "basefont",
    "bgsound",
    "dialog",
    "search",
    "output",
    "meter",
    "progress",
    "data",
    "time",
    "bdi",
    "bdo",
    "ruby",
    "rt",
    "rp",
    "abbr",
    "acronym",
    "cite",
    "dfn",
    "q",
    "samp",
    "var",
    "small",
    "big",
    "font",
    "span",
    "tt",
    "center",
    "div",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "pre",
    "blockquote",
    "ul",
    "ol",
    "li",
    "details",
    "summary",
    "table",
    "tr",
    "td",
    "th",
    "hr",
    "br",
    "img",
    "a",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "ins",
    "s",
    "del",
    "strike",
    "code",
    "kbd",
    "mark",
    "sup",
    "sub",
    "input",
];
/// CommonMark's block-level tag names (HTML block type 6).
const BLOCK: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];
/// Raw-text elements (type 1): they run to their closing tag, blank lines included.
const RAW: &[&str] = &["pre", "script", "style", "textarea"];

fn known(name: &str) -> bool {
    HIDDEN.contains(&name) || SILENT.contains(&name) || BLOCK.contains(&name)
}

/// Splits `text` into text, tags and skipped markup. `<` that does not start a
/// complete tag on this line stays text. `comment` carries an open `<!--`.
pub fn tokens<'a>(text: &'a str, comment: &mut bool) -> Vec<Token<'a>> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if *comment {
            match rest.find("-->") {
                Some(end) => {
                    rest = &rest[end + 3..];
                    *comment = false;
                    out.push(Token::Skip);
                }
                None => {
                    out.push(Token::Skip);
                    break;
                }
            }
            continue;
        }
        let Some(open) = rest.find('<') else {
            out.push(Token::Text(rest));
            break;
        };
        if open > 0 {
            out.push(Token::Text(&rest[..open]));
            rest = &rest[open..];
        }
        if let Some(after) = rest.strip_prefix("<!--") {
            match after.find("-->") {
                Some(end) => rest = &after[end + 3..],
                None => {
                    *comment = true;
                    rest = "";
                }
            }
            out.push(Token::Skip);
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            if let Some(end) = rest.find('>') {
                out.push(Token::Skip);
                rest = &rest[end + 1..];
                continue;
            }
        } else if let Some((tag, len)) = parse_tag(rest)
            && known(&tag.name)
        {
            out.push(Token::Tag(tag));
            rest = &rest[len..];
            continue;
        }
        // Not markup: keep the `<` as text.
        out.push(Token::Text(&rest[..1]));
        rest = &rest[1..];
    }
    out
}

/// A complete tag at the start of `s` and its byte length.
pub fn parse_tag(s: &str) -> Option<(Tag, usize)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'<') {
        return None;
    }
    let mut i = 1;
    let close = b.get(1) == Some(&b'/');
    if close {
        i = 2;
    }
    let start = i;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-') {
        i += 1;
    }
    if i == start || !b[start].is_ascii_alphabetic() {
        return None;
    }
    let name = s[start..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || i > MAX_TAG {
            return None;
        }
        match b[i] {
            b'>' => {
                return Some((Tag { name, close, attrs }, i + 1));
            }
            b'/' if b.get(i + 1) == Some(&b'>') => {
                return Some((Tag { name, close, attrs }, i + 2));
            }
            _ if close => return None,
            _ => {}
        }
        let key = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        if i == key {
            return None;
        }
        let key = s[key..i].to_ascii_lowercase();
        let mut value = String::new();
        let mut j = i;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if b.get(j) == Some(&b'=') {
            j += 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            match b.get(j) {
                Some(&q) if q == b'"' || q == b'\'' => {
                    let end = s[j + 1..].find(q as char)? + j + 1;
                    value = decode(&s[j + 1..end]);
                    i = end + 1;
                }
                Some(_) => {
                    let end = j + b[j..]
                        .iter()
                        .take_while(|&&c| !c.is_ascii_whitespace() && c != b'>')
                        .count();
                    value = decode(&s[j..end]);
                    i = end;
                }
                None => return None,
            }
        }
        if attrs.len() < 32 {
            attrs.push((key, value));
        }
    }
}

const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("ensp", "\u{2002}"),
    ("emsp", "\u{2003}"),
    ("thinsp", "\u{2009}"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("hellip", "…"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("middot", "·"),
    ("bull", "•"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("times", "×"),
    ("divide", "÷"),
    ("plusmn", "±"),
    ("deg", "°"),
    ("para", "¶"),
    ("sect", "§"),
    ("euro", "€"),
    ("pound", "£"),
    ("yen", "¥"),
    ("cent", "¢"),
    ("larr", "←"),
    ("rarr", "→"),
    ("uarr", "↑"),
    ("darr", "↓"),
    ("harr", "↔"),
    ("rArr", "⇒"),
    ("lArr", "⇐"),
    ("check", "✓"),
    ("star", "☆"),
    ("hearts", "♥"),
    ("zwj", "\u{200d}"),
    ("zwnj", "\u{200c}"),
];

/// Text with HTML character references replaced; unknown ones stay as written.
pub fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.as_bytes().iter().take(12).position(|&c| c == b';');
        let decoded = end.and_then(|end| {
            let name = &rest[1..end];
            let value = if let Some(number) = name.strip_prefix('#') {
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse().ok(),
                }?;
                char::from_u32(code)
                    .filter(|c| !c.is_control() || matches!(c, '\t' | '\n'))
                    .map(String::from)
            } else {
                ENTITIES
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| (*v).to_owned())
            }?;
            Some((value, end + 1))
        });
        match decoded {
            Some((value, len)) => {
                out.push_str(&value);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Inline styles opened by tags and still open.
#[derive(Clone, Debug, Default)]
pub struct Inline {
    bold: u8,
    italic: u8,
    underline: u8,
    strike: u8,
    code: u8,
    kbd: u8,
    mark: u8,
    sup: u8,
    sub: u8,
    hidden: u8,
    links: Vec<String>,
}
impl Inline {
    fn depth(counter: &mut u8, close: bool) {
        if close {
            *counter = counter.saturating_sub(1);
        } else if (*counter as usize) < MAX_DEPTH {
            *counter += 1;
        }
    }
    pub fn hidden(&self) -> bool {
        self.hidden > 0
    }
    /// Adds the open styles to `span`.
    pub fn style(&self, span: &mut Span) {
        span.bold |= self.bold > 0;
        span.italic |= self.italic > 0;
        span.underline |= self.underline > 0;
        span.strike |= self.strike > 0;
        span.code |= self.code > 0;
        span.kbd |= self.kbd > 0;
        span.mark |= self.mark > 0;
        if self.sup > 0 {
            span.script = 1;
        } else if self.sub > 0 {
            span.script = -1;
        }
        if span.link.is_none()
            && let Some(link) = self.links.last().filter(|l| !l.is_empty())
        {
            span.link = Some(link.clone());
        }
    }
    /// Applies an inline tag; returns a span it stands for (image, line break,
    /// checkbox). Block tags are left to the caller.
    pub fn apply(&mut self, tag: &Tag) -> Option<Span> {
        let close = tag.close;
        match tag.name.as_str() {
            "b" | "strong" => Self::depth(&mut self.bold, close),
            "i" | "em" | "cite" | "dfn" | "var" => Self::depth(&mut self.italic, close),
            "u" | "ins" => Self::depth(&mut self.underline, close),
            "s" | "del" | "strike" => Self::depth(&mut self.strike, close),
            "code" | "tt" | "samp" => Self::depth(&mut self.code, close),
            "kbd" => Self::depth(&mut self.kbd, close),
            "mark" => Self::depth(&mut self.mark, close),
            "sup" => Self::depth(&mut self.sup, close),
            "sub" => Self::depth(&mut self.sub, close),
            "a" if close => {
                self.links.pop();
            }
            "a" => {
                if self.links.len() < MAX_DEPTH {
                    self.links
                        .push(tag.attr("href").unwrap_or("").trim().to_owned());
                }
            }
            "img" if !close => {
                let image = Image {
                    src: tag.attr("src").unwrap_or("").trim().to_owned(),
                    width: tag.attr("width").and_then(size),
                    height: tag.attr("height").and_then(size),
                };
                let mut span = Span {
                    text: tag.attr("alt").unwrap_or("").to_owned(),
                    image: Some(image),
                    ..Span::default()
                };
                self.style(&mut span);
                return Some(span);
            }
            "br" if !close => {
                return Some(Span {
                    text: " ".into(),
                    line_break: true,
                    ..Span::default()
                });
            }
            "input"
                if !close
                    && tag
                        .attr("type")
                        .is_some_and(|t| t.eq_ignore_ascii_case("checkbox")) =>
            {
                let done = tag.attr("checked").is_some();
                return Some(Span {
                    text: if done { "☑ " } else { "☐ " }.into(),
                    check: Some(done),
                    ..Span::default()
                });
            }
            name if HIDDEN.contains(&name) => Self::depth(&mut self.hidden, close),
            _ => {}
        }
        None
    }
}

/// A pixel size attribute: `120` or `120px`; percentages are ignored.
fn size(value: &str) -> Option<u32> {
    let value = value.trim().trim_end_matches("px").trim();
    value
        .parse::<u32>()
        .ok()
        .filter(|v| (1..=16384).contains(v))
}

/// How a line drawn by the HTML path is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    /// Only tags, comments or hidden content: a thin spacer.
    #[default]
    Empty,
    Paragraph,
    Heading(u8),
    /// Preformatted text.
    Code,
    Quote,
    Rule,
}

/// Block context of the open HTML containers, also applied to Markdown lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub align: Align,
    /// Extra indent in logical pixels (lists, quotes, details).
    pub indent: i32,
    pub quote: bool,
}

/// A line drawn by the HTML path.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub shape: Shape,
    pub spans: Vec<Span>,
    pub context: Context,
    /// The first span is a list marker; wrapped lines align after it.
    pub marker: bool,
}

#[derive(Clone, Debug)]
struct Open {
    name: String,
    align: Option<Align>,
    /// Ordered lists: the next item number.
    next: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Block {
    /// Not in an HTML block: lines are Markdown.
    #[default]
    None,
    /// Ends at a blank line (CommonMark types 6 and 7).
    UntilBlank,
    /// Runs to the closing tag of this raw element (type 1).
    Raw(&'static str),
    /// Runs to the end of a comment, declaration or CDATA (types 2–5).
    Markup(&'static str),
}

/// HTML state across the lines of one page.
#[derive(Clone, Debug, Default)]
pub struct Html {
    stack: Vec<Open>,
    inline: Inline,
    block: Block,
    comment: bool,
    /// A list marker on a line of its own, waiting for the item's text.
    marker: Option<String>,
}

/// Alignment a container sets: `<center>`, `align="…"` or a `text-align` style.
fn alignment(tag: &Tag) -> Option<Align> {
    if tag.name == "center" {
        return Some(Align::Center);
    }
    let style = tag.attr("style").and_then(|style| {
        style.split(';').find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("text-align")
                .then(|| value.trim())
        })
    });
    match tag
        .attr("align")
        .or(style)?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "center" | "middle" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "left" | "justify" | "start" => Some(Align::Left),
        _ => None,
    }
}

/// The HTML block a line starts, per CommonMark's start conditions.
fn starts(text: &str) -> Block {
    let t = text.trim_start();
    if text.len() - t.len() > 3 || !t.starts_with('<') {
        return Block::None;
    }
    let lower: String = t.chars().take(16).collect::<String>().to_ascii_lowercase();
    if lower.starts_with("<!--") {
        return Block::Markup("-->");
    }
    if lower.starts_with("<?") {
        return Block::Markup("?>");
    }
    if lower.starts_with("<![cdata[") {
        return Block::Markup("]]>");
    }
    if lower.starts_with("<!") && lower.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic) {
        return Block::Markup(">");
    }
    let name_start = if lower.starts_with("</") { 2 } else { 1 };
    let name: String = lower[name_start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if name_start == 1
        && let Some(raw) = RAW.iter().find(|r| **r == name)
    {
        let after = t.as_bytes().get(1 + raw.len()).copied();
        if after.is_none_or(|c| c.is_ascii_whitespace() || c == b'>') {
            return Block::Raw(raw);
        }
    }
    if BLOCK.contains(&name.as_str()) {
        let after = t.as_bytes().get(name_start + name.len()).copied();
        if after.is_none_or(|c| c.is_ascii_whitespace() || c == b'>' || c == b'/') {
            return Block::UntilBlank;
        }
    }
    // Type 7: a complete known tag alone on its line.
    if let Some((tag, len)) = parse_tag(t)
        && known(&tag.name)
        && t[len..].trim().is_empty()
    {
        return Block::UntilBlank;
    }
    Block::None
}

/// Whether `text` starts an HTML block.
pub fn claims_start(text: &str) -> bool {
    starts(text) != Block::None
}

impl Html {
    /// Whether the HTML path draws `text`. Call once per line, in order.
    pub fn claims(&mut self, text: &str) -> bool {
        if self.block == Block::None {
            self.block = starts(text);
        }
        if self.block == Block::UntilBlank && text.trim().is_empty() {
            self.end_block();
            return false;
        }
        self.block != Block::None
    }

    /// Context of the open containers for a Markdown line.
    pub fn context(&self) -> Context {
        let mut context = Context::default();
        for open in &self.stack {
            match open.name.as_str() {
                "blockquote" => {
                    context.quote = true;
                    context.indent += 16;
                }
                "ul" | "ol" => context.indent += 22,
                "details" => context.indent += 12,
                _ => {}
            }
            if let Some(align) = open.align {
                context.align = align;
            }
        }
        context
    }

    fn open(&mut self, tag: &Tag) {
        // An item or paragraph implicitly closes the previous one.
        if matches!(tag.name.as_str(), "li" | "p")
            && let Some(at) = self.stack.iter().rposition(|o| o.name == tag.name)
            && self.stack[at + 1..]
                .iter()
                .all(|o| !matches!(o.name.as_str(), "ul" | "ol"))
        {
            self.stack.truncate(at);
        }
        if self.stack.len() < MAX_DEPTH {
            self.stack.push(Open {
                name: tag.name.clone(),
                align: alignment(tag),
                next: tag
                    .attr("start")
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(1),
            });
        }
    }

    fn close(&mut self, name: &str) {
        if let Some(at) = self.stack.iter().rposition(|o| o.name == name) {
            self.stack.truncate(at);
        }
    }

    fn level(&self) -> Option<u8> {
        self.stack.iter().rev().find_map(|o| {
            let b = o.name.as_bytes();
            (b.len() == 2 && b[0] == b'h' && (b'1'..=b'6').contains(&b[1])).then(|| b[1] - b'0')
        })
    }

    fn inside(&self, name: &str) -> bool {
        self.stack.iter().any(|o| o.name == name)
    }

    /// Leaves the HTML block: the next line is Markdown again.
    fn end_block(&mut self) {
        self.block = Block::None;
        self.inline = Inline::default();
        self.marker = None;
    }

    /// Draws one line of an HTML block.
    pub fn line(&mut self, text: &str) -> Line {
        let mut line = Line::default();
        let mut raw = match self.block {
            Block::Raw(name) => Some(name),
            _ => None,
        };
        let pre = raw == Some("pre") || self.inside("pre");
        let mut shape: Option<Shape> = None;
        let mut context: Option<Context> = None;
        let mut summary = false;
        let mut ended = false;
        if let Some(marker) = self.marker.take() {
            line.spans.push(Span {
                text: marker,
                ..Span::default()
            });
            line.marker = true;
        }
        for token in tokens(text, &mut self.comment) {
            match token {
                Token::Skip => {}
                // Script, style and textarea: only their closing tag counts.
                Token::Tag(tag) if raw.is_some_and(|r| r != "pre") => {
                    if tag.close && raw == Some(tag.name.as_str()) {
                        raw = None;
                        ended = true;
                    }
                }
                // Inside hidden content only the hidden elements nest.
                Token::Tag(tag) if self.inline.hidden() && !HIDDEN.contains(&tag.name.as_str()) => {
                }
                Token::Tag(tag) => {
                    let name = tag.name.as_str();
                    if tag.close {
                        if raw == Some(name) {
                            ended = true;
                        }
                        if BLOCK.contains(&name) || name == "pre" {
                            self.close(name);
                        } else {
                            self.inline.apply(&tag);
                        }
                        continue;
                    }
                    match name {
                        "hr" => {
                            shape = Some(Shape::Rule);
                        }
                        "li" => {
                            self.open(&tag);
                            let ordered = self
                                .stack
                                .iter()
                                .rev()
                                .find(|o| matches!(o.name.as_str(), "ul" | "ol"));
                            let marker = match ordered {
                                Some(list) if list.name == "ol" => {
                                    let number = list.next;
                                    format!("{number}. ")
                                }
                                _ => "• ".to_owned(),
                            };
                            if let Some(list) = self
                                .stack
                                .iter_mut()
                                .rev()
                                .find(|o| matches!(o.name.as_str(), "ul" | "ol"))
                            {
                                list.next = list.next.saturating_add(1);
                            }
                            line.spans.push(Span {
                                text: marker,
                                ..Span::default()
                            });
                            line.marker = line.spans.len() == 1;
                        }
                        "summary" => {
                            self.open(&tag);
                            summary = true;
                        }
                        "td" | "th" => {
                            if line.spans.iter().any(|s| !s.text.trim().is_empty()) {
                                line.spans.push(Span {
                                    text: "  ·  ".into(),
                                    ..Span::default()
                                });
                            }
                            self.open(&tag);
                        }
                        "tr" => {
                            if line.spans.iter().any(|s| !s.text.trim().is_empty()) {
                                line.spans.push(Span {
                                    text: " ".into(),
                                    line_break: true,
                                    ..Span::default()
                                });
                            }
                            self.open(&tag);
                        }
                        _ if BLOCK.contains(&name) || name == "pre" => {
                            self.open(&tag);
                        }
                        _ => {
                            if let Some(mut span) = self.inline.apply(&tag)
                                && !self.inline.hidden()
                                && raw.is_none_or(|r| r == "pre")
                            {
                                if span.image.is_some() || span.check.is_some() {
                                    context.get_or_insert(self.context());
                                }
                                self.inline.style(&mut span);
                                line.spans.push(span);
                            }
                        }
                    }
                }
                Token::Text(piece) => {
                    if self.inline.hidden() || raw.is_some_and(|r| r != "pre") {
                        continue;
                    }
                    let text = if pre {
                        decode(piece)
                    } else {
                        let decoded = decode(piece);
                        let mut collapsed = String::with_capacity(decoded.len());
                        let mut space = line.spans.last().is_some_and(|s| s.text.ends_with(' '))
                            || line.spans.is_empty();
                        for c in decoded.chars() {
                            if c.is_whitespace() && c != '\u{a0}' {
                                if !space {
                                    collapsed.push(' ');
                                    space = true;
                                }
                            } else {
                                collapsed.push(c);
                                space = false;
                            }
                        }
                        collapsed
                    };
                    if text.is_empty() {
                        continue;
                    }
                    context.get_or_insert(self.context());
                    if shape.is_none() {
                        shape = Some(if pre {
                            Shape::Code
                        } else if let Some(level) = self.level() {
                            Shape::Heading(level)
                        } else if self.inside("blockquote") {
                            Shape::Quote
                        } else {
                            Shape::Paragraph
                        });
                    }
                    let mut span = Span {
                        text,
                        bold: summary || self.inside("th"),
                        ..Span::default()
                    };
                    self.inline.style(&mut span);
                    if summary && !line.spans.iter().any(|s| s.disclosure) {
                        line.spans.push(Span {
                            text: "▾ ".into(),
                            bold: true,
                            disclosure: true,
                            ..Span::default()
                        });
                    }
                    line.spans.push(span);
                }
            }
        }
        // Raw and markup blocks (comments, declarations) end on the line that
        // closes them.
        if let Block::Markup(end) = self.block
            && text.contains(end)
            && !self.comment
        {
            ended = true;
        }
        if ended {
            self.end_block();
        }
        if let Some(last) = line.spans.last_mut()
            && !pre
            && last.image.is_none()
        {
            let trimmed = last.text.trim_end().len();
            last.text
                .truncate(trimmed.max(usize::from(last.line_break)));
        }
        line.spans
            .retain(|s| !s.text.is_empty() || s.image.is_some());
        let visible = line
            .spans
            .iter()
            .any(|s| !s.text.trim().is_empty() || s.image.is_some() || s.check.is_some());
        // A marker alone waits for the item's text on a later line.
        if line.marker && line.spans.len() == 1 && self.marker.is_none() {
            self.marker = line.spans.pop().map(|s| format!("{} ", s.text.trim_end()));
            line.marker = false;
        }
        let visible = visible && !line.spans.is_empty();
        line.shape = match shape {
            Some(Shape::Rule) if !visible => Shape::Rule,
            Some(Shape::Rule) => Shape::Paragraph,
            Some(shape) if visible => shape,
            _ if visible => Shape::Paragraph,
            _ if pre && !text.is_empty() && raw.is_some() => Shape::Code,
            _ => Shape::Empty,
        };
        line.context = context.unwrap_or_else(|| self.context());
        if pre {
            line.context.indent = 0;
        }
        if !visible {
            line.spans.clear();
        }
        line
    }
}

/// Plain text of a heading written as one line of HTML (`<h1 align="center">Title</h1>`),
/// for the outline.
pub fn heading(text: &str) -> Option<(u8, String)> {
    let t = text.trim();
    let (tag, len) = parse_tag(t)?;
    let level = match tag.name.as_bytes() {
        [b'h', d @ b'1'..=b'6'] => d - b'0',
        _ => return None,
    };
    let mut comment = false;
    let mut inline = Inline::default();
    let mut out = String::new();
    for token in tokens(&t[len..], &mut comment) {
        match token {
            Token::Text(piece) if !inline.hidden() => out.push_str(&decode(piece)),
            Token::Tag(tag) => {
                if tag.name == format!("h{level}") && tag.close {
                    break;
                }
                if let Some(span) = inline.apply(&tag)
                    && span.image.is_some()
                {
                    out.push_str(&span.text);
                }
            }
            _ => {}
        }
    }
    let out: String = out.split_whitespace().collect::<Vec<_>>().join(" ");
    (!out.is_empty()).then_some((level, out))
}
