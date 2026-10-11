//! GDI text layout, limited to one bounded page; only visible rows are painted.
#![allow(unsafe_op_in_unsafe_fn)]
use feathermd::{
    markdown::{Block, Kind, Span},
    settings::Settings,
};
use std::collections::BTreeMap;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontKey {
    pub size: i32,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub strike: bool,
}
pub struct Fonts {
    cache: BTreeMap<FontKey, HFONT>,
    pub dpi: u32,
}
impl Fonts {
    pub fn new(dpi: u32) -> Self {
        Self {
            cache: BTreeMap::new(),
            dpi,
        }
    }
    pub unsafe fn get(&mut self, key: FontKey) -> HFONT {
        *self.cache.entry(key).or_insert_with(|| {
            let face: Vec<u16> = if key.mono {
                "Consolas"
            } else {
                "Microsoft YaHei UI"
            }
            .encode_utf16()
            .chain(Some(0))
            .collect();
            CreateFontW(
                -((key.size as u32 * self.dpi / 96) as i32),
                0,
                0,
                0,
                if key.bold { 700 } else { 400 },
                key.italic as u32,
                0,
                key.strike as u32,
                DEFAULT_CHARSET as u32,
                0,
                0,
                CLEARTYPE_QUALITY as u32,
                0,
                face.as_ptr(),
            )
        })
    }
    pub fn scale(&self, value: i32) -> i32 {
        (value as i64 * self.dpi as i64 / 96) as i32
    }
}
impl Drop for Fonts {
    fn drop(&mut self) {
        for font in self.cache.values() {
            unsafe {
                DeleteObject(*font);
            }
        }
    }
}
#[derive(Clone)]
pub struct Run {
    pub text: Vec<u16>,
    pub x: i32,
    pub width: i32,
    pub font: FontKey,
    pub link: Option<String>,
    pub code: bool,
}
#[derive(Clone)]
pub struct Row {
    pub start: u64,
    pub end: u64,
    pub runs: Vec<Run>,
    pub height: i32,
    pub kind: Kind,
}
pub struct Layout {
    pub rows: Vec<Row>,
    pub max_width: i32,
}
impl Layout {
    pub unsafe fn build(
        dc: HDC,
        blocks: &[Block],
        settings: &Settings,
        fonts: &mut Fonts,
        width: i32,
    ) -> Self {
        let old = GetCurrentObject(dc, OBJ_FONT as u32);
        let mut rows = Vec::new();
        let mut max_width = width;
        for block in blocks {
            let base = match block.kind {
                Kind::Heading(1) => settings.font_size + 14,
                Kind::Heading(2) => settings.font_size + 9,
                Kind::Heading(_) => settings.font_size + 4,
                Kind::Fence => (settings.font_size - 3).max(12),
                _ => settings.font_size,
            };
            let height = fonts.scale(
                base + if matches!(block.kind, Kind::Heading(_)) {
                    16
                } else {
                    10
                },
            );
            let indent = fonts.scale(
                block.indent
                    + if matches!(block.kind, Kind::Code | Kind::Table | Kind::Fence) {
                        12
                    } else {
                        0
                    },
            );
            let mut row = Row {
                start: block.start,
                end: block.end,
                runs: Vec::new(),
                height,
                kind: block.kind,
            };
            let mut x = indent;
            for Span {
                text,
                bold,
                italic,
                code,
                link,
                strike,
            } in &block.spans
            {
                let key = FontKey {
                    size: base,
                    bold: *bold || matches!(block.kind, Kind::Heading(_)),
                    italic: *italic,
                    mono: *code || matches!(block.kind, Kind::Code | Kind::Table | Kind::Fence),
                    strike: *strike,
                };
                SelectObject(dc, fonts.get(key));
                // Display control characters visibly instead of letting them affect native text APIs.
                let text: String = text
                    .chars()
                    .flat_map(|c| {
                        if c == '\t' {
                            "    ".chars().collect::<Vec<_>>()
                        } else {
                            vec![if c.is_control() { '�' } else { c }]
                        }
                    })
                    .collect();
                let utf16: Vec<u16> = text.encode_utf16().collect();
                let mut pos = 0;
                while pos < utf16.len() {
                    let mut fit = 0;
                    let mut size = SIZE::default();
                    let remaining = (width - x).max(1);
                    GetTextExtentExPointW(
                        dc,
                        utf16[pos..].as_ptr(),
                        (utf16.len() - pos) as i32,
                        if settings.wrap {
                            remaining
                        } else {
                            i32::MAX / 4
                        },
                        &mut fit,
                        std::ptr::null_mut(),
                        &mut size,
                    );
                    let mut take = (fit as usize).min(utf16.len() - pos);
                    if take == 0 && x > indent {
                        rows.push(row);
                        row = Row {
                            start: block.start,
                            end: block.end,
                            runs: Vec::new(),
                            height,
                            kind: block.kind,
                        };
                        x = indent;
                        continue;
                    }
                    if take == 0 {
                        take = 1;
                    }
                    if pos + take < utf16.len()
                        && (0xd800..=0xdbff).contains(&utf16[pos + take - 1])
                    {
                        if take > 1 {
                            take -= 1;
                        } else {
                            take = 2.min(utf16.len() - pos);
                        }
                    }
                    if settings.wrap
                        && pos + take < utf16.len()
                        && let Some(space) = utf16[pos..pos + take].iter().rposition(|&c| c == 32)
                        && space > take / 2
                    {
                        take = space + 1;
                    }
                    let slice = &utf16[pos..pos + take];
                    GetTextExtentPoint32W(dc, slice.as_ptr(), take as i32, &mut size);
                    row.runs.push(Run {
                        text: slice.to_vec(),
                        x,
                        width: size.cx,
                        font: key,
                        link: link.clone(),
                        code: *code,
                    });
                    x += size.cx;
                    pos += take;
                    max_width = max_width.max(x);
                    if settings.wrap && pos < utf16.len() {
                        rows.push(row);
                        row = Row {
                            start: block.start,
                            end: block.end,
                            runs: Vec::new(),
                            height,
                            kind: block.kind,
                        };
                        x = indent;
                    }
                }
            }
            rows.push(row);
        }
        SelectObject(dc, old);
        Self { rows, max_width }
    }
}
