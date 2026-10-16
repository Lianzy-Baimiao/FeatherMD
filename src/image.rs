//! Which images the preview may show, and how large. Pure rules, no I/O: only
//! local files under a drive letter with a known raster type qualify. Network
//! addresses never resolve to local paths. HTTP(S) images have a separate,
//! explicit per-image permission path; turning images on never fetches them.
use std::path::{Path, PathBuf};

/// Largest image file read, decoded image and side length accepted.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 32_000_000;
pub const MAX_SIDE: u32 = 16_384;
/// Raster types the Windows GDI+ decoders handle.
const TYPES: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "gif", "bmp", "dib", "tif", "tiff", "ico",
];

/// Whether `bytes` start like a raster type FeatherMD decodes. Decoders pick the
/// format from the content, not the name, so anything else (metafiles, SVG,
/// HTML…) is refused before it reaches one.
pub fn raster(bytes: &[u8]) -> bool {
    const SIGNATURES: &[&[u8]] = &[
        b"\x89PNG\r\n\x1a\n",
        b"\xff\xd8\xff",
        b"GIF87a",
        b"GIF89a",
        b"BM",
        b"II*\0",
        b"MM\0*",
        b"\0\0\x01\0",
    ];
    SIGNATURES.iter().any(|s| bytes.starts_with(s))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A local file that may be read.
    Local(PathBuf),
    /// A network address, never automatically fetched.
    Remote,
    /// A share, device, data URI or otherwise unsafe reference: never opened.
    Refused,
    /// A local file of a type FeatherMD does not decode (SVG, WebP…).
    Unsupported,
}

/// A validated HTTP(S) image address. No credentials, implicit scheme, backslash
/// or control characters; the query is preserved and the fragment is not sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebImage {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub secure: bool,
    pub resource: String,
}
impl WebImage {
    pub fn parse(src: &str) -> Option<Self> {
        let src = src.trim();
        if src.len() > 2048
            || src.chars().any(|c| c.is_control() || c.is_whitespace())
            || src.contains(['\\', '"', '<', '>'])
        {
            return None;
        }
        let (scheme, rest) = src.split_once("://")?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            _ => return None,
        };
        let rest = rest.split('#').next()?;
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, path) = rest.split_at(end);
        let (host, port) = if let Some(ipv6) = authority.strip_prefix('[') {
            let (host, port) = ipv6.split_once(']')?;
            host.parse::<std::net::Ipv6Addr>().ok()?;
            (
                host,
                port.strip_prefix(':')
                    .or_else(|| port.is_empty().then_some(""))?,
            )
        } else {
            let (host, port) = authority.split_once(':').unwrap_or((authority, ""));
            if host.is_empty()
                || !host.is_ascii()
                || !host.split('.').all(|label| {
                    !label.is_empty()
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                })
            {
                return None;
            }
            (host, port)
        };
        if authority.ends_with(':') {
            return None;
        }
        let port = if port.is_empty() {
            if secure { 443 } else { 80 }
        } else {
            port.parse::<u16>().ok()?
        };
        if port == 0 {
            return None;
        }
        let mut resource = if path.starts_with('/') {
            String::new()
        } else {
            "/".into()
        };
        for byte in path.bytes() {
            if byte.is_ascii() {
                resource.push(byte as char);
            } else {
                use std::fmt::Write;
                write!(resource, "%{byte:02X}").ok()?;
            }
        }
        // Invalid escapes can be interpreted differently by network stacks.
        for (i, b) in resource.bytes().enumerate() {
            if b == b'%'
                && !resource
                    .as_bytes()
                    .get(i + 1..i + 3)
                    .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            {
                return None;
            }
        }
        Some(Self {
            url: format!(
                "{}://{}{}",
                if secure { "https" } else { "http" },
                authority.to_ascii_lowercase(),
                resource
            ),
            host: host.to_ascii_lowercase(),
            port,
            secure,
            resource,
        })
    }
}

/// `%xx` escapes decoded; None when the result is not clean UTF-8 text.
fn percent_decode(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out)
        .ok()
        .filter(|s| !s.chars().any(char::is_control))
}

