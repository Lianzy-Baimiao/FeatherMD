//! Text encodings: UTF-8, or for a file that is clearly not UTF-8 the system's legacy
//! code page (GB18030 for the Chinese GBK page, Big5, Shift-JIS, …), as Notepad does.
//! Legacy pages are decoded by Windows; elsewhere every file is read as UTF-8.

/// How a document's bytes become text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Utf8,
    /// A Windows code page.
    Legacy(Codec),
}
impl Encoding {
    /// Shown in the status bar.
    pub fn name(&self) -> String {
        match self {
            Self::Utf8 => "UTF-8".into(),
            Self::Legacy(codec) => codec.name(),
        }
    }
}

/// A legacy code page: single-byte, double-byte (DBCS) or GB18030 (two and four
/// bytes). Character boundaries are found by walking from a known one.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    pub page: u32,
    /// Bytes that start a multi-byte character, as a bit set.
    lead: [u64; 4],
    gb18030: bool,
}
impl std::fmt::Debug for Codec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Codec({})", self.page)
    }
}
impl Codec {
    /// GB18030 (code page 54936), which also reads GBK and GB2312 text.
    pub fn gb18030() -> Self {
        Self::with_leads(54936, &[(0x81, 0xfe)], true)
    }
    /// A code page whose multi-byte characters start with bytes in `ranges`.
    pub fn with_leads(page: u32, ranges: &[(u8, u8)], gb18030: bool) -> Self {
        let mut lead = [0; 4];
        for &(from, to) in ranges {
            for b in from..=to {
                lead[usize::from(b >> 6)] |= 1 << (b & 63);
            }
        }
        Self {
            page,
            lead,
            gb18030,
        }
    }
    fn is_lead(&self, b: u8) -> bool {
        (self.lead[usize::from(b >> 6)] & (1 << (b & 63))) != 0
    }
    fn single_byte(&self) -> bool {
        self.lead == [0; 4]
    }
    /// Length of the character that starts `b` (1 for an invalid or lone byte).
    pub fn char_len(&self, b: &[u8]) -> usize {
        match b.first() {
            None => 0,
            Some(&first) if !self.is_lead(first) => 1,
            _ if self.gb18030 && b.get(1).is_some_and(u8::is_ascii_digit) => 4,
            _ => 2,
        }
    }
    /// The last character boundary at or before `limit` in `b`, which starts on one.
    pub fn floor(&self, b: &[u8], limit: usize) -> usize {
        let limit = limit.min(b.len());
        let mut i = 0;
        while i < limit {
            let next = i + self.char_len(&b[i..]).max(1);
            if next > limit {
                break;
            }
            i = next;
        }
        i
    }
    /// A character starts after `b` wherever `b` is: no multi-byte character
    /// continues with it (trail bytes are 0x40 or above; GB18030 also uses digits).
    pub fn sync(&self, b: u8) -> bool {
        self.single_byte() || (b < 0x40 && !(self.gb18030 && b.is_ascii_digit()))
    }
    /// `bytes` start on a character boundary at `at`, walking from the last byte
    /// after which one certainly starts.
    pub fn aligned(&self, bytes: &[u8], at: usize) -> bool {
        let from = bytes[..at]
            .iter()
            .rposition(|&b| self.sync(b))
            .map_or(0, |i| i + 1);
        let mut i = from;
        while i < at {
            i += self.char_len(&bytes[i..]).max(1);
        }
        i == at
    }
    pub fn name(&self) -> String {
        match self.page {
            54936 => "GB18030".into(),
            950 => "Big5".into(),
            932 => "Shift-JIS".into(),
            949 => "EUC-KR".into(),
            page => format!("CP{page}"),
        }
    }
}
impl Codec {
    /// `bytes` as text, and whether any were invalid (replaced).
    pub fn decode(&self, bytes: &[u8]) -> (String, bool) {
        system::decode(self.page, bytes)
    }
    /// `text` in this code page; None when it has characters the page cannot hold.
    pub fn encode(&self, text: &str) -> Option<Vec<u8>> {
        system::encode(self.page, text)
    }
}

