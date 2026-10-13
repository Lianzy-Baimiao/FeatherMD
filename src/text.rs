//! Source text shown on screen and character-level selection over loaded lines.
//! Pure functions; hit testing and painting live in the GUI.
use crate::document::SourceLine;

fn shown(c: char) -> &'static str {
    if c == '\t' { "    " } else { "\u{fffd}" }
}

/// On-screen text: a tab becomes four spaces and control characters become U+FFFD,
/// so native text APIs never interpret them.
pub fn display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\t' || c.is_control() {
            out.push_str(shown(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// UTF-16 units `c` occupies in `display`.
pub fn units_of(c: char) -> usize {
    if c == '\t' {
        4
    } else if c.is_control() {
        1
    } else {
        c.len_utf16()
    }
}

/// Displayed units before byte `index` of `text` (a partial character counts as before).
pub fn units_before(text: &str, index: usize) -> usize {
    text.char_indices()
        .take_while(|&(i, c)| i + c.len_utf8() <= index)
        .map(|(_, c)| units_of(c))
        .sum()
}

/// Byte index of the character boundary at or before displayed unit `unit`.
pub fn index_at_unit(text: &str, unit: usize) -> usize {
    let mut used = 0;
    for (i, c) in text.char_indices() {
        used += units_of(c);
        if used > unit {
            return i;
        }
    }
    text.len()
}

/// A position between characters of a loaded line: the line's first file byte and a
/// byte index into its decoded text. Lines are ordered by start, so carets compare in
/// reading order and stay meaningful after relayout or a page refill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Caret {
    pub line: u64,
    pub index: usize,
}

fn floor(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Exact source text between two carets over the loaded lines, with CRLF between
/// physical lines and none inside a long line split into segments. The flag is true
/// when part of the range lies outside the loaded lines and was left out.
pub fn selected(lines: &[SourceLine], a: Caret, b: Caret) -> (String, bool) {
    let (from, to) = (a.min(b), a.max(b));
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return (String::new(), from != to);
    };
    let clipped = from.line < first.start || to.line > last.start;
    let mut out = String::new();
    let begin = lines.partition_point(|l| l.start < from.line);
    for (offset, line) in lines[begin..].iter().enumerate() {
        if line.start > to.line {
            break;
        }
        if offset > 0 && !lines[begin + offset - 1].continued {
            out.push_str("\r\n");
        }
        let start = if line.start == from.line {
            floor(&line.text, from.index)
        } else {
            0
        };
        let end = if line.start == to.line {
            floor(&line.text, to.index)
        } else {
            line.text.len()
        };
        out.push_str(&line.text[start.min(end)..end]);
    }
    (out, clipped)
}

#[derive(PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Other,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Other
    }
}

/// Byte range of the word, space run or symbol run containing byte `index`.
pub fn word_at(text: &str, index: usize) -> (usize, usize) {
    let index = floor(text, index);
    let Some(c) = text[index..].chars().next() else {
        return (index, index);
    };
    let kind = class(c);
    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|&(_, c)| class(c) == kind)
        .last()
        .map_or(index, |(i, _)| i);
    let end = text[index..]
        .char_indices()
        .find(|&(_, c)| class(c) != kind)
        .map_or(text.len(), |(i, _)| index + i);
    (start, end)
}
