//! Update checks against GitHub Releases: version comparison, bounded JSON
//! field extraction and scheduling. Pure logic; the Windows reader performs the
//! single HTTPS request. Nothing here downloads, installs or runs anything.
use crate::i18n::{self, Lang, tr, trf};
use std::{cmp::Ordering, fmt};

pub const REPOSITORY: &str = "Lianzy-Baimiao/FeatherMD";
pub const LATEST_API: &str =
    "https://api.github.com/repos/Lianzy-Baimiao/FeatherMD/releases/latest";
/// Automatic checks run at most once per day.
pub const CHECK_INTERVAL: u64 = 24 * 60 * 60;
/// A release record is about 10 KiB; refuse anything far larger.
pub const MAX_RESPONSE: usize = 1 << 20;

/// `[v]MAJOR.MINOR.PATCH[-pre][+build]`. A pre-release sorts before its release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    core: [u32; 3],
    pre: Option<String>,
}
impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let text = text.split_once('+').map_or(text, |(version, _)| version);
        if text.len() > 64 {
            return None;
        }
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut numbers = [0u32; 3];
        for number in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || part.len() > 9 || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *number = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre)
                if !pre.is_empty()
                    && !pre.split('.').any(str::is_empty)
                    && pre
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') =>
            {
                Some(pre.to_string())
            }
            Some(_) => return None,
            None => None,
        };
        Some(Self { core: numbers, pre })
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => {
                    // SemVer: numeric identifiers compare numerically and sort first.
                    let mut a = a.split('.');
                    let mut b = b.split('.');
                    loop {
                        match (a.next(), b.next()) {
                            (None, None) => return Ordering::Equal,
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            (Some(x), Some(y)) => {
                                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                                    (Ok(x), Ok(y)) => x.cmp(&y),
                                    (Ok(_), Err(_)) => Ordering::Less,
                                    (Err(_), Ok(_)) => Ordering::Greater,
                                    (Err(_), Err(_)) => x.cmp(y),
                                };
                                if order != Ordering::Equal {
                                    return order;
                                }
                            }
                        }
                    }
                }
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.core;
        write!(f, "{major}.{minor}.{patch}")?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// The version of this build.
pub fn current() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version {
        core: [0, 0, 0],
        pre: None,
    })
}

