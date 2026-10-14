//! Bounded, cancellable ATX heading navigation. No whole-file allocation or link execution.
use crate::{document::Document, markdown::Fence};
use pulldown_cmark::{Event, Parser};
use std::{
    io,
    sync::atomic::{AtomicU64, Ordering},
};

pub const CHUNK_BYTES: usize = 256 * 1024;
pub const LINE_PREFIX_BYTES: usize = 4096;
pub const MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_HEADINGS: usize = 4096;
pub const MAX_HEADING_CHARS: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub offset: u64,
    pub level: u8,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outline {
    pub headings: Vec<Heading>,
    pub scanned: u64,
    pub size: u64,
    pub complete: bool,
    pub limited: bool,
}

/// Returns None when cancelled. Size is sampled at scan start; external writes are
/// safe, but this is not an immutable snapshot. Setext headings are not indexed.
pub fn scan(document: &Document, epoch: &AtomicU64, ticket: u64) -> io::Result<Option<Outline>> {
    if epoch.load(Ordering::Relaxed) != ticket {
        return Ok(None);
    }
    scan_reader(document.current_size()?, epoch, ticket, |out, offset| {
        document.read_bounded(out, offset)
    })
}

fn scan_reader(
    size: u64,
    epoch: &AtomicU64,
    ticket: u64,
    mut read: impl FnMut(&mut [u8], u64) -> io::Result<usize>,
) -> io::Result<Option<Outline>> {
    let mut result = Outline {
        size,
        ..Outline::default()
    };
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut line = Line::default();
    let mut line_start = 0;
    let mut fence = Fence::default();
    let mut fence_run = 0;
    let limit = size.min(MAX_SCAN_BYTES);
    while result.scanned < limit {
        if epoch.load(Ordering::Relaxed) != ticket {
            return Ok(None);
        }
        let want = (limit - result.scanned).min(CHUNK_BYTES as u64) as usize;
        let n = read(&mut buffer[..want], result.scanned)?;
        if epoch.load(Ordering::Relaxed) != ticket {
            return Ok(None);
        }
        if n == 0 {
            result.size = result.scanned;
            break;
        }
        let mut pos = 0;
        if result.scanned == 0 && buffer[..n].starts_with(b"\xef\xbb\xbf") {
            pos = 3;
            line_start = 3;
        }
        while pos < n {
            let newline = memchr::memchr(b'\n', &buffer[pos..n]).map(|i| pos + i);
            let end = newline.unwrap_or(n);
            line.feed(&buffer[pos..end]);
            pos = end + usize::from(newline.is_some());
            if newline.is_some() {
                line.finish(line_start, &mut fence, &mut fence_run, &mut result);
                line_start = result.scanned + pos as u64;
                if result.headings.len() == MAX_HEADINGS {
                    result.scanned += pos as u64;
                    result.limited = true;
                    return Ok((epoch.load(Ordering::Relaxed) == ticket).then_some(result));
                }
            }
        }
        result.scanned += n as u64;
        if n < want {
            // A positional read reached EOF after the file was shortened.
            result.size = result.scanned;
            break;
        }
    }
    if result.scanned < result.size {
        line.truncated = true;
    }
    line.finish(line_start, &mut fence, &mut fence_run, &mut result);
    result.limited = result.scanned < result.size || result.headings.len() == MAX_HEADINGS;
    result.complete = !result.limited;
    Ok((epoch.load(Ordering::Relaxed) == ticket).then_some(result))
}

#[derive(Default)]
struct Line {
    prefix: Vec<u8>,
    truncated: bool,
    indent: usize,
    marker: u8,
    run: usize,
    tail: bool,
    non_fence: bool,
    tail_backtick: bool,
    tail_content: bool,
}

impl Line {
    fn feed(&mut self, bytes: &[u8]) {
        let keep = bytes.len().min(LINE_PREFIX_BYTES - self.prefix.len());
        self.prefix.extend_from_slice(&bytes[..keep]);
        self.truncated |= keep < bytes.len();
        // Summarize the rest of a possible fence without retaining a long line.
        // A closing fence allows only spaces/tabs (and the final CR in CRLF).
        for &b in bytes {
            if self.non_fence {
                break;
            }
            if self.marker == 0 {
                match b {
                    b' ' if self.indent < 3 => self.indent += 1,
                    b'`' | b'~' => {
                        self.marker = b;
                        self.run = 1;
                    }
                    _ => self.non_fence = true,
                }
            } else if !self.tail && b == self.marker {
                self.run += 1;
            } else {
                self.tail = true;
                self.tail_backtick |= b == b'`';
                self.tail_content |= !matches!(b, b' ' | b'\t' | b'\r');
            }
        }
    }

