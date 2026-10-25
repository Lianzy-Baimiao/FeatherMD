//! Bounded, cancellable outlines by format: Markdown ATX headings, plain-text chapters,
//! and a log's time index (or its error lines). No whole-file allocation or link
//! execution; each line contributes at most a 4096-byte prefix.
use crate::{
    document::Document,
    encoding::Encoding,
    markdown::Fence,
    plain::{self, Format},
};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Heading {
    pub offset: u64,
    pub level: u8,
    pub text: String,
    /// Error and warning lines under a log time entry.
    pub errors: u32,
    pub warnings: u32,
}

/// What an outline lists, by document format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutlineKind {
    /// Markdown headings.
    #[default]
    Headings,
    /// Plain-text chapters (or underlined titles and document numbering).
    Chapters,
    /// A log's time index: days and time ranges with their error and warning counts.
    Time,
    /// The error lines of a log without timestamps.
    Problems,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outline {
    pub headings: Vec<Heading>,
    pub scanned: u64,
    pub size: u64,
    pub complete: bool,
    pub limited: bool,
    pub kind: OutlineKind,
    /// Error and warning lines of the whole log.
    pub errors: u64,
    pub warnings: u64,
}

/// Bytes an outline of `format` reads: Markdown headings and text chapters within a
/// bounded prefix, a log's time index over the whole file (bounded output).
pub fn scan_limit(format: Format) -> u64 {
    match format {
        Format::Markdown => MAX_SCAN_BYTES,
        Format::Text => 256 * 1024 * 1024,
        Format::Log => u64::MAX,
    }
}

/// Returns None when cancelled. Size is sampled at scan start; external writes are
/// safe, but this is not an immutable snapshot. `progress` receives the bytes read so
/// far. Markdown setext headings are not indexed.
pub fn scan(
    document: &Document,
    epoch: &AtomicU64,
    ticket: u64,
    progress: &AtomicU64,
) -> io::Result<Option<Outline>> {
    if epoch.load(Ordering::Relaxed) != ticket {
        return Ok(None);
    }
    let size = document.current_size()?;
    scan_reader(
        (document.format, document.encoding),
        size,
        epoch,
        ticket,
        progress,
        |out, offset| document.read_bounded(out, offset),
    )
}

fn scan_reader(
    (format, encoding): (Format, Encoding),
    size: u64,
    epoch: &AtomicU64,
    ticket: u64,
    progress: &AtomicU64,
    mut read: impl FnMut(&mut [u8], u64) -> io::Result<usize>,
) -> io::Result<Option<Outline>> {
    let mut result = Outline {
        size,
        ..Outline::default()
    };
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut line = Line::default();
    let mut line_start = 0;
    let mut sink = Sink::new(format, encoding);
    let limit = size.min(scan_limit(format));
    let mut full = false;
    'read: while result.scanned < limit {
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
                sink.line(&mut line, line_start, &mut result);
                line.reset();
                line_start = result.scanned + pos as u64;
                if sink.full(&result) {
                    result.scanned += pos as u64;
                    full = true;
                    break 'read;
                }
            }
        }
        result.scanned += n as u64;
        progress.store(result.scanned, Ordering::Relaxed);
        if n < want {
            // A positional read reached EOF after the file was shortened.
            result.size = result.scanned;
            break;
        }
    }
    if !full {
        if result.scanned < result.size {
            line.truncated = true;
        }
        sink.line(&mut line, line_start, &mut result);
    }
    let capped = sink.done(&mut result);
    result.limited = full || capped || result.scanned < result.size;
    result.complete = !result.limited;
    Ok((epoch.load(Ordering::Relaxed) == ticket).then_some(result))
}

