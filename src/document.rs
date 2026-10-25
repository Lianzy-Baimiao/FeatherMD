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
    /// Literal, case-sensitive, wrap-around search in the document's encoding. Checks
    /// cancellation every 256 KiB.
    pub fn search(
        &self,
        query: &str,
        from: u64,
        epoch: &AtomicU64,
        ticket: u64,
    ) -> io::Result<SearchResult> {
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::i18n::tr("搜索词长度应为 1～16384 字节"),
            ));
        }
        // A legacy page is searched for the query's bytes in that page.
        let (needle, legacy): (Vec<u8>, Option<Codec>) = match self.encoding {
            Encoding::Utf8 => (query.as_bytes().to_vec(), None),
            Encoding::Legacy(codec) => match codec.encode(query) {
                Some(bytes) if !bytes.is_empty() => (bytes, Some(codec)),
                _ => return Ok(SearchResult::NotFound),
            },
        };
        let size = self.file.metadata()?.len();
        let from = from.min(size);
        for (start, end) in [
            (from, size),
            (0, from.saturating_add(needle.len() as u64 - 1).min(size)),
        ] {
            let result = self.search_range(&needle, legacy, start, end, epoch, ticket)?;
            if result != SearchResult::NotFound {
                return Ok(result);
            }
        }
        Ok(SearchResult::NotFound)
    }
    fn search_range(
        &self,
        query: &[u8],
        legacy: Option<Codec>,
        start: u64,
        end: u64,
        epoch: &AtomicU64,
        ticket: u64,
    ) -> io::Result<SearchResult> {
        let mut buf = vec![0; SEARCH_BYTES + query.len()];
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
            // In a double-byte page the bytes may also sit across two characters.
            let hit = memchr::memmem::find_iter(&buf[..used], query)
                .find(|&i| legacy.is_none_or(|codec| codec.aligned(&buf[..used], i)));
            if let Some(i) = hit {
                return Ok(SearchResult::Found(offset - carry as u64 + i as u64));
            }
            carry = (query.len() - 1).min(used);
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
