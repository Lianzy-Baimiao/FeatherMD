//! GDI text layout, limited to one bounded page; only visible rows are painted.
#![allow(unsafe_op_in_unsafe_fn)]
use feathermd::{
    markdown::{Block, Kind, Span},
    settings::Settings,
    table::{self, Align},
    text,
};
use std::{collections::BTreeMap, rc::Rc};
/// Logical padding inside table cells.
const CELL_PAD_X: i32 = 10;
const CELL_PAD_Y: i32 = 4;
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
    family: String,
    pub dpi: u32,
}
impl Fonts {
    pub fn new(dpi: u32) -> Self {
        Self::with_family(dpi, feathermd::i18n::current().ui_font())
    }
    pub fn with_family(dpi: u32, family: &str) -> Self {
        Self {
            family: Settings::validate_font_family(family)
                .unwrap_or("Microsoft YaHei UI")
                .into(),
            cache: BTreeMap::new(),
            dpi,
        }
    }
    pub unsafe fn get(&mut self, key: FontKey) -> HFONT {
        *self.cache.entry(key).or_insert_with(|| {
            let face: Vec<u16> = if key.mono { "Consolas" } else { &self.family }
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
    /// Displayed UTF-16 units of the block placed on earlier rows (wrapping).
    pub unit: usize,
    pub table: Option<TableLine>,
}
/// One visual line of a table row: the grid shared by the whole table.
#[derive(Clone)]
pub struct TableLine {
    /// Vertical rule positions relative to the text origin, left to right.
    pub edges: Rc<[i32]>,
    pub header: bool,
    /// First visual line of its table row: draws the rule above the row.
    pub first: bool,
    /// Last visual line of the whole table: draws the bottom rule.
    pub close: bool,
    /// Cell padding above the text band, and the band's height.
    pub inset: i32,
    pub band: i32,
    /// The row's cells as tab-separated text, for copying.
    pub copy: Rc<str>,
}
fn row(block: &Block, height: i32, unit: usize) -> Row {
    Row {
        start: block.start,
        end: block.end,
        runs: Vec::new(),
        height,
        kind: block.kind,
        unit,
        table: None,
    }
}
impl feathermd::viewport::RowHeight for Row {
    fn height(&self) -> i32 {
        self.height
    }
}
pub struct Layout {
    pub rows: Vec<Row>,
    pub max_width: i32,
    /// Right edge of the widest table; wider than the layout width only when even
    /// the narrowest columns do not fit, so tables scroll on their own.
    pub table_width: i32,
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
        let mut table_width = 0;
        let mut index = 0;
        while index < blocks.len() {
            let block = &blocks[index];
            if let Some(group) = block.table.as_ref().map(|t| t.group) {
                let count = blocks[index..]
                    .iter()
                    .take_while(|b| b.table.as_ref().is_some_and(|t| t.group == group))
                    .count();
                let right = layout_table(
                    dc,
                    &blocks[index..index + count],
                    settings,
                    fonts,
                    width,
                    &mut rows,
                );
                table_width = table_width.max(right);
                max_width = max_width.max(right);
                index += count;
                continue;
            }
            index += 1;
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
                    + if matches!(block.kind, Kind::Code | Kind::Fence) {
                        12
                    } else {
                        0
                    },
            );
            let mut placed = 0;
            let mut row = row(block, height, placed);
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
                    mono: *code || matches!(block.kind, Kind::Code | Kind::Fence),
                    strike: *strike,
                };
                SelectObject(dc, fonts.get(key));
                // Display control characters visibly instead of letting them affect native text APIs.
                let utf16: Vec<u16> = text::display(text).encode_utf16().collect();
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
                        row = self::row(block, height, placed);
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
                    placed += take;
                    max_width = max_width.max(x);
                    if settings.wrap && pos < utf16.len() {
                        rows.push(row);
                        row = self::row(block, height, placed);
                        x = indent;
                    }
                }
            }
            if row.runs.is_empty() {
                // Empty lines keep an anchor at the text origin for source selection.
                row.runs.push(Run {
                    text: Vec::new(),
                    x: indent,
                    width: 0,
                    font: FontKey {
                        size: base,
                        bold: false,
                        italic: false,
                        mono: matches!(block.kind, Kind::Code | Kind::Fence),
                        strike: false,
                    },
                    link: None,
                    code: false,
                });
            }
            rows.push(row);
        }
        SelectObject(dc, old);
        Self {
            rows,
            max_width,
            table_width,
        }
    }
}
/// One cell's displayed text and the pen position after each UTF-16 unit.
struct Cell {
    units: Vec<u16>,
    ends: Vec<i32>,
    /// `(first unit, font, link, inline code)` of each styled piece.
    pieces: Vec<(usize, FontKey, Option<String>, bool)>,
}
unsafe fn measure(dc: HDC, spans: &[Span], size: i32, header: bool, fonts: &mut Fonts) -> Cell {
    let mut cell = Cell {
        units: Vec::new(),
        ends: Vec::new(),
        pieces: Vec::new(),
    };
    for span in spans {
        let units: Vec<u16> = text::display(&span.text).encode_utf16().collect();
        if units.is_empty() {
            continue;
        }
        let key = FontKey {
            size,
            bold: span.bold || header,
            italic: span.italic,
            mono: span.code,
            strike: span.strike,
        };
        SelectObject(dc, fonts.get(key));
        let mut dx = vec![0; units.len()];
        let (mut fit, mut extent) = (0, SIZE::default());
        GetTextExtentExPointW(
            dc,
            units.as_ptr(),
            units.len() as i32,
            i32::MAX / 4,
            &mut fit,
            dx.as_mut_ptr(),
            &mut extent,
        );
        let origin = cell.ends.last().copied().unwrap_or(0);
        let mut previous = 0;
        for &end in &dx {
            // Keep pen positions monotonic even if a driver reports less.
            previous = previous.max(end);
            cell.ends.push(origin + previous);
        }
        cell.pieces
            .push((cell.units.len(), key, span.link.clone(), span.code));
        cell.units.extend(units);
    }
    cell
}
/// Lays out consecutive rows of one table and returns its right edge.
unsafe fn layout_table(
    dc: HDC,
    blocks: &[Block],
    settings: &Settings,
    fonts: &mut Fonts,
    width: i32,
    rows: &mut Vec<Row>,
) -> i32 {
    let size = settings.font_size;
    let (pad_x, pad_y) = (fonts.scale(CELL_PAD_X), fonts.scale(CELL_PAD_Y));
    let band = fonts.scale(size + 10);
    let em = fonts.scale(size);
    let table: Vec<(&Block, &feathermd::markdown::TableRow)> = blocks
        .iter()
        .filter_map(|b| b.table.as_ref().map(|t| (b, t)))
        .collect();
    let Some(align) = table.first().map(|(_, t)| t.align.clone()) else {
        return 0;
    };
    let columns = align.len();
    let cells: Vec<Vec<Cell>> = table
        .iter()
        .map(|(_, t)| {
            t.cells
                .iter()
                .map(|spans| measure(dc, spans, size, t.header, fonts))
                .collect()
        })
        .collect();
    let mut natural = vec![2 * pad_x; columns];
    let mut minimum = vec![2 * pad_x; columns];
    for row in &cells {
        for (c, cell) in row.iter().enumerate().take(columns) {
            let full = cell.ends.last().copied().unwrap_or(0);
            natural[c] = natural[c].max(full + 2 * pad_x);
            // Columns keep about six ems (or their content) and whole words up to
            // twelve ems; longer URLs and words wrap inside the column.
            let token = table::widest_token(&cell.units, &cell.ends).clamp(6 * em, 12 * em);
            minimum[c] = minimum[c].max(token + 2 * pad_x);
        }
    }
    let widths = if settings.wrap {
        table::fit_columns(&natural, &minimum, width)
    } else {
        natural
    };
    let mut edges = Vec::with_capacity(columns + 1);
    let mut x = 0;
    edges.push(0);
    for w in &widths {
        x += w;
        edges.push(x);
    }
    let right = x;
    // Keep the closing rule inside the table's own pixels.
    if let Some(last) = edges.last_mut() {
        *last = (*last - 1).max(0);
    }
    let edges: Rc<[i32]> = edges.into();
    for (r, ((block, row), cells)) in table.iter().zip(&cells).enumerate() {
        let copy: Rc<str> = row
            .cells
            .iter()
            .map(|spans| {
                spans
                    .iter()
                    .map(|s| s.text.replace(['\t', '\r', '\n'], " "))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\t")
            .into();
        let lines: Vec<Vec<(usize, usize)>> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, w)| {
                let content = if settings.wrap {
                    (w - 2 * pad_x).max(1)
                } else {
                    i32::MAX / 4
                };
                let lines = table::break_lines(&cell.units, &cell.ends, content);
                if lines.is_empty() {
                    vec![(0, 0)]
                } else {
                    lines
                }
            })
            .collect();
        let count = lines.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for line in 0..count {
            let first = line == 0;
            let last = line + 1 == count;
            let mut out = Row {
                start: block.start,
                end: block.end,
                runs: Vec::new(),
                height: band + if first { pad_y } else { 0 } + if last { pad_y } else { 0 },
                kind: Kind::Table,
                unit: 0,
                table: Some(TableLine {
                    edges: edges.clone(),
                    header: row.header,
                    first,
                    close: last && r + 1 == table.len(),
                    inset: if first { pad_y } else { 0 },
                    band,
                    copy: copy.clone(),
                }),
            };
            for (c, cell) in cells.iter().enumerate().take(columns) {
                let Some(&(a, b)) = lines.get(c).and_then(|l| l.get(line)) else {
                    continue;
                };
                let at = |i: usize| if i == 0 { 0 } else { cell.ends[i - 1] };
                let used = at(b) - at(a);
                let room = (widths[c] - 2 * pad_x).max(0);
                let shift = match align.get(c).copied().unwrap_or_default() {
                    Align::Left => 0,
                    Align::Center => ((room - used) / 2).max(0),
                    Align::Right => (room - used).max(0),
                };
                let origin = edges[c] + pad_x + shift - at(a);
                for (p, piece) in cell.pieces.iter().enumerate() {
                    let next = cell.pieces.get(p + 1).map_or(cell.units.len(), |n| n.0);
                    let (from, to) = (piece.0.max(a), next.min(b));
                    if from >= to {
                        continue;
                    }
                    out.runs.push(Run {
                        text: cell.units[from..to].to_vec(),
                        x: origin + at(from),
                        width: at(to) - at(from),
                        font: piece.1,
                        link: piece.2.clone(),
                        code: piece.3,
                    });
                }
            }
            rows.push(out);
        }
    }
    right
}