/// The encoding of a file from its first bytes: UTF-8 unless they are clearly not
/// (more invalid bytes than valid multi-byte characters; no NUL) and `legacy` reads
/// them without error, as Notepad falls back to the ANSI page.
pub fn detect(sample: &[u8], legacy: Option<Codec>) -> Encoding {
    if sample.starts_with(b"\xef\xbb\xbf") || sample.contains(&0) || mostly_utf8(sample) {
        return Encoding::Utf8;
    }
    match legacy {
        Some(codec) => {
            let end = codec.floor(sample, sample.len());
            if codec.decode(&sample[..end]).1 {
                Encoding::Utf8
            } else {
                Encoding::Legacy(codec)
            }
        }
        None => Encoding::Utf8,
    }
}
/// No invalid UTF-8, or at least as many valid multi-byte characters as invalid
/// bytes (UTF-8 text with stray bytes). Legacy CJK text reads as about one valid
/// UTF-8 character per ten invalid bytes, Western ANSI text as none.
fn mostly_utf8(sample: &[u8]) -> bool {
    let multibyte = |text: &str| text.chars().filter(|c| !c.is_ascii()).count();
    let (mut good, mut bad) = (0, 0);
    let mut rest = sample;
    loop {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                good += multibyte(valid);
                break;
            }
            Err(error) => {
                let (valid, after) = rest.split_at(error.valid_up_to());
                good += multibyte(std::str::from_utf8(valid).unwrap_or_default());
                match error.error_len() {
                    Some(n) => {
                        bad += 1;
                        rest = &after[n..];
                    }
                    // The sample ends inside a character.
                    None => break,
                }
            }
        }
    }
    bad == 0 || good >= bad
}

pub use system::{codec as system_codec, page_codec};

#[cfg(windows)]
mod system {
    use super::Codec;
    use windows_sys::Win32::Globalization::{
        CPINFO, GetACP, GetCPInfo, MB_ERR_INVALID_CHARS, MultiByteToWideChar, WideCharToMultiByte,
    };

    /// The system's legacy (ANSI) code page; None when it is UTF-8.
    pub fn codec() -> Option<Codec> {
        page_codec(unsafe { GetACP() })
    }
    /// A Windows code page: GBK (936) reads as GB18030, its superset; UTF-8, UTF-16
    /// and pages wider than two bytes have none.
    pub fn page_codec(page: u32) -> Option<Codec> {
        if page == 936 || page == 54936 {
            return Some(Codec::gb18030());
        }
        if matches!(page, 65001 | 1200 | 1201) {
            return None;
        }
        let mut info: CPINFO = unsafe { std::mem::zeroed() };
        if unsafe { GetCPInfo(page, &mut info) } == 0 || info.MaxCharSize > 2 {
            return None;
        }
        let ranges: Vec<(u8, u8)> = info
            .LeadByte
            .chunks(2)
            .take_while(|r| r[0] != 0)
            .map(|r| (r[0], r[1]))
            .collect();
        Some(Codec::with_leads(page, &ranges, false))
    }
    pub fn decode(page: u32, bytes: &[u8]) -> (String, bool) {
        if bytes.is_empty() {
            return (String::new(), false);
        }
        let convert = |flags| -> Option<Vec<u16>> {
            let (from, n) = (bytes.as_ptr(), bytes.len() as i32);
            let len = unsafe { MultiByteToWideChar(page, flags, from, n, std::ptr::null_mut(), 0) };
            if len <= 0 {
                return None;
            }
            let mut wide = vec![0u16; len as usize];
            let written =
                unsafe { MultiByteToWideChar(page, flags, from, n, wide.as_mut_ptr(), len) };
            (written == len).then_some(wide)
        };
        match convert(MB_ERR_INVALID_CHARS) {
            Some(wide) => (String::from_utf16_lossy(&wide), false),
            None => match convert(0) {
                Some(wide) => (String::from_utf16_lossy(&wide), true),
                None => (String::from_utf8_lossy(bytes).into_owned(), true),
            },
        }
    }
    pub fn encode(page: u32, text: &str) -> Option<Vec<u8>> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return Some(Vec::new());
        }
        // GB18030 holds all of Unicode and takes no default-character report.
        let mut lossy = 0;
        let used: *mut i32 = if page == 54936 {
            std::ptr::null_mut()
        } else {
            &raw mut lossy
        };
        let (from, n) = (wide.as_ptr(), wide.len() as i32);
        let null = std::ptr::null();
        let len =
            unsafe { WideCharToMultiByte(page, 0, from, n, std::ptr::null_mut(), 0, null, used) };
        if len <= 0 || lossy != 0 {
            return None;
        }
        let mut out = vec![0u8; len as usize];
        let written =
            unsafe { WideCharToMultiByte(page, 0, from, n, out.as_mut_ptr(), len, null, used) };
        (written == len && lossy == 0).then_some(out)
    }
}

#[cfg(not(windows))]
mod system {
    use super::Codec;
    pub fn codec() -> Option<Codec> {
        None
    }
    pub fn page_codec(_: u32) -> Option<Codec> {
        None
    }
    pub fn decode(_: u32, bytes: &[u8]) -> (String, bool) {
        let text = String::from_utf8_lossy(bytes);
        let bad = matches!(text, std::borrow::Cow::Owned(_));
        (text.into_owned(), bad)
    }
    pub fn encode(_: u32, _: &str) -> Option<Vec<u8>> {
        None
    }
}
