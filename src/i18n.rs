//! Interface languages. Simplified Chinese source text is the lookup key; the
//! table in `strings.rs` supplies the other columns. A missing key falls back
//! to the source text, so nothing is ever blank.
use std::{
    collections::HashMap,
    fmt::{Display, Write},
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

mod strings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    #[default]
    ZhCn,
    ZhTw,
    En,
    Ja,
}

impl Lang {
    /// Display and table-column order.
    pub const ALL: [Self; 4] = [Self::ZhCn, Self::ZhTw, Self::En, Self::Ja];

    pub fn index(self) -> usize {
        self as usize
    }
    /// Persisted INI value.
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }
    /// Shown untranslated, so every reader can find their own language.
    pub fn native(self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::ZhTw => "繁體中文",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "zh-cn" | "zh-hans" | "zh" => Some(Self::ZhCn),
            "zh-tw" | "zh-hant" | "zh-hk" => Some(Self::ZhTw),
            "en" | "en-us" | "en-gb" => Some(Self::En),
            "ja" | "ja-jp" => Some(Self::Ja),
            _ => None,
        }
    }
    /// Windows LANGID: Taiwan, Hong Kong and Macao use Traditional Chinese, other
    /// Chinese locales Simplified; languages without a translation get English.
    pub fn from_langid(id: u16) -> Self {
        match (id & 0x3ff, id >> 10) {
            (0x04, 0x01 | 0x03 | 0x05 | 0x1f) => Self::ZhTw,
            (0x04, _) => Self::ZhCn,
            (0x11, _) => Self::Ja,
            _ => Self::En,
        }
    }
    /// The Windows display language of the current user.
    pub fn system() -> Self {
        #[cfg(windows)]
        {
            Self::from_langid(unsafe {
                windows_sys::Win32::Globalization::GetUserDefaultUILanguage()
            })
        }
        #[cfg(not(windows))]
        {
            Self::En
        }
    }
    /// `None` in settings follows the system.
    pub fn resolve(choice: Option<Self>) -> Self {
        choice.unwrap_or_else(Self::system)
    }
    /// Interface face whose CJK glyphs match the language's regional forms.
    pub fn ui_font(self) -> &'static str {
        match self {
            Self::ZhCn | Self::En => "Microsoft YaHei UI",
            Self::ZhTw => "Microsoft JhengHei UI",
            Self::Ja => "Yu Gothic UI",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::ALL[usize::from(CURRENT.load(Ordering::Relaxed)).min(Lang::ALL.len() - 1)]
}

fn lookup() -> &'static HashMap<&'static str, &'static [&'static str; 4]> {
    static MAP: OnceLock<HashMap<&'static str, &'static [&'static str; 4]>> = OnceLock::new();
    MAP.get_or_init(|| strings::TABLE.iter().map(|row| (row[0], row)).collect())
}

/// Translate source text into the current language.
pub fn tr(key: &'static str) -> &'static str {
    tr_in(current(), key)
}

pub fn tr_in(lang: Lang, key: &'static str) -> &'static str {
    match lookup().get(key) {
        Some(row) if !row[lang.index()].is_empty() => row[lang.index()],
        _ => key,
    }
}

/// Translate a template and fill each `{}` in order.
pub fn trf(template: &'static str, args: &[&dyn Display]) -> String {
    trf_in(current(), template, args)
}

pub fn trf_in(lang: Lang, template: &'static str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut parts = tr_in(lang, template).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        if let Some(arg) = args.next() {
            let _ = write!(out, "{arg}");
        }
        out.push_str(part);
    }
    out
}

/// Every translated row as [zh-CN, zh-TW, en, ja], for integrity tests.
pub fn table() -> &'static [[&'static str; 4]] {
    strings::TABLE
}

/// Marks source text kept in a constant table; translate it with `tr` at display time.
#[macro_export]
macro_rules! key {
    ($text:literal) => {
        $text
    };
}
