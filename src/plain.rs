//! Plain text and logs: which format a file is read as, chapter lines of plain text,
//! and the timestamp and level at the start of a log line. Pure functions over short
//! line prefixes; no regular expressions, no whole-file work.
use crate::{
    document::SourceLine,
    highlight::Token,
    markdown::{self, Block, Kind, Span},
};
use std::borrow::Cow;

/// How a document is shown and what its outline lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Markdown: headings.
    #[default]
    Markdown,
    /// Plain text: chapters.
    Text,
    /// Logs: a time index, or the error lines of a log without timestamps.
    Log,
}

/// The format from the file name: Markdown extensions (and README-like names without
/// one), logs (`.log`, rotated `app.log.1`, `.out`, `.err`), plain text otherwise.
pub fn detect(path: &std::path::Path) -> Format {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = name.rsplit_once('.').map(|(_, e)| e);
    match ext {
        Some("md" | "markdown" | "mdown" | "mkd" | "mkdn" | "mdwn") => Format::Markdown,
        Some("log" | "out" | "err" | "trace") => Format::Log,
        _ if name.contains(".log.") || name.contains(".log-") || name.contains(".log_") => {
            Format::Log
        }
        None if matches!(
            name.as_str(),
            "readme" | "changelog" | "changes" | "contributing" | "authors" | "notes"
        ) =>
        {
            Format::Markdown
        }
        _ => Format::Text,
    }
}