/// `name:` where `name` is a URI scheme of two or more letters (one letter is a drive).
fn scheme(text: &str) -> Option<&str> {
    let (name, _) = text.split_once(':')?;
    let mut chars = name.chars();
    (name.len() >= 2
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
    .then_some(name)
}

fn drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Where `src` (an image reference of a document in `folder`) points.
pub fn resolve(src: &str, folder: &Path) -> Source {
    let src = src.trim();
    // `\\?\` and `\\.\` (either slash) are device paths, never files to read. Checked
    // before the query split below, which would cut them down to `//`.
    if matches!(
        src.as_bytes(),
        [b'/' | b'\\', b'/' | b'\\', b'?' | b'.', b'/' | b'\\', ..]
    ) {
        return Source::Refused;
    }
    // A file name ends before a query or fragment.
    let src = src.split(['?', '#']).next().unwrap_or("");
    if src.is_empty() || src.len() > 2048 {
        return Source::Refused;
    }
    let path = match scheme(src).map(str::to_ascii_lowercase).as_deref() {
        Some("http" | "https" | "ftp" | "ftps" | "ws" | "wss") => return Source::Remote,
        Some("file") => {
            let rest = &src[5..];
            let rest = rest.strip_prefix("//").map_or(Some(rest), |after| {
                // file://host/... names another machine unless the host is empty
                // or localhost.
                let (host, path) = after.split_at(after.find('/').unwrap_or(after.len()));
                (host.is_empty() || host.eq_ignore_ascii_case("localhost")).then_some(path)
            });
            let Some(rest) = rest else {
                return Source::Remote;
            };
            let rest = rest.trim_start_matches('/');
            if !drive(rest) {
                return Source::Refused;
            }
            rest.to_owned()
        }
        Some(_) => return Source::Refused,
        // `//host/path` is a network address too.
        None if src.starts_with("//") => return Source::Remote,
        None => src.to_owned(),
    };
    let Some(path) = percent_decode(&path) else {
        return Source::Refused;
    };
    let path = path.replace('/', "\\");
    if path.starts_with("\\\\") {
        // UNC shares and \\?\ \\.\ device paths.
        return Source::Refused;
    }
    let joined = if drive(&path) {
        if !path[2..].starts_with('\\') {
            // `C:name` is relative to a drive's current directory.
            return Source::Refused;
        }
        path
    } else {
        let base = folder.to_string_lossy();
        if !drive(&base) || base[2..].starts_with("\\\\") {
            return Source::Refused;
        }
        // `/images/a.png` is relative to the document's folder, as in a repository.
        format!(
            "{}\\{}",
            base.trim_end_matches('\\'),
            path.trim_start_matches('\\')
        )
    };
    let Some(normal) = normalize(&joined) else {
        return Source::Refused;
    };
    let extension = normal
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| !e.contains('\\'));
    if !extension.is_some_and(|e| TYPES.contains(&e.as_str())) {
        return Source::Unsupported;
    }
    Source::Local(PathBuf::from(normal))
}

/// `C:\a\.\b\..\c.png` as `C:\a\c.png`; None for names Windows treats specially:
/// device names, streams, wildcards, trailing dots or spaces.
fn normalize(path: &str) -> Option<String> {
    let (letter, rest) = path.split_at(2);
    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => {
                if !valid_name(name) {
                    return None;
                }
                parts.push(name);
            }
        }
    }
    let out = format!("{}\\{}", letter.to_ascii_uppercase(), parts.join("\\"));
    (out.len() <= 1024 && !parts.is_empty()).then_some(out)
}

fn valid_name(name: &str) -> bool {
    if name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
    {
        return false;
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit());
    !reserved
}

/// Device-pixel size of an image whose decoded size is `natural`: size
/// attributes are CSS pixels (`scale` = DPI / 96), one missing attribute keeps
/// the aspect ratio, and the result fits `max` without ever being enlarged by
/// the fit.
pub fn display_size(
    natural: (u32, u32),
    attributes: (Option<u32>, Option<u32>),
    scale: f32,
    max: (i32, i32),
) -> (i32, i32) {
    let (nw, nh) = (natural.0.max(1) as f32, natural.1.max(1) as f32);
    let (w, h) = match attributes {
        (Some(w), Some(h)) => (w as f32, h as f32),
        (Some(w), None) => (w as f32, w as f32 * nh / nw),
        (None, Some(h)) => (h as f32 * nw / nh, h as f32),
        (None, None) => (nw, nh),
    };
    let (mut w, mut h) = (w * scale, h * scale);
    let (mw, mh) = (max.0.max(1) as f32, max.1.max(1) as f32);
    let fit = (mw / w).min(mh / h).min(1.0);
    w *= fit;
    h *= fit;
    ((w.round() as i32).max(1), (h.round() as i32).max(1))
}

/// Text shown in place of an image: its alternative text, else its file name.
pub fn label(alt: &str, src: &str) -> String {
    let alt = alt.trim();
    if !alt.is_empty() {
        return alt.to_owned();
    }
    let name = src
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    percent_decode(name).unwrap_or_else(|| name.to_owned())
}
