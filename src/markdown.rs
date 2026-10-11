//! Window-local Markdown. Never execute HTML or fetch external resources.
use crate::document::SourceLine;
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
pub fn blocks(lines: &[SourceLine], raw: bool) -> Vec<Block> {
    let mut fence = Fence::default();
    let mut continued = false;
    lines
        .iter()
        .map(|line| {
            let text = line.text.as_str();
            let mut kind = Kind::Paragraph;
            let mut indent = 0;
            let spans = if raw {
                kind = Kind::Code;
                plain(text)
            } else if !continued && fence.feed(text) {
                kind = Kind::Fence;
                plain(if fence.is_open() {
                    text.trim().trim_start_matches(['`', '~']).trim()
                } else {
                    ""
                })
            } else if fence.is_open() || continued {
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
                } else if t.starts_with('|') {
                    kind = Kind::Table;
                } else if text.starts_with("    ") || text.starts_with('\t') {
                    kind = Kind::Code;
                }
                if matches!(kind, Kind::Code | Kind::Table) {
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
            continued = line.continued;
            Block {
                start: line.start,
                end: line.end,
                kind,
                spans,
                indent,
            }
        })
        .collect()
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
                    text: "[图片: ".into(),
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