/// What each format makes of a line, decoded in the document's encoding.
struct Sink {
    encoding: Encoding,
    kind: SinkKind,
}
enum SinkKind {
    Markdown { fence: Fence, run: usize },
    Text(TextScan),
    Log(Box<LogScan>),
}
impl Sink {
    fn new(format: Format, encoding: Encoding) -> Self {
        let kind = match format {
            Format::Markdown => SinkKind::Markdown {
                fence: Fence::default(),
                run: 0,
            },
            Format::Text => SinkKind::Text(TextScan::default()),
            Format::Log => SinkKind::Log(Box::default()),
        };
        Self { encoding, kind }
    }
    fn line(&mut self, line: &mut Line, offset: u64, result: &mut Outline) {
        let encoding = &self.encoding;
        match &mut self.kind {
            SinkKind::Markdown { fence, run } => line.finish(offset, fence, run, result, encoding),
            SinkKind::Text(scan) => scan.line(offset, &line.text(encoding)),
            SinkKind::Log(scan) => scan.line(offset, &line.text(encoding)),
        }
    }
    /// The heading cap is reached: stop reading.
    fn full(&self, result: &Outline) -> bool {
        match &self.kind {
            SinkKind::Markdown { .. } => result.headings.len() == MAX_HEADINGS,
            SinkKind::Text(scan) => scan.found.len() == MAX_HEADINGS,
            SinkKind::Log(_) => false,
        }
    }
    /// Writes the headings; true when some were left out.
    fn done(self, result: &mut Outline) -> bool {
        match self.kind {
            SinkKind::Markdown { .. } => {
                result.kind = OutlineKind::Headings;
                false
            }
            SinkKind::Text(scan) => scan.done(result),
            SinkKind::Log(scan) => scan.done(result),
        }
    }
}
/// Plain text: chapter lines, underlined titles and, when there are no chapters,
/// Chinese document numbering (`一、`, `（一）`).
#[derive(Default)]
struct TextScan {
    /// (offset, level, title, not a chapter: an underlined title or numbering).
    found: Vec<(u64, u8, String, bool)>,
    /// The last line, when it could be an underlined title.
    title: Option<(u64, String)>,
}
impl TextScan {
    fn line(&mut self, offset: u64, text: &str) {
        let title = self.title.take();
        let trimmed = text.trim_matches([' ', '\t', '\u{3000}']);
        let short = |t: &str| t.chars().take(MAX_HEADING_CHARS).collect::<String>();
        if let Some(level) = plain::chapter(text) {
            self.found.push((offset, level, short(trimmed), false));
        } else if let Some(level) = plain::setext(text)
            && let Some((at, title)) = title.as_ref()
        {
            // Underlined titles rank with document numbering: kept without chapters.
            self.found.push((*at, level, short(title), true));
        } else if let Some(level) = plain::numbered(text) {
            self.found.push((offset, level, short(trimmed), true));
        } else if plain::setext_title(text) {
            // Reuse the last title's buffer: most short lines are only candidates.
            let mut buffer = title.map(|(_, t)| t).unwrap_or_default();
            buffer.clear();
            buffer.push_str(trimmed);
            self.title = Some((offset, buffer));
        }
    }
    fn done(self, result: &mut Outline) -> bool {
        result.kind = OutlineKind::Chapters;
        // Underlined titles and numbering count only in a document without chapters.
        let chapters = self.found.iter().any(|f| !f.3);
        let found: Vec<_> = self
            .found
            .into_iter()
            .filter(|f| !chapters || !f.3)
            .collect();
        // The shallowest level found is level 1.
        let top = found.iter().map(|f| f.1).min().unwrap_or(1);
        result.headings = found
            .into_iter()
            .map(|(offset, level, text, _)| Heading {
                offset,
                level: level - top + 1,
                text,
                ..Heading::default()
            })
            .collect();
        false
    }
}

/// Time buckets, finest first; the scan coarsens when a log has too many.
const STEPS: [i64; 9] = [
    60,
    300,
    900,
    1800,
    3600,
    3 * 3600,
    6 * 3600,
    86_400,
    7 * 86_400,
];
const MAX_SEGMENTS: usize = 240;
const MAX_PROBLEMS: usize = 1000;

