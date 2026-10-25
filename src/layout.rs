//! GDI text layout, limited to one bounded page; only visible rows are painted.
//! Spacing follows one rhythm: body lines 1.6× the font size, a short gap for a
//! blank line, more room above headings, smaller monospace for code.
#![allow(unsafe_op_in_unsafe_fn)]
use feathermd::{
    markdown::{self, Block, Kind, Span},
    settings::Settings,
    table::{self, Align},
    text,
};
use std::{collections::BTreeMap, rc::Rc};
/// Logical padding inside table cells.
const CELL_PAD_X: i32 = 10;
pub const CELL_PAD_Y: i32 = 4;
/// Logical padding of inline code and keyboard chips.
const CHIP_PAD: i32 = 4;
/// Logical room above an image and below the text line it stands on.
const PICTURE_PAD: i32 = 4;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontKey {
    pub size: i32,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub strike: bool,
    pub underline: bool,
}
pub struct Fonts {
    cache: BTreeMap<FontKey, HFONT>,
    /// Ascent and descent in pixels, measured once per font.
    metrics: BTreeMap<FontKey, (i32, i32)>,
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
            metrics: BTreeMap::new(),
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
                key.underline as u32,
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
    /// Ascent and descent of `key` in pixels.
    pub unsafe fn metrics(&mut self, dc: HDC, key: FontKey) -> (i32, i32) {
        if let Some(&m) = self.metrics.get(&key) {
            return m;
        }
        let old = SelectObject(dc, self.get(key));
        let mut tm: TEXTMETRICW = std::mem::zeroed();
        GetTextMetricsW(dc, &mut tm);
        SelectObject(dc, old);
        let size = self.scale(key.size);
        let m = if tm.tmHeight > 0 {
            (tm.tmAscent, tm.tmDescent)
        } else {
            (size * 4 / 5, size / 5)
        };
        self.metrics.insert(key, m);
        m
    }
    pub fn scale(&self, value: i32) -> i32 {
        (value as i64 * self.dpi as i64 / 96) as i32
    }
    pub fn family(&self) -> &str {
        &self.family
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
/// Body line height for font size `size` (logical pixels).
pub fn line(size: i32) -> i32 {
    (size * 8 + 2) / 5
}
/// Monospace size used for code next to body text of size `size`.
pub fn code_size(size: i32) -> i32 {
    ((size * 7 + 4) / 8).max(11)
}
/// Heading size for level 1–6.
pub fn heading_size(size: i32, level: u8) -> i32 {
    match level {
        1 => size * 2,
        2 => (size * 8 + 2) / 5,
        3 => (size * 13 + 5) / 10,
        4 => (size * 23 + 10) / 20,
        _ => size,
    }
}
#[derive(Clone, Default)]
pub struct Run {
    pub text: Vec<u16>,
    pub x: i32,
    pub width: i32,
    pub font: FontKey,
    pub link: Option<String>,
    /// Explicit image download, distinct from an enclosing hyperlink.
    pub load_image: Option<String>,
    /// Inline code: a rounded chip from `chip.0` to `chip.1` (relative to the text
    /// origin, like `x`).
    pub code: bool,
    pub kbd: bool,
    pub mark: bool,
    pub chip: (i32, i32),
    pub token: Option<feathermd::highlight::Token>,
    /// A task checkbox drawn instead of `text` (which stays the copy text).
    pub check: Option<bool>,
    /// An open disclosure chevron (`<summary>`), drawn the same way.
    pub disclosure: bool,
    /// Ascent and descent of `font`, and how far it sits above the baseline
    /// (superscript positive, subscript negative), in pixels.
    pub ascent: i32,
    pub descent: i32,
    pub rise: i32,
    /// Secondary text (an image placeholder).
    pub muted: bool,
    /// A decoded image drawn in place of `text` (one placeholder unit).
    pub picture: Option<Inline>,
}
/// An image placed in a line of text, `width` (the run's) by `height` pixels.
#[derive(Clone)]
pub struct Inline {
    /// Cache key of the decoded image.
    pub key: Rc<str>,
    /// Top edge relative to the row's top.
    pub top: i32,
    pub height: i32,
    /// Copied in place of the image.
    pub alt: String,
}
/// How the preview shows one image, decided by the caller's cache.
pub enum Picture {
    /// Images are off: the plain placeholder.
    Off,
    /// Decoded, `width` × `height` pixels.
    Ready {
        width: u32,
        height: u32,
        key: Rc<str>,
    },
    /// Not shown, with the reason.
    Note(&'static str),
    Load {
        url: String,
        note: &'static str,
    },
}
/// What layout needs beyond the blocks: settings, the image cache and the tallest
/// image a row may hold (the reading area's height).
pub struct View<'a> {
    pub settings: &'a Settings,
    pub pictures: &'a dyn Fn(&markdown::Image) -> Picture,
    pub max_height: i32,
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
    /// Baseline of the text, from the row's top.
    pub baseline: i32,
    /// First and last visual line of its block.
    pub first: bool,
    pub last: bool,
    /// Left edge of the block's content relative to the text origin.
    pub indent: i32,
    /// Quote bar position relative to the text origin, for quoted blocks.
    pub bar: Option<i32>,
    /// Inside a code box with a rounded background (set after layout).
    pub boxed: bool,
}
impl Row {
    fn new(block: &Block, height: i32, unit: usize) -> Self {
        Self {
            start: block.start,
            end: block.end,
            runs: Vec::new(),
            height,
            kind: block.kind,
            unit,
            table: None,
            baseline: 0,
            first: true,
            last: true,
            indent: 0,
            bar: None,
            boxed: false,
        }
    }
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
    /// Every other body row gets a faint stripe.
    pub stripe: bool,
    /// The row's cells as tab-separated text, for copying.
    pub copy: Rc<str>,
    /// The row's displayed cells joined by tabs: the units a selection counts in,
    /// shared by all visual lines of the row.
    pub text: Rc<[u16]>,
    /// First unit of each cell in `text`.
    pub cells: Rc<[usize]>,
    /// Unit of `text` where each run of this visual line starts.
    pub starts: Vec<usize>,
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
/// Vertical shape of a block: font size, line height, space above and below.
struct Metrics {
    size: i32,
    line: i32,
    above: i32,
    below: i32,
}
fn metrics(block: &Block, settings: &Settings, previous: Option<Kind>) -> Metrics {
    let f = settings.font_size;
    let plain = |size: i32, line: i32| Metrics {
        size,
        line,
        above: 0,
        below: 0,
    };
    match block.kind {
        Kind::Heading(level) => {
            let size = heading_size(f, level);
            Metrics {
                size,
                line: (size * 13 + 5) / 10,
                // More room above than below binds a heading to its section.
                above: if previous.is_none() { 0 } else { f / 2 },
                below: if level <= 2 { (f + 1) / 3 } else { 0 },
            }
        }
        Kind::Code if settings.raw => plain(f, line(f)),
        Kind::Code => {
            let size = code_size(f);
            plain(size, (size * 3 + 1) / 2)
        }
        Kind::Fence => {
            let size = (f * 3 / 4).max(11);
            if block.spans.iter().any(|s| !s.text.is_empty()) {
                Metrics {
                    size,
                    line: size + 4,
                    above: 6,
                    below: 0,
                }
            } else {
                plain(size, 8)
            }
        }
        Kind::Spacer => plain(f, 4),
        Kind::Rule => plain(f, line(f)),
        // Logs read densely, like code; plain text like body text, a blank line a
        // whole line high as in the file.
        Kind::Log if settings.raw => plain(f, line(f)),
        Kind::Log => {
            let size = code_size(f);
            plain(size, (size * 3 + 1) / 2)
        }
        Kind::Text => plain(f, line(f)),
        // A blank line is a paragraph gap, shorter than a text line.
        Kind::Paragraph | Kind::Quote if block.spans.is_empty() => plain(f, (f * 3 + 2) / 4),
        _ => plain(f, line(f)),
    }
}
/// Baseline that centres text of `ascent` and `descent` in a band of `band` pixels.
fn baseline(band: i32, (ascent, descent): (i32, i32)) -> i32 {
    (band + ascent - descent + 1) / 2
}
impl Layout {
    pub unsafe fn build(
        dc: HDC,
        blocks: &[Block],
        view: &View,
        fonts: &mut Fonts,
        width: i32,
    ) -> Self {
        let settings = view.settings;
        let old = GetCurrentObject(dc, OBJ_FONT as u32);
        let mut out = Self {
            rows: Vec::new(),
            max_width: width,
            table_width: 0,
        };
        let mut index = 0;
        let mut previous = None;
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
                    &mut out.rows,
                );
                out.table_width = out.table_width.max(right);
                out.max_width = out.max_width.max(right);
                index += count;
                previous = Some(Kind::Table);
                continue;
            }
            index += 1;
            if settings.raw {
                raw_rows(dc, block, settings, fonts, width, &mut out);
            } else {
                let m = metrics(block, settings, previous);
                flow(dc, block, view, fonts, width, &m, &mut out);
            }
            previous = Some(block.kind);
        }
        SelectObject(dc, old);
        out
    }
}
/// A styled piece of a block: its first unit, font and chip padding.
struct Piece {
    from: usize,
    span: usize,
    key: FontKey,
    lead: i32,
    trail: i32,
    ascent: i32,
    descent: i32,
    rise: i32,
    muted: bool,
    /// A decoded image: one unit as wide as the image, and its height.
    picture: Option<(Rc<str>, i32)>,
    load_image: Option<String>,
}
/// One preview block as visual lines. Its pieces are measured once and broken
/// together: after spaces or next to CJK, never before closing punctuation; list
/// items hang after their marker; `<br>` ends a line. A decoded image is one
/// unbreakable unit, so logos, badge rows and inline icons share this path.
unsafe fn flow(
    dc: HDC,
    block: &Block,
    view: &View,
    fonts: &mut Fonts,
    width: i32,
    m: &Metrics,
    out: &mut Layout,
) {
    let settings = view.settings;
    let heading = matches!(block.kind, Kind::Heading(_));
    let code_block = matches!(block.kind, Kind::Code | Kind::Fence);
    // Log lines are monospace like code, without the code box's inset.
    let mono = code_block || block.kind == Kind::Log;
    let indent = fonts.scale(block.indent + if code_block { 16 } else { 0 });
    let base = FontKey {
        size: m.size,
        bold: heading,
        mono,
        ..FontKey::default()
    };
    let base_metrics = fonts.metrics(dc, base);
    let band = fonts.scale(m.line);
    let (above, below) = (fonts.scale(m.above), fonts.scale(m.below));
    let pad = fonts.scale(CHIP_PAD);
    let mut units: Vec<u16> = Vec::new();
    let mut ends: Vec<i32> = Vec::new();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut breaks = Vec::new();
    for (index, span) in block.spans.iter().enumerate() {
        if span.line_break {
            breaks.push(units.len());
            continue;
        }
        let chip = (span.code || span.kbd) && !mono;
        let size = if span.code || span.kbd {
            code_size(m.size)
        } else if span.script != 0 {
            (m.size * 3 + 2) / 4
        } else {
            m.size
        };
        let key = FontKey {
            size,
            bold: span.bold || heading,
            italic: span.italic,
            mono: span.code || span.kbd || mono,
            strike: span.strike,
            underline: span.underline,
        };
        // A decoded image takes one unit as wide as the image; any other image
        // shows its placeholder text.
        let mut picture = None;
        let mut load_image = None;
        let shown = match &span.image {
            Some(image) => match (view.pictures)(image) {
                Picture::Ready {
                    width: natural_width,
                    height: natural_height,
                    key,
                } => {
                    // Fit the narrowest line the image may land on: wrapped lines of
                    // a list item start where the text after its marker starts.
                    let hang = if block.marker && !pieces.is_empty() {
                        let after_marker = pieces.get(1).map_or(units.len(), |p| p.from);
                        after_marker.checked_sub(1).map_or(0, |i| ends[i])
                    } else {
                        0
                    };
                    let size = feathermd::image::display_size(
                        (natural_width, natural_height),
                        (image.width, image.height),
                        fonts.dpi as f32 / 96.0,
                        (
                            (width - indent - hang).max(1),
                            (view.max_height - fonts.scale(PICTURE_PAD) * 2).max(1),
                        ),
                    );
                    picture = Some((key, size));
                    "\u{fffc}".to_owned()
                }
                Picture::Off => markdown::image_placeholder(image, &span.text, None),
                Picture::Load { url, note } => {
                    load_image = Some(url);
                    markdown::image_placeholder(image, &span.text, Some(note))
                }
                Picture::Note(reason) => {
                    markdown::image_placeholder(image, &span.text, Some(reason))
                }
            },
            None => text::display(&span.text),
        };
        let utf16: Vec<u16> = shown.encode_utf16().collect();
        if utf16.is_empty() {
            continue;
        }
        let (ascent, descent) = fonts.metrics(dc, key);
        SelectObject(dc, fonts.get(key));
        let mut dx = vec![0; utf16.len()];
        let (mut fit, mut extent) = (0, SIZE::default());
        GetTextExtentExPointW(
            dc,
            utf16.as_ptr(),
            utf16.len() as i32,
            i32::MAX / 4,
            &mut fit,
            dx.as_mut_ptr(),
            &mut extent,
        );
        let (lead, trail) = if chip && picture.is_none() {
            (pad, pad)
        } else {
            (0, 0)
        };
        let from = units.len();
        let origin = ends.last().copied().unwrap_or(0) + lead;
        let mut previous = 0;
        for &d in &dx {
            // Keep pen positions monotonic even if a driver reports less.
            previous = previous.max(d);
            ends.push(origin + previous);
        }
        if span.check.is_some() || span.disclosure {
            // A drawn box or chevron, then a gap, in place of the glyphs.
            let side = (fonts.scale(m.size) * 7 + 4) / 8;
            let gap = fonts.scale(7);
            for (i, end) in ends[from..].iter_mut().enumerate() {
                *end = origin + if i == 0 { side } else { side + gap };
            }
        }
        if let Some(last) = ends.last_mut() {
            *last += trail;
        }
        if let Some((_, (image_width, _))) = &picture {
            // The image's single unit is exactly as wide as the image.
            ends[from] = origin + image_width;
        }
        let em = fonts.scale(m.size);
        pieces.push(Piece {
            from,
            span: index,
            key,
            lead,
            trail,
            ascent,
            descent,
            rise: match span.script {
                1 => em / 3,
                -1 => -em / 6,
                _ => 0,
            },
            muted: span.image.is_some() && picture.is_none(),
            load_image,
            picture: picture.map(|(key, (_, image_height))| (key, image_height)),
        });
        units.extend(utf16);
    }
    let bar = block.bar.map(|b| fonts.scale(b));
    let anchor = |x: i32| Run {
        x,
        font: base,
        ascent: base_metrics.0,
        descent: base_metrics.1,
        ..Run::default()
    };
    if pieces.is_empty() {
        let mut row = Row::new(block, above + band + below, 0);
        row.baseline = above + baseline(band, base_metrics);
        row.indent = indent;
        row.bar = bar;
        row.runs.push(anchor(indent));
        out.rows.push(row);
        return;
    }
    let at = |i: usize| if i == 0 { 0 } else { ends[i - 1] };
    // Wrapped lines of a list item start after its marker.
    let hang = if block.marker {
        pieces.get(1).map_or(0, |p| at(p.from))
    } else {
        0
    };
    let first_room = (width - indent).max(1);
    let rest_room = (width - indent - hang).max(1);
    let mut lines: Vec<(usize, usize)> = Vec::new();
    let mut from = 0;
    breaks.push(units.len());
    for cut in breaks {
        let cut = cut.max(from);
        let room = if lines.is_empty() {
            first_room
        } else {
            rest_room
        };
        // Code, logs and plain text keep the spaces that open their line; Markdown
        // prose drops them.
        let keep = if (mono || block.kind == Kind::Text) && lines.is_empty() {
            units[from..cut].iter().take_while(|&&u| u == 32).count()
        } else {
            0
        };
        let segment = if settings.wrap {
            let mut segment = table::break_range(
                &units,
                &ends,
                from + keep,
                cut,
                (room - (at(from + keep) - at(from))).max(1),
                rest_room,
            );
            if let Some(first) = segment.first_mut().filter(|_| keep > 0) {
                first.0 = from;
            }
            segment
        } else {
            let mut start = from + keep;
            while start < cut && units[start] == 32 {
                start += 1;
            }
            if keep > 0 {
                start = from;
            }
            let mut end = cut;
            while end > start && units[end - 1] == 32 {
                end -= 1;
            }
            if start < end {
                vec![(start, end)]
            } else {
                Vec::new()
            }
        };
        if segment.is_empty() {
            lines.push((cut, cut));
        } else {
            lines.extend(segment);
        }
        from = cut;
    }
    let count = lines.len();
    for (li, &(a, b)) in lines.iter().enumerate() {
        let x0 = indent + if li > 0 && block.marker { hang } else { 0 };
        let room = if li == 0 { first_room } else { rest_room };
        let origin = at(a);
        let used = at(b) - origin;
        let shift = match block.align {
            Align::Left => 0,
            Align::Center => ((room - used) / 2).max(0),
            Align::Right => (room - used).max(0),
        };
        let top = if li == 0 { above } else { 0 };
        let bottom = if li + 1 == count { below } else { 0 };
        // A line holding an image grows to it: images stand on the text baseline,
        // with a little room above them and below the descenders.
        let tallest = pieces
            .iter()
            .enumerate()
            .filter(|&(pi, p)| {
                let next = pieces.get(pi + 1).map_or(units.len(), |n| n.from);
                p.from.max(a) < next.min(b)
            })
            .filter_map(|(_, p)| p.picture.as_ref().map(|(_, height)| *height))
            .max();
        let (line_band, line_baseline) = match tallest {
            Some(height) => {
                let room = fonts.scale(PICTURE_PAD);
                let image_baseline = (room + height).max(baseline(band, base_metrics));
                (
                    (image_baseline + base_metrics.1 + room).max(band),
                    image_baseline,
                )
            }
            None => (band, baseline(band, base_metrics)),
        };
        let mut row = Row::new(block, top + line_band + bottom, a);
        row.first = li == 0;
        row.last = li + 1 == count;
        row.baseline = top + line_baseline;
        row.indent = indent;
        row.bar = bar;
        for (pi, piece) in pieces.iter().enumerate() {
            let next = pieces.get(pi + 1).map_or(units.len(), |p| p.from);
            let (from, to) = (piece.from.max(a), next.min(b));
            if from >= to {
                continue;
            }
            let span: &Span = &block.spans[piece.span];
            let glyph = at(from) + if from == piece.from { piece.lead } else { 0 };
            let stop = ends[to - 1] - if to == next { piece.trail } else { 0 };
            let left = x0 + shift - origin;
            row.runs.push(Run {
                text: units[from..to].to_vec(),
                x: left + glyph,
                width: (stop - glyph).max(0),
                font: piece.key,
                link: span.link.clone(),
                load_image: piece.load_image.clone(),
                code: span.code && !code_block,
                kbd: span.kbd && !code_block,
                mark: span.mark,
                chip: (left + at(from), left + ends[to - 1]),
                token: span.token,
                check: span.check,
                disclosure: span.disclosure,
                ascent: piece.ascent,
                descent: piece.descent,
                rise: piece.rise,
                muted: piece.muted,
                picture: piece.picture.as_ref().map(|(key, height)| Inline {
                    key: key.clone(),
                    top: row.baseline - height,
                    height: *height,
                    alt: feathermd::image::label(
                        &span.text,
                        span.image.as_ref().map_or("", |i| i.src.as_str()),
                    ),
                }),
            });
        }
        if row.runs.is_empty() {
            row.runs.push(anchor(x0));
        }
        out.max_width = out.max_width.max(x0 + shift + used);
        out.rows.push(row);
    }
}
/// Source mode: every line as monospace text, wrapped by width only, so displayed
/// units map one to one onto the source for character selection.
unsafe fn raw_rows(
    dc: HDC,
    block: &Block,
    settings: &Settings,
    fonts: &mut Fonts,
    width: i32,
    out: &mut Layout,
) {
    let key = FontKey {
        size: settings.font_size,
        mono: true,
        ..FontKey::default()
    };
    let height = fonts.scale(line(settings.font_size));
    let indent = fonts.scale(12);
    let (ascent, descent) = fonts.metrics(dc, key);
    let new_row = |placed: usize| {
        let mut row = Row::new(block, height, placed);
        row.baseline = baseline(height, (ascent, descent));
        row.indent = indent;
        row
    };
    let run = |text: Vec<u16>, x: i32, width: i32| Run {
        text,
        x,
        width,
        font: key,
        ascent,
        descent,
        ..Run::default()
    };
    SelectObject(dc, fonts.get(key));
    let mut placed = 0;
    let mut row = new_row(placed);
    let mut x = indent;
    for span in &block.spans {
        // Display control characters visibly instead of letting them affect native text APIs.
        let utf16: Vec<u16> = text::display(&span.text).encode_utf16().collect();
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
                out.rows.push(row);
                row = new_row(placed);
                x = indent;
                continue;
            }
            if take == 0 {
                take = 1;
            }
            if pos + take < utf16.len() && (0xd800..=0xdbff).contains(&utf16[pos + take - 1]) {
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
            row.runs.push(run(slice.to_vec(), x, size.cx));
            x += size.cx;
            pos += take;
            placed += take;
            out.max_width = out.max_width.max(x);
            if settings.wrap && pos < utf16.len() {
                out.rows.push(row);
                row = new_row(placed);
                x = indent;
            }
        }
    }
    if row.runs.is_empty() {
        // Empty lines keep an anchor at the text origin for source selection.
        row.runs.push(run(Vec::new(), indent, 0));
    }
    out.rows.push(row);
}
/// One cell's displayed text and the pen position after each UTF-16 unit.
struct Cell {
    units: Vec<u16>,
    ends: Vec<i32>,
    pieces: Vec<CellPiece>,
}
/// A styled piece of a cell.
struct CellPiece {
    from: usize,
    key: FontKey,
    link: Option<String>,
    code: bool,
    ascent: i32,
    descent: i32,
}
unsafe fn measure(dc: HDC, spans: &[Span], size: i32, header: bool, fonts: &mut Fonts) -> Cell {
    let mut cell = Cell {
        units: Vec::new(),
        ends: Vec::new(),
        pieces: Vec::new(),
    };
    for span in spans {
        // Images in table cells keep their placeholder text.
        let shown = if span.line_break {
            " ".to_owned()
        } else if let Some(image) = &span.image {
            markdown::image_placeholder(image, &span.text, None)
        } else {
            text::display(&span.text)
        };
        let units: Vec<u16> = shown.encode_utf16().collect();
        if units.is_empty() {
            continue;
        }
        let key = FontKey {
            size: if span.code || span.kbd {
                code_size(size)
            } else {
                size
            },
            bold: span.bold || header,
            italic: span.italic,
            mono: span.code || span.kbd,
            strike: span.strike,
            underline: span.underline,
        };
        let (ascent, descent) = fonts.metrics(dc, key);
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
        cell.pieces.push(CellPiece {
            from: cell.units.len(),
            key,
            link: span.link.clone(),
            code: span.code || span.kbd,
            ascent,
            descent,
        });
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
    let band = fonts.scale(line(size));
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
    let mut body = 0;
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
        // Selection units: the displayed cells joined by tabs.
        let mut text = Vec::new();
        let mut starts_of = Vec::with_capacity(cells.len());
        for cell in cells.iter().take(columns) {
            if !starts_of.is_empty() {
                text.push(u16::from(b'\t'));
            }
            starts_of.push(text.len());
            text.extend_from_slice(&cell.units);
        }
        let (text, cell_starts): (Rc<[u16]>, Rc<[usize]>) = (text.into(), starts_of.into());
        let stripe = !row.header && {
            body += 1;
            body % 2 == 0
        };
        let base = FontKey {
            size,
            bold: row.header,
            ..FontKey::default()
        };
        let base_metrics = fonts.metrics(dc, base);
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
            let inset = if first { pad_y } else { 0 };
            let (mut runs, mut starts) = (Vec::new(), Vec::new());
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
                    let next = cell.pieces.get(p + 1).map_or(cell.units.len(), |n| n.from);
                    let (from, to) = (piece.from.max(a), next.min(b));
                    if from >= to {
                        continue;
                    }
                    starts.push(cell_starts[c] + from);
                    runs.push(Run {
                        text: cell.units[from..to].to_vec(),
                        x: origin + at(from),
                        width: at(to) - at(from),
                        font: piece.key,
                        link: piece.link.clone(),
                        code: piece.code,
                        chip: (
                            origin + at(from) - fonts.scale(2),
                            origin + at(to) + fonts.scale(2),
                        ),
                        ascent: piece.ascent,
                        descent: piece.descent,
                        ..Run::default()
                    });
                }
            }
            rows.push(Row {
                start: block.start,
                end: block.end,
                runs,
                height: band + if first { pad_y } else { 0 } + if last { pad_y } else { 0 },
                kind: Kind::Table,
                unit: 0,
                table: Some(TableLine {
                    edges: edges.clone(),
                    header: row.header,
                    first,
                    close: last && r + 1 == table.len(),
                    stripe,
                    copy: copy.clone(),
                    text: text.clone(),
                    cells: cell_starts.clone(),
                    starts,
                }),
                baseline: inset + baseline(band, base_metrics),
                first,
                last,
                indent: 0,
                bar: None,
                boxed: false,
            });
        }
    }
    right
}
