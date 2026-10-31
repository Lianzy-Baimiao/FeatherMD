//! Bounded positional reads, safe even when the file is edited or truncated externally.
use crate::encoding::{self, Codec, Encoding};
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
pub const PAGE_BYTES: usize = 128 * 1024;
pub const SEGMENT_BYTES: usize = 4096;
const LOOK_BEHIND: u64 = 32 * 1024;
const SEARCH_BYTES: usize = 256 * 1024;
pub const MAX_QUERY_BYTES: usize = 16 * 1024;
const DETECT_BYTES: usize = 64 * 1024;
/// Matches one find-all scan lists; the scan stops at the next one.
pub const MAX_MATCHES: usize = 1000;
/// Bytes of the line read on each side of a listed match.
const CONTEXT_BYTES: usize = 240;
#[derive(Debug)]
pub struct Document {
    file: File,
    pub path: PathBuf,
    pub size: u64,
    /// UTF-8, or the system's legacy code page for a file that is clearly not UTF-8.
    pub encoding: Encoding,
    /// Read as Markdown, plain text or a log: from the name at open, refined from the
    /// first page by `plain::sniff`.
    pub format: crate::plain::Format,
}
#[derive(Clone, Debug)]
pub struct SourceLine {
    pub start: u64,
    pub end: u64,
    pub text: String,
    pub continued: bool,
}
#[derive(Clone, Debug)]
pub struct Page {
    pub lines: Vec<SourceLine>,
    pub start: u64,
    pub end: u64,
    pub size: u64,
    pub partial_context: bool,
    /// Some bytes were invalid in the document's encoding and were replaced.
    pub invalid_utf8: bool,
    pub encoding: Encoding,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SearchResult {
    Found(u64),
    NotFound,
    Cancelled,
}
/// What to search for: the text, and whether letter case must match (otherwise
/// ASCII letters match either case).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query<'a> {
    pub text: &'a str,
    pub match_case: bool,
}
impl<'a> From<&'a str> for Query<'a> {
    /// A literal, case-sensitive query.
    fn from(text: &'a str) -> Self {
        Self {
            text,
            match_case: true,
        }
    }
}
impl<'a> From<&'a String> for Query<'a> {
    fn from(text: &'a String) -> Self {
        Query::from(text.as_str())
    }
}
/// The bytes searched for one form of a query, with that form's text.
struct Needle {
    text: String,
    bytes: Vec<u8>,
}
/// One listed match: where it is and the text of its line around it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Match {
    pub offset: u64,
    /// Line number, from 1.
    pub line: u64,
    /// The matched text as the file has it (its case; Markdown escapes removed).
    pub text: String,
    /// Up to `CONTEXT_BYTES` of the line before and after the match, as text.
    pub before: String,
    pub after: String,
}
/// Every match of a query in file order, at most `MAX_MATCHES`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Matches {
    pub items: Vec<Match>,
    /// The file has more matches than are listed.
    pub more: bool,
}
fn read_at(file: &File, out: &mut [u8], offset: u64) -> io::Result<usize> {
    #[cfg(windows)]
    {
        file.seek_read(out, offset)
    }
    #[cfg(unix)]
    {
        file.read_at(out, offset)
    }
}
fn fill_at(file: &File, out: &mut [u8], offset: u64) -> io::Result<usize> {
    let mut n = 0;
    while n < out.len() {
        match read_at(file, &mut out[n..], offset + n as u64) {
            Ok(0) => break,
            Ok(read) => n += read,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}
fn continuation(b: u8) -> bool {
    b & 0xc0 == 0x80
}
/// Context bytes as one line of text: tabs and control characters become spaces.
fn snippet(bytes: &[u8], legacy: Option<Codec>) -> String {
    let text = match legacy {
        Some(codec) => codec.decode(bytes).0,
        None => String::from_utf8_lossy(bytes).into_owned(),
    };
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
impl Document {
    pub(crate) fn current_size(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }
    /// A caller-owned buffer cannot increase one background read beyond 256 KiB.
    pub(crate) fn read_bounded(&self, out: &mut [u8], offset: u64) -> io::Result<usize> {
        let len = out.len().min(SEARCH_BYTES);
        fill_at(&self.file, &mut out[..len], offset)
    }
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path.as_ref())?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::i18n::tr("请选择普通文件"),
            ));
        }
        // The first 64 KiB decide the encoding; a page read stays bounded as before.
        let mut sample = vec![0u8; DETECT_BYTES];
        let n = fill_at(&file, &mut sample, 0)?;
        sample.truncate(n);
        let mut bom = [0u8; 4];
        bom[..n.min(4)].copy_from_slice(&sample[..n.min(4)]);
        if n >= 2 && (bom.starts_with(&[0xff, 0xfe]) || bom.starts_with(&[0xfe, 0xff])) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                crate::i18n::tr(
                    "此文件是 UTF-16/UTF-32，请先另存为 UTF-8 再打开（不会修改原文件）",
                ),
            ));
        }
        if n == 4 && bom == [0, 0, 0xfe, 0xff] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                crate::i18n::tr("此文件是 UTF-32，请先另存为 UTF-8"),
            ));
        }
        Ok(Self {
            file,
            path: path.as_ref().to_owned(),
            size: meta.len(),
            encoding: encoding::detect(&sample, encoding::system_codec()),
            format: crate::plain::detect(path.as_ref()),
        })
    }
    /// Reads at most PAGE_BYTES + 4 bytes, even for a single multi-gigabyte line.
    pub fn page(&self, anchor: u64) -> io::Result<Page> {
        let size = self.file.metadata()?.len();
        let anchor = anchor.min(size.saturating_sub(1));
        let mut start = anchor.saturating_sub(LOOK_BEHIND);
        let mut bytes = vec![0; PAGE_BYTES + 4];
        let n = fill_at(&self.file, &mut bytes, start)?;
        bytes.truncate(n);
        let legacy = match self.encoding {
            Encoding::Legacy(codec) => Some(codec),
            Encoding::Utf8 => None,
        };
        // The buffer may end halfway through a UTF-8 scalar, not a corrupt file. (A
        // legacy page's segments stop at whole characters as they are walked.)
        if start + (n as u64) < size && n > 0 && legacy.is_none() {
            let mut lead = n - 1;
            while lead > 0 && continuation(bytes[lead]) {
                lead -= 1;
            }
            let expected = match bytes[lead] {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            };
            if n - lead < expected {
                bytes.truncate(lead);
            }
        }
        let mut begin = 0;
        if start == 0 && bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            begin = 3;
        } else if start > 0 {
            let head = &bytes[..bytes.len().min(SEGMENT_BYTES)];
            if let Some(i) = memchr::memchr(b'\n', head) {
                begin = i + 1;
            } else if let Some(codec) = legacy {
                // Inside a long line: after a byte that no character continues with.
                begin = head
                    .iter()
                    .position(|&b| codec.sync(b))
                    .map_or(0, |i| i + 1);
            } else {
                while begin < bytes.len() && continuation(bytes[begin]) {
                    begin += 1;
                }
            }
        }
        start += begin as u64;
        let data = &bytes[begin..];
        let mut lines = Vec::new();
        let mut pos = 0;
        let mut invalid_utf8 = false;
        while pos < data.len() && pos < PAGE_BYTES.saturating_sub(begin + 4) {
            let limit = (pos + SEGMENT_BYTES).min(data.len());
            let nl = memchr::memchr(b'\n', &data[pos..limit]);
            let mut end = nl.map_or(limit, |i| pos + i + 1);
            if nl.is_none() && start + (end as u64) < size {
                match legacy {
                    // Walk from the segment's start, which is a character boundary.
                    Some(codec) => end = pos + codec.floor(&data[pos..], end - pos),
                    None => {
                        while end > pos && end < data.len() && continuation(data[end]) {
                            end -= 1;
                        }
                    }
                }
            }
            if end == pos {
                break;
            }
            let raw = &data[pos..end];
            let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            let (text, bad) = match legacy {
                Some(codec) => codec.decode(raw),
                None => {
                    let text = String::from_utf8_lossy(raw);
                    let bad = matches!(text, std::borrow::Cow::Owned(_));
                    (text.into_owned(), bad)
                }
            };
            invalid_utf8 |= bad;
            lines.push(SourceLine {
                start: start + pos as u64,
                end: start + end as u64,
                text,
                continued: nl.is_none() && start + (end as u64) < size,
            });
            pos = end;
        }
        // Retain a neighborhood around the anchor. Even millions of blank lines cannot
        // force an unbounded number of native layout calls on the UI thread.
        let first = lines
            .partition_point(|line| line.end <= anchor)
            .saturating_sub(128);
        if first > 0 {
            lines.drain(..first);
        }
        lines.truncate(2048);
        let start = lines.first().map_or(start, |l| l.start);
        let end = lines.last().map_or(start, |l| l.end);
        Ok(Page {
            lines,
            start,
            end,
            size,
            partial_context: start > 3,
            invalid_utf8,
            encoding: self.encoding,
        })
    }
    /// The bytes a query is searched for, in the document's encoding: the query
    /// itself and, in Markdown, its punctuation backslash-escaped as converters
    /// write it (`T\_NAME` for `T_NAME`: only `_` and `*`, then all ASCII
    /// punctuation). Empty when the query has no form in a legacy page. The flag
    /// says whether windows must be folded to lower case: only when letters match
    /// either case and some needle byte is an ASCII letter (folding leaves every
    /// other byte as it is, so a query without letters is found as written).
    fn needles(&self, query: Query) -> (Vec<Needle>, Option<Codec>, bool) {
        let fold = !query.match_case;
        let query = query.text;
        let mut texts = vec![query.to_owned()];
        if self.format == crate::plain::Format::Markdown {
            let escape = |all: bool| {
                let mut out = String::with_capacity(query.len() * 2);
                for c in query.chars() {
                    if c == '_' || c == '*' || (all && c.is_ascii_punctuation()) {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out
            };
            for text in [escape(false), escape(true)] {
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
        if fold {
            // Letters of either case: the needles and the windows are searched in
            // lower case (ASCII only, so no byte moves).
            for text in &mut texts {
                text.make_ascii_lowercase();
            }
            let mut unique: Vec<String> = Vec::new();
            for text in texts {
                if !unique.contains(&text) {
                    unique.push(text);
                }
            }
            texts = unique;
        }
        let (mut needles, legacy): (Vec<Needle>, Option<Codec>) = match self.encoding {
            Encoding::Utf8 => (
                texts
                    .into_iter()
                    .map(|text| Needle {
                        bytes: text.clone().into_bytes(),
                        text,
                    })
                    .collect(),
                None,
            ),
            Encoding::Legacy(codec) => (
                texts
                    .into_iter()
                    .filter_map(|text| {
                        let bytes = codec.encode(&text)?;
                        (!bytes.is_empty()).then_some(Needle { text, bytes })
                    })
                    .collect(),
                Some(codec),
            ),
        };
        let fold = fold
            && needles
                .iter()
                .any(|n| n.bytes.iter().any(u8::is_ascii_alphabetic));
        if fold {
            // The bytes fold as the windows do: a trail byte that is a capital letter
            // in a legacy page must still meet its folded self.
            for needle in &mut needles {
                needle.bytes.make_ascii_lowercase();
            }
        }
        (needles, legacy, fold)
    }
    /// A match of `needle` at `i` in `window` is real: in a legacy page whose
    /// letters were folded, a trail byte that only looked like a letter of the
    /// other case must not count, so the bytes must read as the needle's text.
    fn real(window: &[u8], i: usize, needle: &Needle, legacy: Option<Codec>, fold: bool) -> bool {
        let Some(codec) = legacy else {
            return true;
        };
        if !codec.aligned(window, i) {
            return false;
        }
        !fold
            || window.get(i..i + needle.bytes.len()).is_some_and(|bytes| {
                let (text, bad) = codec.decode(bytes);
                !bad && text.eq_ignore_ascii_case(&needle.text)
            })
    }
    /// Literal, wrap-around search in the document's encoding (in Markdown also for
    /// backslash-escaped punctuation); case-sensitive unless the query says letters
    /// match either case. Checks cancellation every 256 KiB.
    pub fn search<'a>(
        &self,
        query: impl Into<Query<'a>>,
        from: u64,
        epoch: &AtomicU64,
        ticket: u64,
    ) -> io::Result<SearchResult> {
        let query = query.into();
        if query.text.is_empty() || query.text.len() > MAX_QUERY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::i18n::tr("搜索词长度应为 1～16384 字节"),
            ));
        }
        // A legacy page is searched for the query's bytes in that page.
        let (needles, legacy, fold) = self.needles(query);
        let Some(longest) = needles.iter().map(|n| n.bytes.len()).max() else {
            return Ok(SearchResult::NotFound);
        };
        let size = self.file.metadata()?.len();
        let from = from.min(size);
        for (start, end) in [
            (from, size),
            (0, from.saturating_add(longest as u64 - 1).min(size)),
        ] {
            let result = self.search_range(&needles, (legacy, fold), start, end, epoch, ticket)?;
            if result != SearchResult::NotFound {
                return Ok(result);
            }
        }
        Ok(SearchResult::NotFound)
    }
    /// Every literal match of `query` in file order (in Markdown also of its
    /// backslash-escaped forms; ASCII letters of either case unless the query
    /// matches case), at most `MAX_MATCHES` (then `more`), with line numbers, the
    /// matched text and the text around each (Markdown escapes removed).
    /// Reads 256 KiB at a time, checking cancellation and reporting the bytes read
    /// to `progress`; None when cancelled.
    pub fn find_all<'a>(
        &self,
        query: impl Into<Query<'a>>,
        epoch: &AtomicU64,
        ticket: u64,
        progress: &AtomicU64,
    ) -> io::Result<Option<Matches>> {
        let query = query.into();
        if query.text.is_empty() || query.text.len() > MAX_QUERY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::i18n::tr("搜索词长度应为 1～16384 字节"),
            ));
        }
        let (needles, legacy, fold) = self.needles(query);
        let Some(longest) = needles.iter().map(|n| n.bytes.len()).max() else {
            return Ok(Some(Matches::default()));
        };
        let size = self.file.metadata()?.len();
        let mut buf = vec![0; SEARCH_BYTES + longest];
        // The window in lower case when letters match either case.
        let mut folded: Vec<u8> = Vec::new();
        // (offset, line, length) of each listed match.
        let mut found: Vec<(u64, u64, usize)> = Vec::new();
        let mut more = false;
        let (mut offset, mut carry) = (0u64, 0usize);
        // Newlines are counted up to file offset `counted`; matches may not
        // overlap, so the next one starts at `next` or later.
        let (mut counted, mut lines, mut next) = (0u64, 1u64, 0u64);
        'scan: while offset < size {
            if epoch.load(Ordering::Relaxed) != ticket {
                return Ok(None);
            }
            let want = (size - offset).min(SEARCH_BYTES as u64) as usize;
            let read = fill_at(&self.file, &mut buf[carry..carry + want], offset)?;
            if read == 0 {
                break;
            }
            let used = carry + read;
            let base = offset - carry as u64;
            // The needles' matches merged in file order, each needle's lazily.
            let window = &buf[..used];
            let hay = if fold {
                folded.clear();
                folded.extend_from_slice(window);
                folded.make_ascii_lowercase();
                &folded[..]
            } else {
                window
            };
            let mut hits: Vec<_> = needles
                .iter()
                .enumerate()
                .map(|(k, n)| {
                    memchr::memmem::find_iter(hay, &n.bytes)
                        .map(move |i| (i, k))
                        .peekable()
                })
                .collect();
            while let Some((i, k)) = hits.iter_mut().filter_map(|h| h.peek().copied()).min() {
                for h in hits.iter_mut() {
                    if h.peek().is_some_and(|&(j, _)| j == i) {
                        h.next();
                    }
                }
                let len = needles[k].bytes.len();
                let at = base + i as u64;
                if at < next || !Self::real(window, i, &needles[k], legacy, fold) {
                    continue;
                }
                if found.len() == MAX_MATCHES {
                    more = true;
                    break 'scan;
                }
                let from = (counted.saturating_sub(base) as usize).min(i);
                lines += memchr::memchr_iter(b'\n', &window[from..i]).count() as u64;
                counted = counted.max(at);
                found.push((at, lines, len));
                next = at + len as u64;
            }
            carry = (longest - 1).min(used);
            let keep = used - carry;
            let from = (counted.saturating_sub(base) as usize).min(keep);
            lines += memchr::memchr_iter(b'\n', &buf[from..keep]).count() as u64;
            counted = counted.max(base + keep as u64);
            buf.copy_within(keep..used, 0);
            offset += read as u64;
            progress.store(offset, Ordering::Relaxed);
            if read < want {
                break;
            }
        }
        let mut items = Vec::with_capacity(found.len());
        let markdown = self.format == crate::plain::Format::Markdown;
        for (n, (at, line, len)) in found.into_iter().enumerate() {
            if n % 64 == 0 && epoch.load(Ordering::Relaxed) != ticket {
                return Ok(None);
            }
            let (mut before, mut after) = self.context(at, len, legacy, size)?;
            let mut matched = vec![0; len];
            let n = fill_at(&self.file, &mut matched, at)?;
            let mut text = snippet(&matched[..n], legacy);
            if markdown {
                before = crate::markdown::unescape(&before);
                after = crate::markdown::unescape(&after);
                text = crate::markdown::unescape(&text);
            }
            items.push(Match {
                offset: at,
                line,
                text,
                before,
                after,
            });
        }
        Ok(Some(Matches { items, more }))
    }
    /// The text of the line on each side of the match at `at`, `CONTEXT_BYTES` at
    /// most each way, cut at whole characters; leading indentation dropped.
    fn context(
        &self,
        at: u64,
        len: usize,
        legacy: Option<Codec>,
        size: u64,
    ) -> io::Result<(String, String)> {
        let start = at.saturating_sub(CONTEXT_BYTES as u64);
        let mut head = vec![0; (at - start) as usize];
        let n = fill_at(&self.file, &mut head, start)?;
        head.truncate(n);
        let from = match memchr::memrchr(b'\n', &head) {
            Some(i) => i + 1,
            None if start == 0 => 0,
            // Inside a long line: from the first character boundary.
            None => match legacy {
                Some(codec) => head
                    .iter()
                    .position(|&b| codec.sync(b))
                    .map_or(head.len(), |i| i + 1),
                None => head
                    .iter()
                    .position(|&b| !continuation(b))
                    .unwrap_or(head.len()),
            },
        };
        let mut head = &head[from..];
        if start == 0 && from == 0 && legacy.is_none() {
            head = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
        }
        let tail_start = (at + len as u64).min(size);
        let mut tail = vec![0; ((size - tail_start) as usize).min(CONTEXT_BYTES)];
        let n = fill_at(&self.file, &mut tail, tail_start)?;
        tail.truncate(n);
        let cut = memchr::memchr2(b'\n', b'\r', &tail);
        let mut end = cut.unwrap_or(tail.len());
        if cut.is_none() {
            end = match legacy {
                Some(codec) => codec.floor(&tail, end),
                None => match std::str::from_utf8(&tail[..end]) {
                    Ok(_) => end,
                    Err(e) if e.error_len().is_none() => e.valid_up_to(),
                    Err(_) => end,
                },
            };
        }
        let before = snippet(head, legacy);
        Ok((
            before.trim_start().to_owned(),
            snippet(&tail[..end], legacy),
        ))
    }
    /// The first match of any of `needles` in `start..end`.
    fn search_range(
        &self,
        needles: &[Needle],
        (legacy, fold): (Option<Codec>, bool),
        start: u64,
        end: u64,
        epoch: &AtomicU64,
        ticket: u64,
    ) -> io::Result<SearchResult> {
        let longest = needles.iter().map(|n| n.bytes.len()).max().unwrap_or(1);
        let mut buf = vec![0; SEARCH_BYTES + longest];
        let mut folded: Vec<u8> = Vec::new();
        let mut offset = start;
        let mut carry = 0;
        while offset < end {
            if epoch.load(Ordering::Relaxed) != ticket {
                return Ok(SearchResult::Cancelled);
            }
            let want = (end - offset).min(SEARCH_BYTES as u64) as usize;
            let read = fill_at(&self.file, &mut buf[carry..carry + want], offset)?;
            if read == 0 {
                break;
            }
            let used = carry + read;
            let window = &buf[..used];
            let hay = if fold {
                folded.clear();
                folded.extend_from_slice(window);
                folded.make_ascii_lowercase();
                &folded[..]
            } else {
                window
            };
            // In a double-byte page the bytes may also sit across two characters.
            let hit = needles
                .iter()
                .filter_map(|needle| {
                    memchr::memmem::find_iter(hay, &needle.bytes)
                        .find(|&i| Self::real(window, i, needle, legacy, fold))
                })
                .min();
            if let Some(i) = hit {
                return Ok(SearchResult::Found(offset - carry as u64 + i as u64));
            }
            carry = (longest - 1).min(used);
            buf.copy_within(used - carry..used, 0);
            offset += read as u64;
        }
        Ok(SearchResult::NotFound)
    }
}
pub fn human_size(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}