/// A log: consecutive time segments with their error and warning counts, or, for a
/// log without timestamps, its error lines.
#[derive(Default)]
struct LogScan {
    segments: Vec<Segment>,
    step: usize,
    problems: Vec<(u64, String)>,
    errors: u64,
    warnings: u64,
    stamped: u64,
    /// More segments than the coarsest step allows, or more error lines than listed.
    segments_capped: bool,
    problems_capped: bool,
}
struct Segment {
    offset: u64,
    start: i64,
    stamp: plain::Stamp,
    errors: u32,
    warnings: u32,
}
impl LogScan {
    fn line(&mut self, offset: u64, text: &str) {
        let clean = plain::strip_ansi(text);
        // Indented lines continue an entry (stack traces): no timestamp of their own.
        let parsed = if clean.starts_with([' ', '\t']) {
            None
        } else {
            plain::stamp(&clean)
        };
        if let Some(parsed) = &parsed {
            self.stamped += 1;
            let start = parsed.stamp.seconds();
            let step = STEPS[self.step];
            let opens = self
                .segments
                .last()
                .is_none_or(|last| start.div_euclid(step) > last.start.div_euclid(step));
            if opens {
                self.segments.push(Segment {
                    offset,
                    start,
                    stamp: parsed.stamp,
                    errors: 0,
                    warnings: 0,
                });
                if self.segments.len() > MAX_SEGMENTS {
                    self.coarsen();
                }
            }
        }
        let level = plain::log_level(&clean, parsed.as_ref()).map(|(level, _, _)| level);
        let (error, warning) = (
            level.is_some_and(|l| l >= plain::Level::Error),
            level == Some(plain::Level::Warn),
        );
        self.errors += u64::from(error);
        self.warnings += u64::from(warning);
        if let Some(segment) = self.segments.last_mut() {
            segment.errors += u32::from(error);
            segment.warnings += u32::from(warning);
        }
        // Error lines matter only while no timestamp has been seen.
        if self.stamped == 0 && plain::problem(&clean) {
            if self.problems.len() < MAX_PROBLEMS {
                let line: String = clean.trim().chars().take(MAX_HEADING_CHARS).collect();
                self.problems.push((offset, line));
            } else {
                self.problems_capped = true;
            }
        }
    }
    /// The next coarser step, merging segments that now share a bucket.
    fn coarsen(&mut self) {
        while self.segments.len() > MAX_SEGMENTS {
            if self.step + 1 == STEPS.len() {
                // Keep the first segments; the outline says it is limited.
                self.segments.truncate(MAX_SEGMENTS);
                self.segments_capped = true;
                return;
            }
            self.step += 1;
            let step = STEPS[self.step];
            let mut merged: Vec<Segment> = Vec::with_capacity(self.segments.len());
            for segment in self.segments.drain(..) {
                match merged.last_mut() {
                    Some(last) if last.start.div_euclid(step) == segment.start.div_euclid(step) => {
                        last.errors += segment.errors;
                        last.warnings += segment.warnings;
                    }
                    _ => merged.push(segment),
                }
            }
            self.segments = merged;
        }
    }
    fn done(self, result: &mut Outline) -> bool {
        result.errors = self.errors;
        result.warnings = self.warnings;
        if self.segments.is_empty() {
            result.kind = OutlineKind::Problems;
            result.headings = self
                .problems
                .into_iter()
                .map(|(offset, text)| Heading {
                    offset,
                    level: 1,
                    text,
                    errors: 1,
                    warnings: 0,
                })
                .collect();
            return self.problems_capped;
        }
        result.kind = OutlineKind::Time;
        let step = STEPS[self.step];
        let day = |s: &Segment| s.stamp.day();
        // Days as level-1 entries when a finer index spans more than one day.
        let days = step < 86_400
            && self
                .segments
                .iter()
                .any(|s| day(s).is_some() && day(s) != day(&self.segments[0]));
        let mut current = None;
        for (i, segment) in self.segments.iter().enumerate() {
            if days && day(segment).is_some() && day(segment) != current {
                current = day(segment);
                let (errors, warnings) = self.segments[i..]
                    .iter()
                    .take_while(|s| day(s) == current)
                    .fold((0, 0), |(e, w), s| (e + s.errors, w + s.warnings));
                result.headings.push(Heading {
                    offset: segment.offset,
                    level: 1,
                    text: day_label(&segment.stamp),
                    errors,
                    warnings,
                });
            }
            let text = if step < 86_400 {
                let at = (segment.start.div_euclid(step) * step).rem_euclid(86_400);
                format!("{:02}:{:02}", at / 3600, at % 3600 / 60)
            } else {
                day_label(&segment.stamp)
            };
            result.headings.push(Heading {
                offset: segment.offset,
                level: if days { 2 } else { 1 },
                text,
                errors: segment.errors,
                warnings: segment.warnings,
            });
        }
        self.segments_capped
    }
}
/// `2026-10-24`, or `10-24` when the log has no year.
fn day_label(stamp: &plain::Stamp) -> String {
    let (month, day) = stamp.date.unwrap_or((0, 0));
    match stamp.year {
        Some(year) => format!("{year}-{month:02}-{day:02}"),
        None => format!("{month:02}-{day:02}"),
    }
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
        encoding: &Encoding,
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
            let text = self.text(encoding);
            if let Some((level, content)) = atx(&text, self.truncated) {
                result.headings.push(Heading {
                    offset,
                    level,
                    text: plain_heading(content),
                    ..Heading::default()
                });
            } else if text.trim_start().starts_with("<h")
                && let Some((level, title)) = crate::html::heading(&text)
            {
                // `<h1 align="center">Title</h1>` on one line, as many READMEs open.
                result.headings.push(Heading {
                    offset,
                    level,
                    text: title.chars().take(MAX_HEADING_CHARS).collect(),
                    ..Heading::default()
                });
            }
        }
    }

    /// The line's kept prefix as text, without a character cut by the prefix limit
    /// and without the CR of CRLF.
    fn text(&self, encoding: &Encoding) -> String {
        if let Encoding::Legacy(codec) = encoding {
            let end = codec.floor(&self.prefix, self.prefix.len());
            let mut text = codec.decode(&self.prefix[..end]).0;
            if text.ends_with('\r') {
                text.pop();
            }
            return text;
        }
        let mut prefix = self.prefix.as_slice();
        if self.truncated
            && let Err(error) = std::str::from_utf8(prefix)
            && error.error_len().is_none()
        {
            prefix = &prefix[..error.valid_up_to()];
        }
        let mut text = String::from_utf8_lossy(prefix).into_owned();
        if text.ends_with('\r') {
            text.pop();
        }
        text
    }

    /// Ready for the next line, keeping the prefix buffer's allocation.
    fn reset(&mut self) {
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
        let progress = AtomicU64::new(0);
        let mut reads = 0;
        let size = MAX_SCAN_BYTES * 2;
        let result = scan_reader(
            (Format::Markdown, Encoding::Utf8),
            size,
            &epoch,
            7,
            &progress,
            |out, offset| {
                assert!(out.len() <= CHUNK_BYTES);
                assert_eq!(offset, 0);
                reads += 1;
                out.fill(b'x');
                epoch.store(8, Ordering::Relaxed);
                Ok(out.len())
            },
        )
        .unwrap();
        assert!(result.is_none());
        assert_eq!(reads, 1);
    }

    #[test]
    fn truncation_during_scan_reports_observed_eof() {
        let epoch = AtomicU64::new(0);
        let progress = AtomicU64::new(0);
        let size = MAX_SCAN_BYTES;
        let result = scan_reader(
            (Format::Markdown, Encoding::Utf8),
            size,
            &epoch,
            0,
            &progress,
            |out, offset| {
                if offset == 0 {
                    out.fill(b'x');
                    out[..7].copy_from_slice(b"# kept\n");
                    Ok(out.len())
                } else {
                    Ok(0)
                }
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.scanned, CHUNK_BYTES as u64);
        assert_eq!(result.size, result.scanned);
        assert!(result.complete);
        assert_eq!(result.headings[0].text, "kept");
    }

    /// The outline of `text` read as `format`.
    fn outline(format: Format, text: &str) -> Outline {
        let (epoch, progress) = (AtomicU64::new(0), AtomicU64::new(0));
        let bytes = text.as_bytes();
        scan_reader(
            (format, Encoding::Utf8),
            bytes.len() as u64,
            &epoch,
            0,
            &progress,
            |out, offset| {
                let from = (offset as usize).min(bytes.len());
                let n = out.len().min(bytes.len() - from);
                out[..n].copy_from_slice(&bytes[from..from + n]);
                Ok(n)
            },
        )
        .unwrap()
        .unwrap()
    }
    fn entries(outline: &Outline) -> Vec<(u8, &str)> {
        outline
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect()
    }

    #[test]
    fn text_outlines_list_chapters_and_drop_numbering_inside_chapters() {
        let novel = "书名\n\n第一卷 风起\n\n第一章 少年\n　　他走了。\n一、清单项\n第二章：重逢\n\n第二卷\n第三章\n";
        let result = outline(Format::Text, novel);
        assert_eq!(result.kind, OutlineKind::Chapters);
        assert_eq!(
            entries(&result),
            [
                (1, "第一卷 风起"),
                (2, "第一章 少年"),
                (2, "第二章：重逢"),
                (1, "第二卷"),
                (2, "第三章")
            ]
        );
        let at = result.headings[1].offset as usize;
        assert!(novel[at..].starts_with("第一章"));
        // Without chapters: underlined titles and document numbering, shallowest first.
        let report = "Report\n======\n\nScope\n-----\n一、总则\n（一）适用范围\n";
        let result = outline(Format::Text, report);
        assert_eq!(
            entries(&result),
            [
                (1, "Report"),
                (2, "Scope"),
                (1, "一、总则"),
                (2, "（一）适用范围")
            ]
        );
        assert!(
            outline(Format::Text, "just prose.\nmore prose.\n")
                .headings
                .is_empty()
        );
    }

    #[test]
    fn log_outlines_index_days_and_time_ranges_with_their_problems() {
        let log = "2026-10-23 23:59:58 WARN low disk\n\
                   2026-10-24 08:00:01 INFO start\n\
                   2026-10-24 08:00:30 ERROR failed\n\
                   \tat com.example.Main.run(Main.java:3)\n\
                   2026-10-24 08:10:00 ERROR again\n";
        let result = outline(Format::Log, log);
        assert_eq!(result.kind, OutlineKind::Time);
        assert_eq!((result.errors, result.warnings), (2, 1));
        let shape: Vec<_> = result
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str(), h.errors, h.warnings))
            .collect();
        assert_eq!(
            shape,
            [
                (1, "2026-10-23", 0, 1),
                (2, "23:59", 0, 1),
                (1, "2026-10-24", 2, 0),
                (2, "08:00", 1, 0),
                (2, "08:10", 1, 0),
            ]
        );
        let at = result.headings[3].offset as usize;
        assert!(log[at..].starts_with("2026-10-24 08:00:01"));
    }

    #[test]
    fn busy_logs_coarsen_to_a_bounded_index_and_keep_their_counts() {
        // Three days, a line a minute, an error at the top of each hour.
        let mut log = String::new();
        for day in 23..26 {
            for minute in 0..24 * 60 {
                let level = if minute % 60 == 0 { "ERROR" } else { "INFO" };
                log.push_str(&format!(
                    "2026-10-{day} {:02}:{:02}:00 {level} tick\n",
                    minute / 60,
                    minute % 60
                ));
            }
        }
        let result = outline(Format::Log, &log);
        let days: Vec<_> = result.headings.iter().filter(|h| h.level == 1).collect();
        let ranges: Vec<_> = result.headings.iter().filter(|h| h.level == 2).collect();
        assert_eq!(days.len(), 3);
        assert!(
            ranges.len() <= MAX_SEGMENTS && ranges.len() >= 3 * 24,
            "{} ranges",
            ranges.len()
        );
        assert!(days.iter().all(|d| d.errors == 24));
        assert_eq!(ranges.iter().map(|r| r.errors).sum::<u32>(), 72);
        assert_eq!(result.errors, 72);
        assert!(result.complete);
    }

    #[test]
    fn logs_without_timestamps_list_their_error_lines() {
        let build = "Compiling foo v0.1.0\n\
                     warning: unused variable `x`\n\
                     error[E0308]: mismatched types\n\
                     src/main.rs:3:5: error: expected one\n\
                     Traceback (most recent call last):\n  File \"a.py\", line 1\n";
        let result = outline(Format::Log, build);
        assert_eq!(result.kind, OutlineKind::Problems);
        assert_eq!(
            entries(&result),
            [
                (1, "error[E0308]: mismatched types"),
                (1, "src/main.rs:3:5: error: expected one"),
                (1, "Traceback (most recent call last):"),
            ]
        );
        assert_eq!((result.errors, result.warnings), (2, 1));
        let empty = outline(Format::Log, "hello\nworld\n");
        assert_eq!(
            (empty.kind, empty.headings.len()),
            (OutlineKind::Problems, 0)
        );
    }

    #[test]
    fn a_log_index_reads_the_whole_file_and_reports_progress() {
        let line = b"2026-10-24 08:15:30,123 INFO request done\n";
        let size = MAX_SCAN_BYTES + 3 * CHUNK_BYTES as u64;
        let (epoch, progress) = (AtomicU64::new(0), AtomicU64::new(0));
        let fill = |out: &mut [u8], offset: u64| -> io::Result<usize> {
            for (i, byte) in out.iter_mut().enumerate() {
                *byte = line[(offset as usize + i) % line.len()];
            }
            Ok(out.len())
        };
        let log = scan_reader(
            (Format::Log, Encoding::Utf8),
            size,
            &epoch,
            0,
            &progress,
            fill,
        )
        .unwrap()
        .unwrap();
        assert_eq!(log.scanned, size);
        assert_eq!(progress.load(Ordering::Relaxed), size);
        assert!(log.complete);
        assert_eq!(log.headings.len(), 1);
        // Markdown keeps its 16 MiB bound.
        let progress = AtomicU64::new(0);
        let markdown = scan_reader(
            (Format::Markdown, Encoding::Utf8),
            size,
            &epoch,
            0,
            &progress,
            fill,
        )
        .unwrap()
        .unwrap();
        assert_eq!(markdown.scanned, MAX_SCAN_BYTES);
        assert!(markdown.limited);
    }
}