/// Plain text whose first lines mostly start like log lines (a timestamp, or a level
/// such as `ERROR` or `[warn]`) is read as a log. Markdown and logs stay as they are.
pub fn sniff(format: Format, lines: &[SourceLine]) -> Format {
    if format != Format::Text {
        return format;
    }
    // Lines that open an entry; indented lines continue one (stack traces).
    let candidates: Vec<&str> = lines
        .iter()
        .filter(|l| !l.text.trim().is_empty() && !l.text.starts_with([' ', '\t']))
        .take(40)
        .map(|l| l.text.as_str())
        .collect();
    // A level counts when it opens the line (`ERROR:`, `[warn]`, `level=error`), not
    // when prose mentions one.
    let logged = candidates
        .iter()
        .filter(|t| {
            let t = strip_ansi(t);
            stamp(&t).is_some()
                || level(&t, 0)
                    .is_some_and(|l| l.strong && l.range.0 <= 16 && t[..l.range.0].is_ascii())
        })
        .count();
    if candidates.len() >= 3 && logged * 2 > candidates.len() {
        Format::Log
    } else {
        Format::Text
    }
}
/// A log timestamp. Syslog has no year, glog and logcat no year either, and some logs
/// only the time of day; those parts are None.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamp {
    pub year: Option<u16>,
    pub date: Option<(u8, u8)>,
    pub time: (u8, u8, u8),
}
impl Stamp {
    /// Days since 1970-01-01 (year 2000 when the log does not say), or None.
    pub fn day(&self) -> Option<i64> {
        let (month, day) = self.date?;
        Some(days_from_civil(
            i64::from(self.year.unwrap_or(2000)),
            i64::from(month),
            i64::from(day),
        ))
    }
    /// Seconds on the log's own clock, for ordering and bucketing.
    pub fn seconds(&self) -> i64 {
        let (h, m, s) = self.time;
        self.day().unwrap_or(0) * 86_400 + i64::from(h) * 3600 + i64::from(m) * 60 + i64::from(s)
    }
}
/// Days from 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Where a log line's timestamp is: the stamp, its bytes, and a one-letter level
/// that belongs to the format (glog `E1024 …`, logcat `… 1234  5678 E Tag`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub stamp: Stamp,
    pub range: (usize, usize),
    pub letter: Option<(Level, (usize, usize))>,
}
/// A byte cursor over a line prefix.
struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}
impl Cursor<'_> {
    fn digits(&mut self, n: usize) -> Option<u32> {
        let s = self.b.get(self.i..self.i + n)?;
        if !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        self.i += n;
        Some(s.iter().fold(0, |v, d| v * 10 + u32::from(d - b'0')))
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        let hit = self.peek() == Some(c);
        self.i += usize::from(hit);
        hit
    }
    fn want(&mut self, c: u8) -> Option<()> {
        self.eat(c).then_some(())
    }
    /// HH:MM:SS, then an optional fraction (`.123` or `,123`) and zone (`Z`, `+08:00`).
    fn time(&mut self) -> Option<(u8, u8, u8)> {
        let h = self.digits(2)?;
        self.want(b':')?;
        let m = self.digits(2)?;
        self.want(b':')?;
        let s = self.digits(2)?;
        if h > 23 || m > 59 || s > 60 {
            return None;
        }
        if matches!(self.peek(), Some(b'.' | b','))
            && self.b.get(self.i + 1).is_some_and(u8::is_ascii_digit)
        {
            self.i += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if !self.eat(b'Z') && matches!(self.peek(), Some(b'+' | b'-')) {
            let save = self.i;
            self.i += 1;
            let zone = self.digits(2).is_some() && {
                self.eat(b':');
                self.digits(2).is_some()
            };
            if !zone {
                self.i = save;
            }
        }
        Some((h as u8, m as u8, s as u8))
    }
    /// YYYY, MM and DD separated by `sep`.
    fn ymd(&mut self, sep: u8) -> Option<(u16, u8, u8)> {
        let y = self.digits(4)?;
        self.want(sep)?;
        let m = self.digits(2)?;
        self.want(sep)?;
        let d = self.digits(2)?;
        valid(m, d).then_some((y as u16, m as u8, d as u8))
    }
}
fn valid(month: u32, day: u32) -> bool {
    (1..=12).contains(&month) && (1..=31).contains(&day)
}
fn month(name: &[u8]) -> Option<u32> {
    const NAMES: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    NAMES.iter().position(|n| *n == name).map(|i| i as u32 + 1)
}
/// A timestamp shape tried at one position.
type Pattern = fn(&mut Cursor) -> Option<Stamp>;
/// The timestamp of a log line: at its start (after `[` or spaces) as ISO
/// `2026-10-24 08:15:30,123` / `2026-10-24T08:15:30Z`, `2026/10/24 08:15:30`,
/// log4j `24/10/24 08:15:30`, logcat `10-24 08:15:30.123`, syslog `Oct 24 08:15:30`,
/// glog `I1024 08:15:30.123456` or a bare `08:15:30`; otherwise an ISO date-time or
/// an access-log `[24/Oct/2026:08:15:30 +0000]` within the first 160 bytes.
pub fn stamp(text: &str) -> Option<Parsed> {
    let b = &text.as_bytes()[..text.len().min(160)];
    let start = b
        .iter()
        .take(4)
        .take_while(|&&c| c == b'[' || c == b' ')
        .count();
    let at = |i: usize| Cursor { b, i };
    // Patterns at the start; logcat carries its level letter after the PID and TID.
    let leading: [(Pattern, bool); 6] = [
        (iso_dash, false),
        (iso_slash, false),
        (log4j, false),
        (logcat, true),
        (syslog, false),
        (time_only, false),
    ];
    for (pattern, lettered) in leading {
        let mut c = at(start);
        if let Some(stamp) = pattern(&mut c) {
            let letter = if lettered {
                logcat_letter(b, c.i)
            } else {
                None
            };
            return Some(Parsed {
                stamp,
                range: (start, c.i),
                letter,
            });
        }
    }
    // glog: a level letter, then MMDD and the time.
    if let Some(level) = b.first().and_then(|&c| letter(c)) {
        let mut c = at(1);
        if let Some(m) = c.digits(2)
            && let Some(d) = c.digits(2)
            && c.eat(b' ')
            && let Some(time) = c.time()
            && valid(m, d)
        {
            let stamp = Stamp {
                year: None,
                date: Some((m as u8, d as u8)),
                time,
            };
            return Some(Parsed {
                stamp,
                range: (1, c.i),
                letter: Some((level, (0, 1))),
            });
        }
    }
    // An ISO date-time later in the line (logfmt `time=…`, JSON), not inside a word.
    for i in 1..b.len().saturating_sub(18) {
        if b[i].is_ascii_digit() && !b[i - 1].is_ascii_alphanumeric() {
            let mut c = at(i);
            if let Some(stamp) = iso_dash(&mut c) {
                return Some(Parsed {
                    stamp,
                    range: (i, c.i),
                    letter: None,
                });
            }
        }
    }
    // Access logs: `[24/Oct/2026:08:15:30 +0000]`.
    let brackets = b.iter().enumerate().filter(|&(_, &c)| c == b'[');
    for (i, _) in brackets.take_while(|&(i, _)| i + 20 < b.len()) {
        let mut c = at(i + 1);
        if let Some(stamp) = access(&mut c) {
            return Some(Parsed {
                stamp,
                range: (i + 1, c.i),
                letter: None,
            });
        }
    }
    None
}
/// `2026-10-24 08:15:30` or `2026-10-24T08:15:30`.
fn iso_dash(c: &mut Cursor) -> Option<Stamp> {
    let (year, month, day) = c.ymd(b'-')?;
    if !(c.eat(b'T') || c.eat(b' ')) {
        return None;
    }
    let time = c.time()?;
    Some(Stamp {
        year: Some(year),
        date: Some((month, day)),
        time,
    })
}
/// `2026/10/24 08:15:30`.
fn iso_slash(c: &mut Cursor) -> Option<Stamp> {
    let (year, month, day) = c.ymd(b'/')?;
    c.want(b' ')?;
    let time = c.time()?;
    Some(Stamp {
        year: Some(year),
        date: Some((month, day)),
        time,
    })
}
/// log4j's `yy/MM/dd HH:mm:ss` (Spark, Hadoop): `24/10/24 08:15:30`.
fn log4j(c: &mut Cursor) -> Option<Stamp> {
    let year = c.digits(2)?;
    c.want(b'/')?;
    let month = c.digits(2)?;
    c.want(b'/')?;
    let day = c.digits(2)?;
    c.want(b' ')?;
    let time = c.time()?;
    valid(month, day).then_some(Stamp {
        year: Some(2000 + year as u16),
        date: Some((month as u8, day as u8)),
        time,
    })
}
/// Logcat `10-24 08:15:30.123`.
fn logcat(c: &mut Cursor) -> Option<Stamp> {
    let month = c.digits(2)?;
    c.want(b'-')?;
    let day = c.digits(2)?;
    c.want(b' ')?;
    let time = c.time()?;
    valid(month, day).then_some(Stamp {
        year: None,
        date: Some((month as u8, day as u8)),
        time,
    })
}
/// Syslog `Oct 24 08:15:30`, `Oct  4 08:15:30`.
fn syslog(c: &mut Cursor) -> Option<Stamp> {
    let month = month(c.b.get(c.i..c.i + 3)?)?;
    c.i += 3;
    c.want(b' ')?;
    c.eat(b' ');
    let day = c.digits(2).or_else(|| c.digits(1))?;
    c.want(b' ')?;
    let time = c.time()?;
    valid(month, day).then_some(Stamp {
        year: None,
        date: Some((month as u8, day as u8)),
        time,
    })
}
/// A bare time of day, `08:15:30.123`.
fn time_only(c: &mut Cursor) -> Option<Stamp> {
    let time = c.time()?;
    Some(Stamp {
        year: None,
        date: None,
        time,
    })
}
/// Access logs `24/Oct/2026:08:15:30` (inside `[…]`).
fn access(c: &mut Cursor) -> Option<Stamp> {
    let day = c.digits(2)?;
    c.want(b'/')?;
    let month = month(c.b.get(c.i..c.i + 3)?)?;
    c.i += 3;
    c.want(b'/')?;
    let year = c.digits(4)?;
    c.want(b':')?;
    let time = c.time()?;
    valid(month, day).then_some(Stamp {
        year: Some(year as u16),
        date: Some((month as u8, day as u8)),
        time,
    })
}
/// Logcat `… 08:15:30.123  1234  5678 E Tag: …`: the level letter after PID and TID.
fn logcat_letter(b: &[u8], from: usize) -> Option<(Level, (usize, usize))> {
    let mut c = Cursor { b, i: from };
    for _ in 0..2 {
        c.want(b' ')?;
        while c.eat(b' ') {}
        let digits = b[c.i..].iter().take_while(|d| d.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        c.i += digits;
    }
    c.want(b' ')?;
    while c.eat(b' ') {}
    let at = c.i;
    let level = letter(*b.get(at)?)?;
    (b.get(at + 1) == Some(&b' ')).then_some((level, (at, at + 1)))
}
/// One-letter levels of glog and logcat.
fn letter(c: u8) -> Option<Level> {
    Some(match c {
        b'V' => Level::Trace,
        b'D' => Level::Debug,
        b'I' => Level::Info,
        b'W' => Level::Warn,
        b'E' => Level::Error,
        b'F' | b'A' => Level::Fatal,
        _ => return None,
    })
}
/// Log severity, lowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}
/// The level word of a log line and its bytes. `strong` when it cannot be prose: an
/// uppercase word, `[warn]` / `<error>`, a key (`level=error`, `"level":"warn"`), or a
/// word followed by `:` (`Error:`, compiler-style `file:3:5: error[E0308]:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    pub level: Level,
    pub range: (usize, usize),
    pub strong: bool,
}
/// The first level word at or after byte `from`, within the first 160 bytes. Lower-
/// and title-case words count only as the first word or in one of the strong forms.
pub fn level(text: &str, from: usize) -> Option<Found> {
    let b = &text.as_bytes()[..text.len().min(160)];
    let mut i = from.min(b.len());
    let mut first = true;
    while i < b.len() {
        if !b[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let word = &b[start..i];
        let was_first = std::mem::replace(&mut first, false);
        let Some(level) = level_name(word) else {
            continue;
        };
        let upper = word.iter().all(u8::is_ascii_uppercase);
        let lower = word.iter().all(u8::is_ascii_lowercase);
        let title = word[0].is_ascii_uppercase() && word[1..].iter().all(u8::is_ascii_lowercase);
        if !(upper || lower || title) {
            continue;
        }
        let before = start.checked_sub(1).map(|j| b[j]);
        let after = b.get(i).copied();
        let bracketed = matches!(
            (before, after),
            (Some(b'['), Some(b']')) | (Some(b'<'), Some(b'>'))
        );
        let keyed = keyed(&b[..start]);
        let colon = after == Some(b':') || (after == Some(b'[') && level >= Level::Warn);
        let strong = upper || bracketed || keyed || colon;
        if strong || was_first {
            return Some(Found {
                level,
                range: (start, i),
                strong,
            });
        }
    }
    None
}
/// A `level` key just before a value: `level=`, `lvl=`, `severity=`, `"level":"`.
fn keyed(before: &[u8]) -> bool {
    let trimmed = before.strip_suffix(b"\"").unwrap_or(before);
    let Some(key) = trimmed
        .strip_suffix(b"=")
        .or_else(|| trimmed.strip_suffix(b":"))
        .or_else(|| trimmed.strip_suffix(b": "))
    else {
        return false;
    };
    let key = key.strip_suffix(b"\"").unwrap_or(key);
    [&b"level"[..], b"lvl", b"severity", b"loglevel"]
        .iter()
        .any(|k| key.len() >= k.len() && key[key.len() - k.len()..].eq_ignore_ascii_case(k))
}
fn level_name(word: &[u8]) -> Option<Level> {
    const NAMES: [(&[u8], Level); 17] = [
        (b"fatal", Level::Fatal),
        (b"critical", Level::Fatal),
        (b"crit", Level::Fatal),
        (b"panic", Level::Fatal),
        (b"emerg", Level::Fatal),
        (b"alert", Level::Fatal),
        (b"error", Level::Error),
        (b"err", Level::Error),
        (b"severe", Level::Error),
        (b"warning", Level::Warn),
        (b"warn", Level::Warn),
        (b"info", Level::Info),
        (b"notice", Level::Info),
        (b"debug", Level::Debug),
        (b"dbg", Level::Debug),
        (b"trace", Level::Trace),
        (b"verbose", Level::Trace),
    ];
    if !(3..=8).contains(&word.len()) {
        return None;
    }
    NAMES
        .iter()
        .find(|(name, _)| word.eq_ignore_ascii_case(name))
        .map(|&(_, level)| level)
}
/// A line worth listing in a log without timestamps: an error or fatal level, or the
/// start of a stack trace or panic.
pub fn problem(text: &str) -> bool {
    let text = strip_ansi(text);
    if level(&text, 0).is_some_and(|f| f.strong && f.level >= Level::Error) {
        return true;
    }
    let t = text.trim_start();
    [
        "Traceback (most recent call last)",
        "Exception in thread ",
        "Caused by: ",
    ]
    .iter()
    .any(|m| t.starts_with(m))
        || head(&text, 160).contains("panicked at")
}
/// At most `n` bytes of `text`, cut at a character boundary.
fn head(text: &str, n: usize) -> &str {
    let mut end = text.len().min(n);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
/// `text` without ANSI escape sequences (colours in CI and terminal logs).
pub fn strip_ansi(text: &str) -> Cow<'_, str> {
    if !text.contains('\u{1b}') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters and intermediates up to a final byte @ to ~.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC (hyperlinks, titles) up to BEL or ESC \.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next().is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    Cow::Owned(out)
}
/// Chinese numerals, full-width and ASCII digits. (Chinese pattern text is written as
/// chars: it is matched in documents, never shown, so it is not interface text.)
fn numeral(c: char) -> bool {
    c.is_ascii_digit()
        || ('０'..='９').contains(&c)
        || small_numeral(c)
        || matches!(
            c,
            '千' | '万'
                | '萬'
                | '两'
                | '兩'
                | '壹'
                | '贰'
                | '貳'
                | '叁'
                | '參'
                | '肆'
                | '伍'
                | '陆'
                | '陸'
                | '柒'
                | '捌'
                | '玖'
                | '拾'
                | '佰'
                | '仟'
        )
}
/// The numerals of document numbering (`一、`, `（十二）`).
fn small_numeral(c: char) -> bool {
    matches!(
        c,
        '零' | '〇' | '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十' | '百'
    )
}
/// Byte length of `word` at the start of `text`.
fn prefix(text: &str, word: &[char]) -> Option<usize> {
    let mut chars = text.char_indices();
    for &c in word {
        if chars.next()?.1 != c {
            return None;
        }
    }
    Some(chars.next().map_or(text.len(), |(i, _)| i))
}
const SENTENCE: [char; 9] = ['。', '，', '！', '？', '；', '…', ',', ';', '!'];
/// What may follow a chapter mark: nothing, a separator and a title, or a short title.
fn titled(rest: &str) -> bool {
    rest.is_empty()
        || rest.starts_with([
            ' ', '\u{3000}', '：', ':', '、', '.', '．', '·', '-', '—', '（', '(', '【', '[', '《',
            '「',
        ])
        || (rest.chars().count() <= 20 && !rest.contains(SENTENCE))
}
/// The outline level of a plain-text chapter line: 1 for volumes and parts (`第三卷`,
/// `Part One`), 2 for chapters (`第十二章`, `第3話`, `Chapter 12`, `序章`, `Prologue`),
/// 3 for sections (`第二节`, `Section 3`). Long lines are prose, never chapters.
pub fn chapter(line: &str) -> Option<u8> {
    let t = line.trim_matches([' ', '\t', '\u{3000}']);
    if t.is_empty() || t.chars().count() > 60 {
        return None;
    }
    if let Some(rest) = t.strip_prefix('第') {
        let digits = rest.chars().take_while(|&c| numeral(c)).count();
        if digits == 0 {
            return None;
        }
        let rest = &rest[rest
            .char_indices()
            .nth(digits)
            .map_or(rest.len(), |(i, _)| i)..];
        let mut chars = rest.chars();
        let level = match chars.next()? {
            '卷' | '巻' | '部' | '篇' | '集' => 1,
            '章' | '回' | '話' | '话' => 2,
            '节' | '節' => 3,
            _ => return None,
        };
        return titled(chars.as_str()).then_some(level);
    }
    special(t).or_else(|| english(t))
}
/// Front and back matter: `序章`, `楔子`, `尾声`, `番外三`, alone or before a separator.
fn special(t: &str) -> Option<u8> {
    // Longer words first: `番外篇` before `番外`.
    const WORDS: [&[char]; 15] = [
        &['番', '外', '篇'],
        &['完', '本', '感', '言'],
        &['序', '章'],
        &['序', '言'],
        &['序', '幕'],
        &['楔', '子'],
        &['引', '子'],
        &['前', '言'],
        &['尾', '声'],
        &['尾', '聲'],
        &['后', '记'],
        &['後', '記'],
        &['番', '外'],
        &['终', '章'],
        &['終', '章'],
    ];
    let end = WORDS.iter().find_map(|w| prefix(t, w))?;
    let rest = &t[end..];
    let ok = rest.is_empty()
        || rest.starts_with([
            ' ', '\u{3000}', '：', ':', '、', '·', '-', '—', '（', '(', '【', '[',
        ])
        || (rest.starts_with(numeral) && rest.chars().count() <= 20 && !rest.contains(SENTENCE));
    ok.then_some(2)
}
/// `Chapter 12: The Storm`, `PART ONE`, `Book II`, `Prologue`: short lines only.
fn english(t: &str) -> Option<u8> {
    if t.len() > 60 {
        return None;
    }
    let (first, rest) = t.split_once(' ').unwrap_or((t, ""));
    let first = first.trim_end_matches([':', '.']).to_ascii_lowercase();
    let words = rest.split_whitespace().count();
    let sentence = t.ends_with(['.', ',', ';', '?', '!']);
    let level = match first.as_str() {
        "part" | "book" | "volume" => 1,
        "chapter" => 2,
        "section" => 3,
        "prologue" | "epilogue" | "preface" | "foreword" | "afterword" | "introduction"
        | "appendix" | "interlude" => return (words <= 6 && !sentence).then_some(2),
        _ => return None,
    };
    let number = rest.split_whitespace().next()?;
    let number = number
        .trim_end_matches([':', '.', ',', '-'])
        .to_ascii_lowercase();
    // `Chapter 1.` alone is a title; a longer line ending like a sentence is prose.
    (english_number(&number) && (words <= 1 || !sentence)).then_some(level)
}
fn english_number(word: &str) -> bool {
    const ROMAN: [&str; 20] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii", "xiv",
        "xv", "xvi", "xvii", "xviii", "xix", "xx",
    ];
    const WORDS: [&str; 20] = [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    (word.starts_with(|c: char| c.is_ascii_digit())
        && word.chars().all(|c| c.is_ascii_digit() || c == '.'))
        || ROMAN.contains(&word)
        || WORDS.contains(&word)
}
/// Chinese document numbering: `一、总则` (1) and `（一）适用范围` (2), short titles only.
/// The outline uses these only when a file has no chapters.
pub fn numbered(line: &str) -> Option<u8> {
    let t = line.trim_matches([' ', '\t', '\u{3000}']);
    if t.chars().count() > 40 {
        return None;
    }
    let chinese = small_numeral;
    let (level, rest) = if let Some(rest) = t.strip_prefix(['（', '(']) {
        let n = rest.chars().take_while(|&c| chinese(c)).count();
        let rest = &rest[rest.char_indices().nth(n)?.0..];
        (2, rest.strip_prefix(['）', ')']).filter(|_| n > 0)?)
    } else {
        let n = t.chars().take_while(|&c| chinese(c)).count();
        let rest = &t[t.char_indices().nth(n)?.0..];
        (1, rest.strip_prefix('、').filter(|_| n > 0)?)
    };
    let rest = rest.trim_start();
    (!rest.is_empty() && !rest.contains(SENTENCE) && !rest.ends_with(['。', '.'])).then_some(level)
}
/// A setext underline under a title: `===` (1) or `---` (2), three or more.
pub fn setext(line: &str) -> Option<u8> {
    let t = line.trim_end();
    if t.len() < 3 {
        return None;
    }
    if t.bytes().all(|b| b == b'=') {
        Some(1)
    } else if t.bytes().all(|b| b == b'-') {
        Some(2)
    } else {
        None
    }
}
/// A line that an underline below it makes a title: short, not itself an underline
/// or a chapter, not ending like a sentence.
pub fn setext_title(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.chars().count() <= 60
        && setext(t).is_none()
        && chapter(t).is_none()
        && !t.ends_with(['。', '，', '.', ',', ';', '；', ':', '：'])
}
/// Preview blocks of a page of plain text or a log, one per line. Text keeps every
/// line as written and shows chapter and underlined title lines as headings; a log
/// line is monospace with its timestamp muted and its level coloured. Markdown and
/// source mode go through `markdown::blocks`.
pub fn blocks(lines: &[SourceLine], format: Format, raw: bool) -> Vec<Block> {
    if raw || format == Format::Markdown {
        return markdown::blocks(lines, raw);
    }
    let mut out = Vec::with_capacity(lines.len());
    // The previous segment did not end its line (a long line split on the page).
    let mut continued = false;
    // The previous line became a title through an underline below it.
    let mut titled = false;
    for (i, line) in lines.iter().enumerate() {
        let text = line.text.as_str();
        let tail = std::mem::replace(&mut continued, line.continued);
        let was_titled = std::mem::replace(&mut titled, false);
        let block = if format == Format::Log {
            let spans = if tail {
                plain_spans(text)
            } else {
                log_spans(text)
            };
            markdown::block(line, Kind::Log, spans, 0)
        } else if tail {
            markdown::block(line, Kind::Text, plain_spans(text), 0)
        } else if let Some(level) = chapter(text) {
            heading(line, level + 1)
        } else if was_titled && setext(text).is_some() {
            markdown::block(line, Kind::Rule, Vec::new(), 0)
        } else if let Some(level) = lines
            .get(i + 1)
            .filter(|next| !line.continued && setext_title(text) && !next.continued)
            .and_then(|next| setext(&next.text))
        {
            titled = true;
            heading(line, level + 2)
        } else if text.trim().is_empty() {
            markdown::block(line, Kind::Text, Vec::new(), 0)
        } else {
            markdown::block(line, Kind::Text, plain_spans(text), 0)
        };
        out.push(block);
    }
    out
}
/// A chapter or title line shown as a heading of `level`, without its indent.
fn heading(line: &SourceLine, level: u8) -> Block {
    let title = line.text.trim_matches([' ', '\t', '\u{3000}']);
    markdown::block(line, Kind::Heading(level.min(6)), plain_spans(title), 0)
}
fn plain_spans(text: &str) -> Vec<Span> {
    vec![Span {
        text: text.to_owned(),
        ..Span::default()
    }]
}
/// The timestamp and level of a log line, without its ANSI escapes. A level word is
/// looked for after the timestamp, or anywhere when the timestamp is inside the line.
pub fn log_level(text: &str, parsed: Option<&Parsed>) -> Option<(Level, (usize, usize), bool)> {
    if let Some((level, range)) = parsed.and_then(|p| p.letter) {
        return Some((level, range, true));
    }
    let from = parsed.map_or(0, |p| if p.range.0 <= 4 { p.range.1 } else { 0 });
    level(text, from)
        .filter(|f| parsed.is_none_or(|p| f.range.1 <= p.range.0 || f.range.0 >= p.range.1))
        .map(|f| (f.level, f.range, f.strong))
}
/// A log line's spans: muted timestamp, coloured level (errors red, warnings amber,
/// debug and trace muted), the rest plain.
fn log_spans(text: &str) -> Vec<Span> {
    let clean = strip_ansi(text);
    let clean = clean.as_ref();
    let parsed = stamp(clean);
    let mut marks: Vec<((usize, usize), Token)> = Vec::new();
    if let Some(p) = &parsed {
        marks.push((p.range, Token::Comment));
    }
    if let Some((level, range, _)) = log_level(clean, parsed.as_ref()) {
        let token = match level {
            Level::Fatal | Level::Error => Some(Token::Deleted),
            Level::Warn => Some(Token::Number),
            Level::Debug | Level::Trace => Some(Token::Comment),
            Level::Info => None,
        };
        if let Some(token) = token {
            marks.push((range, token));
        }
    }
    marks.sort_by_key(|(range, _)| range.0);
    let mut spans = Vec::new();
    let mut at = 0;
    for ((from, to), token) in marks {
        if from < at || to > clean.len() {
            continue;
        }
        if from > at {
            spans.push(Span {
                text: clean[at..from].to_owned(),
                ..Span::default()
            });
        }
        spans.push(Span {
            text: clean[from..to].to_owned(),
            token: Some(token),
            ..Span::default()
        });
        at = to;
    }
    if at < clean.len() {
        spans.push(Span {
            text: clean[at..].to_owned(),
            ..Span::default()
        });
    }
    spans
}