    fn finish(
        &mut self,
        offset: u64,
        fence: &mut Fence,
        fence_run: &mut usize,
        result: &mut Outline,
    ) {
        let mut delimiter = false;
        if !self.non_fence && self.run >= 3 && (!fence.is_open() || self.run >= *fence_run) {
            // Normalize oversized marker runs, retaining their exact length
            // separately so a shorter closing run still cannot close them.
            let mut candidate = (self.marker as char)
                .to_string()
                .repeat(self.run.min(LINE_PREFIX_BYTES));
            if fence.is_open() && self.tail_content {
                candidate.push_str(" x");
            } else if !fence.is_open() && self.tail_backtick {
                candidate.push_str(" x`");
            }
            delimiter = fence.feed(&candidate);
            if delimiter && fence.is_open() {
                *fence_run = self.run;
            }
        }
        if !delimiter && !fence.is_open() {
            let mut prefix = self.prefix.as_slice();
            if self.truncated
                && let Err(error) = std::str::from_utf8(prefix)
                && error.error_len().is_none()
            {
                prefix = &prefix[..error.valid_up_to()];
            }
            let text = String::from_utf8_lossy(prefix);
            let text = text.trim_end_matches('\r');
            if let Some((level, content)) = atx(text, self.truncated) {
                result.headings.push(Heading {
                    offset,
                    level,
                    text: plain_heading(content),
                });
            } else if text.trim_start().starts_with("<h")
                && let Some((level, title)) = crate::html::heading(text)
            {
                // `<h1 align="center">Title</h1>` on one line, as many READMEs open.
                result.headings.push(Heading {
                    offset,
                    level,
                    text: title.chars().take(MAX_HEADING_CHARS).collect(),
                });
            }
        }
        self.prefix.clear();
        *self = Self {
            prefix: std::mem::take(&mut self.prefix),
            ..Self::default()
        };
    }
}

fn atx(text: &str, truncated: bool) -> Option<(u8, &str)> {
    let trimmed = text.trim_start_matches(' ');
    if text.len() - trimmed.len() > 3 {
        return None;
    }
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes)
        || trimmed
            .as_bytes()
            .get(hashes)
            .is_some_and(|b| !matches!(b, b' ' | b'\t'))
    {
        return None;
    }
    let mut content = trimmed[hashes..].trim_matches([' ', '\t']);
    if !truncated {
        let closing = content.trim_end_matches('#');
        if closing.is_empty() || closing.ends_with([' ', '\t']) {
            content = closing.trim_end_matches([' ', '\t']);
        }
    }
    Some((hashes as u8, content))
}

fn plain_heading(content: &str) -> String {
    let mut text = String::new();
    let mut remaining = MAX_HEADING_CHARS;
    for event in Parser::new(content) {
        match event {
            Event::Text(value) | Event::Code(value) => {
                for c in value.chars().take(remaining) {
                    text.push(if c.is_control() { ' ' } else { c });
                    remaining -= 1;
                }
            }
            Event::SoftBreak | Event::HardBreak if remaining > 0 => {
                text.push(' ');
                remaining -= 1;
            }
            _ => {}
        }
        if remaining == 0 {
            break;
        }
    }
    text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_checked_after_each_bounded_read() {
        let epoch = AtomicU64::new(7);
        let mut reads = 0;
        let result = scan_reader(MAX_SCAN_BYTES * 2, &epoch, 7, |out, offset| {
            assert!(out.len() <= CHUNK_BYTES);
            assert_eq!(offset, 0);
            reads += 1;
            out.fill(b'x');
            epoch.store(8, Ordering::Relaxed);
            Ok(out.len())
        })
        .unwrap();
        assert!(result.is_none());
        assert_eq!(reads, 1);
    }

    #[test]
    fn truncation_during_scan_reports_observed_eof() {
        let epoch = AtomicU64::new(0);
        let result = scan_reader(MAX_SCAN_BYTES, &epoch, 0, |out, offset| {
            if offset == 0 {
                out.fill(b'x');
                out[..7].copy_from_slice(b"# kept\n");
                Ok(out.len())
            } else {
                Ok(0)
            }
        })
        .unwrap()
        .unwrap();
        assert_eq!(result.scanned, CHUNK_BYTES as u64);
        assert_eq!(result.size, result.scanned);
        assert!(result.complete);
        assert_eq!(result.headings[0].text, "kept");
    }
}