/// Tags end up in URLs and the settings file: ASCII letters, digits, `.`, `-`, `_`.
pub fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// The release page for a tag, built from the fixed repository rather than
/// any URL in the response.
pub fn release_page(tag: &str) -> Option<String> {
    valid_tag(tag).then(|| format!("https://github.com/{REPOSITORY}/releases/tag/{tag}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    /// Display title without control or bidirectional-override characters.
    pub title: String,
    /// `YYYY-MM-DD`, or empty when GitHub did not report a valid date.
    pub published: String,
}
impl Release {
    pub fn page(&self) -> String {
        release_page(&self.tag)
            .unwrap_or_else(|| format!("https://github.com/{REPOSITORY}/releases"))
    }
}

/// Reads `tag_name`, `name`, `published_at`, `draft` and `prerelease` from the
/// top level of a GitHub "latest release" response. Nested values are skipped.
pub fn parse_latest(json: &[u8]) -> Result<Release, String> {
    if json.len() > MAX_RESPONSE {
        return Err(tr("更新信息过大").into());
    }
    let fields = fields(json).ok_or(tr("更新信息格式无效"))?;
    if fields.draft || fields.prerelease {
        return Err(tr("最新发布不是正式版本").into());
    }
    let tag = fields.tag.ok_or(tr("最新发布缺少版本号"))?;
    if !valid_tag(&tag) {
        return Err(tr("最新发布的版本号无效").into());
    }
    let version = Version::parse(&tag).ok_or_else(|| trf("无法识别的版本号：{}", &[&tag]))?;
    let title = fields
        .name
        .map_or_else(String::new, |name| clean(&name, 120));
    let published = fields
        .published
        .filter(|date| {
            let b = date.as_bytes();
            b.len() >= 10
                && b[..10].iter().enumerate().all(|(i, c)| {
                    if i == 4 || i == 7 {
                        *c == b'-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
        })
        .map(|date| date[..10].to_string())
        .unwrap_or_default();
    Ok(Release {
        tag,
        version,
        title,
        published,
    })
}

fn clean(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Automatic checks: enabled and at least a day since the last recorded one.
/// A clock that moved backwards also allows a check.
pub fn due(enabled: bool, last: u64, now: u64) -> bool {
    enabled && (last > now || now - last >= CHECK_INTERVAL)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "上次检查" wording for a Unix timestamp, in the current interface language.
pub fn ago(last: u64, now: u64) -> String {
    ago_in(i18n::current(), last, now)
}

pub fn ago_in(lang: Lang, last: u64, now: u64) -> String {
    if last == 0 {
        return i18n::tr_in(lang, "尚未检查").into();
    }
    let (count, one, many) = match now.saturating_sub(last) {
        0..60 => return i18n::tr_in(lang, "刚刚").into(),
        elapsed @ 60..3600 => (
            elapsed / 60,
            crate::key!("1 分钟前"),
            crate::key!("{} 分钟前"),
        ),
        elapsed @ 3600..86400 => (
            elapsed / 3600,
            crate::key!("1 小时前"),
            crate::key!("{} 小时前"),
        ),
        elapsed => (
            elapsed / 86400,
            crate::key!("1 天前"),
            crate::key!("{} 天前"),
        ),
    };
    if count == 1 {
        i18n::tr_in(lang, one).into()
    } else {
        i18n::trf_in(lang, many, &[&count])
    }
}

#[derive(Default)]
struct Fields {
    tag: Option<String>,
    name: Option<String>,
    published: Option<String>,
    draft: bool,
    prerelease: bool,
}

fn fields(json: &[u8]) -> Option<Fields> {
    let json = json.strip_prefix(b"\xef\xbb\xbf").unwrap_or(json);
    let mut s = Scanner { bytes: json, at: 0 };
    let mut fields = Fields::default();
    if !s.eat(b'{') {
        return None;
    }
    if s.eat(b'}') {
        return Some(fields);
    }
    loop {
        let key = s.string()?;
        if !s.eat(b':') {
            return None;
        }
        let next = s.peek()?;
        match key.as_str() {
            "tag_name" if next == b'"' => fields.tag = Some(s.string()?),
            "name" if next == b'"' => fields.name = Some(s.string()?),
            "published_at" if next == b'"' => fields.published = Some(s.string()?),
            "draft" | "prerelease" if matches!(next, b't' | b'f') => {
                let flag = s.scalar()? == "true";
                if key == "draft" {
                    fields.draft = flag;
                } else {
                    fields.prerelease = flag;
                }
            }
            _ => s.skip()?,
        }
        if s.eat(b',') {
            continue;
        }
        return s.eat(b'}').then_some(fields);
    }
}

/// Minimal JSON reader. Every step advances or fails; nesting is counted, not recursed.
struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Scanner<'_> {
    fn peek(&mut self) -> Option<u8> {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
        self.bytes.get(self.at).copied()
    }
    fn eat(&mut self, byte: u8) -> bool {
        let hit = self.peek() == Some(byte);
        if hit {
            self.at += 1;
        }
        hit
    }
    fn hex(&mut self) -> Option<u32> {
        let digits = self.bytes.get(self.at..self.at + 4)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        self.at += 4;
        u32::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
    }
    fn string(&mut self) -> Option<String> {
        if !self.eat(b'"') {
            return None;
        }
        let mut out = Vec::new();
        loop {
            let byte = *self.bytes.get(self.at)?;
            self.at += 1;
            match byte {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let escape = *self.bytes.get(self.at)?;
                    self.at += 1;
                    let c = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let high = self.hex()?;
                            let code = match high {
                                0xd800..=0xdbff => {
                                    if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                                        return None;
                                    }
                                    self.at += 2;
                                    let low = self.hex()?;
                                    if !(0xdc00..=0xdfff).contains(&low) {
                                        return None;
                                    }
                                    0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                                }
                                0xdc00..=0xdfff => return None,
                                code => code,
                            };
                            char::from_u32(code)?
                        }
                        _ => return None,
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
                0x00..=0x1f => return None,
                byte => out.push(byte),
            }
        }
    }
    /// A number, `true`, `false` or `null`.
    fn scalar(&mut self) -> Option<&str> {
        self.peek()?;
        let start = self.at;
        while let Some(b'-' | b'+' | b'.' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z') =
            self.bytes.get(self.at)
        {
            self.at += 1;
        }
        if self.at == start {
            return None;
        }
        std::str::from_utf8(&self.bytes[start..self.at]).ok()
    }
    fn skip(&mut self) -> Option<()> {
        match self.peek()? {
            b'"' => self.string().map(drop),
            b'{' | b'[' => {
                let mut depth = 0usize;
                loop {
                    match self.peek()? {
                        b'"' => {
                            self.string()?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth = depth.checked_sub(1)?;
                            if depth == 0 {
                                self.at += 1;
                                return Some(());
                            }
                        }
                        _ => {}
                    }
                    self.at += 1;
                }
            }
            _ => self.scalar().map(drop),
        }
    }
}
