//! The reading column: code boxes, quotes, tables, rules, chips, checkboxes and
//! coloured code, drawn from the bounded layout of the visible rows only.
use super::chrome::{chrome_round, mix};
use super::*;
use feathermd::highlight::Token;

impl Palette {
    /// Code-token colours, harmonised with each skin's text colour.
    pub(super) fn token(&self, token: Token) -> u32 {
        let dark = self.is_dark();
        let (hue, weight) = match (token, dark) {
            (Token::Keyword, false) => (rgb(166, 38, 164), 82),
            (Token::Keyword, true) => (rgb(203, 143, 238), 85),
            (Token::Type, false) => (rgb(0, 118, 138), 82),
            (Token::Type, true) => (rgb(86, 199, 210), 85),
            (Token::Function, false) => (rgb(40, 92, 196), 82),
            (Token::Function, true) => (rgb(118, 170, 255), 85),
            (Token::String, false) => (rgb(42, 128, 44), 82),
            (Token::String, true) => (rgb(158, 206, 106), 85),
            (Token::Number, false) => (rgb(178, 92, 0), 85),
            (Token::Number, true) => (rgb(240, 160, 90), 85),
            (Token::Comment, false) => (rgb(128, 132, 128), 70),
            (Token::Comment, true) => (rgb(124, 132, 140), 70),
            (Token::Meta, false) => (rgb(150, 100, 30), 75),
            (Token::Meta, true) => (rgb(220, 180, 110), 80),
            (Token::Inserted, false) => (rgb(30, 130, 60), 85),
            (Token::Inserted, true) => (rgb(120, 210, 140), 85),
            (Token::Deleted, false) => (rgb(190, 40, 50), 85),
            (Token::Deleted, true) => (rgb(250, 120, 120), 85),
        };
        mix(self.fg, hue, weight)
    }
    /// Dark skins have a darker background than text.
    pub(super) fn is_dark(&self) -> bool {
        let sum = |c: u32| (c & 255) + ((c >> 8) & 255) + ((c >> 16) & 255);
        sum(self.bg) < sum(self.fg)
    }
    /// The faint background of every other table body row.
    pub(super) fn stripe(&self) -> u32 {
        mix(self.page, self.code, 55)
    }
    /// Background of Find's matches in the text: amber, the current one stronger.
    pub(super) fn find_mark(&self, current: bool) -> u32 {
        let (hue, weight) = match (self.is_dark(), current) {
            (false, true) => (rgb(255, 150, 30), 55),
            (false, false) => (rgb(255, 214, 64), 40),
            (true, true) => (rgb(220, 120, 20), 48),
            (true, false) => (rgb(200, 160, 40), 26),
        };
        mix(self.page, hue, weight)
    }
}

/// Per visible row, Find's matches on it: displayed units `(a, b)` of its block and
/// whether it is the current match.
type Marks = Vec<Vec<(usize, usize, bool)>>;

/// First index of `needle` in `units` at or after `from`; ASCII letters of either
/// case when `fold`.
fn find_units(units: &[u16], needle: &[u16], from: usize, fold: bool) -> Option<usize> {
    if needle.is_empty() || units.len() < needle.len() {
        return None;
    }
    let lower = |u: u16| if (65..=90).contains(&u) { u + 32 } else { u };
    (from..=units.len() - needle.len()).find(|&i| {
        let here = &units[i..i + needle.len()];
        if fold {
            here.iter().zip(needle).all(|(&a, &b)| lower(a) == lower(b))
        } else {
            here == needle
        }
    })
}

impl App {
    /// Find's matches in the visible rows `first..end` of the preview: per row the
    /// displayed units `(a, b)` of each match in its block and whether it is the
    /// current one; with Find closed only the current one. The flag says the
    /// current match was placed, so its row needs no whole-row highlight.
    fn find_marks(&self, first: usize, end: usize) -> (Marks, bool) {
        let mut out = vec![Vec::new(); end.saturating_sub(first)];
        let open = self.edit_mode == Some(FIND) && self.listed();
        let (Some(layout), Some(page)) = (&self.layout, &self.page) else {
            return (out, false);
        };
        if self.query.is_empty() || self.settings.raw || (!open && self.found.is_none()) {
            return (out, false);
        }
        let needle: Vec<u16> = self.query.encode_utf16().collect();
        let rows = &layout.rows;
        let mut placed = false;
        let mut index = first;
        while index < end {
            let block = rows[index].start;
            let from = rows[..index]
                .iter()
                .rposition(|r| r.start != block)
                .map_or(0, |i| i + 1);
            let to = index
                + rows[index..]
                    .iter()
                    .take_while(|r| r.start == block)
                    .count();
            let span = &rows[from..to];
            let units = select::units(span, block);
            // Which occurrence in the block is the current match, counted in its line.
            let wanted = self
                .found
                .filter(|&f| f >= block && f < rows[index].end)
                .and_then(|f| self.occurrence(page, block, f));
            let mut at = 0;
            let mut k = 0;
            while let Some(i) = find_units(&units, &needle, at, !self.match_case) {
                let current = wanted == Some(k);
                placed |= current;
                if open || current {
                    for row in from.max(first)..to.min(end) {
                        out[row - first].push((i, i + needle.len(), current));
                    }
                }
                at = i + needle.len();
                k += 1;
            }
            index = to;
        }
        (out, placed)
    }
    /// How many matches of the query come before the one at byte `found` in the
    /// source line that starts at `start`; None when that cannot be told.
    fn occurrence(&self, page: &Page, start: u64, found: u64) -> Option<usize> {
        if self.listed()
            && self
                .matches
                .items
                .binary_search_by_key(&found, |m| m.offset)
                .is_ok()
        {
            let before = self.matches.items.partition_point(|m| m.offset < start);
            let at = self.matches.items.partition_point(|m| m.offset < found);
            return Some(at - before);
        }
        // Not listed: count in the line itself (UTF-8 text keeps its byte offsets).
        if page.encoding != feathermd::encoding::Encoding::Utf8 || page.invalid_utf8 {
            return None;
        }
        let line = line_of(&page.lines, start)?;
        let text = line.text.get(..(found - line.start) as usize)?;
        // Escaped punctuation (`T\_NAME`) matches too, as Find counts it.
        let text = match self.format() {
            Format::Markdown => markdown::unescape(text),
            _ => text.to_owned(),
        };
        Some(if self.match_case {
            text.matches(self.query.as_str()).count()
        } else {
            text.to_ascii_lowercase()
                .matches(self.query.to_ascii_lowercase().as_str())
                .count()
        })
    }
    /// Paints the layout rows from `self.top` down to `bottom`, starting at `y`.
    pub(super) unsafe fn paint_rows(&mut self, dc: HDC, p: Palette, mut y: i32, bottom: i32) {
        let left = self.left();
        let body_width = self.body_width();
        let raw = self.settings.raw;
        let preview = if raw { None } else { self.preview_range() };
        self.painted_pictures.clear();
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let soft = |a: u32, b: u32, percent: u32| mix(a, b, percent);
        let bar_color = soft(p.edge, p.muted, 35);
        let stripe = p.stripe();
        let table_pad = self.reading_fonts.scale(crate::layout::CELL_PAD_Y);
        let visible_end = Viewport::new(&layout.rows, bottom - y)
            .visible_end(self.top)
            .min(layout.rows.len());
        // Find's matches as marks on the text; the whole row only when the current
        // one cannot be placed (inside markup, a link or a legacy encoding).
        let (marks, placed) = self.find_marks(self.top, visible_end);
        self.find_placed = placed;
        // After a jump, a table wider than the column scrolls to the match's column.
        if !self.loading && std::mem::take(&mut self.reveal_found) {
            'reveal: for (slot, row_marks) in marks.iter().enumerate() {
                let row = &layout.rows[self.top + slot];
                let scrolls = !self.settings.wrap
                    || row
                        .table
                        .as_ref()
                        .and_then(|t| t.edges.last())
                        .is_some_and(|&edge| edge + 1 > body_width);
                for &(a, b, _) in row_marks.iter().filter(|m| m.2 && scrolls) {
                    for (first, run) in select::runs(row) {
                        let fonts = &mut self.reading_fonts;
                        let Some((from, to)) =
                            select::extent(run, first, (a, b), |r| pens(dc, fonts, r))
                        else {
                            continue;
                        };
                        // Extents are from the text origin; the column shows
                        // `horizontal .. horizontal + body_width` of it.
                        if from < self.horizontal || to > self.horizontal + body_width {
                            self.horizontal = (from - self.s(48)).max(0);
                        }
                        break 'reveal;
                    }
                }
            }
            self.horizontal = self.horizontal.min(self.horizontal_max());
        }
        // Code boxes: one rounded sheet behind each visible run of code rows. A box
        // that continues off screen extends past the clip so its corners stay hidden.
        if !raw {
            let end = Viewport::new(&layout.rows, bottom - y)
                .visible_end(self.top)
                .min(layout.rows.len());
            let mut top = y;
            let mut open: Option<i32> = None;
            for index in self.top..end {
                let row = &layout.rows[index];
                let next = top + row.height;
                if row.boxed {
                    if open.is_none() {
                        let above = index > 0 && layout.rows[index - 1].boxed && index == self.top;
                        open = Some(if above { top - self.s(12) } else { top });
                    }
                    let continues = layout.rows.get(index + 1).is_some_and(|r| r.boxed);
                    if !continues || index + 1 == end {
                        let close = if continues { next + self.s(12) } else { next };
                        if let Some(start) = open.take() {
                            chrome_round(
                                dc,
                                RECT {
                                    left: left + self.s(2),
                                    top: start,
                                    right: left + body_width,
                                    bottom: close,
                                },
                                p.code,
                                self.s(6),
                            );
                        }
                    }
                }
                top = next;
            }
        }
        for (index, row) in layout.rows.iter().enumerate().skip(self.top) {
            if y >= bottom {
                break;
            }
            let rect = RECT {
                left,
                top: y,
                right: left + body_width,
                bottom: y + row.height,
            };
            // Preview selection: the units of this row it covers; only the selected
            // characters are highlighted, in tables too.
            let found = !placed && self.found.is_some_and(|n| n >= row.start && n < row.end);
            let row_marks = marks
                .get(index.wrapping_sub(self.top))
                .map_or(&[][..], Vec::as_slice);
            let cover = preview
                .and_then(|(from, to)| select::covered(row, from, to))
                .filter(|_| !found);
            let shift = self.shift(row);
            if found {
                Self::fill(dc, &rect, p.selection);
            } else if raw {
                Self::fill(dc, &rect, p.code);
            }
            // In a table the highlight stays off the cell padding, so the rules
            // between rows stay visible.
            let band_rect = match &row.table {
                Some(_) => RECT {
                    top: rect.top + if row.first { table_pad } else { 0 },
                    bottom: rect.bottom - if row.last { table_pad } else { 0 },
                    ..rect
                },
                None => rect,
            };
            if let Some(table) = &row.table {
                let (x0, x1) = (
                    left + table.edges[0] - shift,
                    left + table.edges[table.edges.len() - 1] - shift,
                );
                if !found && (table.header || table.stripe) {
                    Self::fill(
                        dc,
                        &RECT {
                            left: x0,
                            right: x1,
                            ..rect
                        },
                        if table.header { p.code } else { stripe },
                    );
                }
                let line = |top: i32| RECT {
                    left: x0,
                    top,
                    right: x1 + 1,
                    bottom: top + 1,
                };
                if table.first {
                    Self::fill(dc, &line(y), p.edge);
                }
                if table.close {
                    Self::fill(dc, &line(y + row.height - 1), p.edge);
                }
                for &edge in table.edges.iter() {
                    let x = left + edge - shift;
                    Self::fill(
                        dc,
                        &RECT {
                            left: x,
                            top: y,
                            right: x + 1,
                            bottom: y + row.height,
                        },
                        p.edge,
                    );
                }
            }
            // Source mode: highlight the selected characters only.
            if raw
                && let (Some(sel), Some(page), Some(run)) =
                    (self.text_selection, &self.page, row.runs.first())
                && let Some((a, b, eol)) = selected_units(&page.lines, sel, row)
            {
                let old = SelectObject(dc, self.reading_fonts.get(run.font));
                let pen = |n: usize| {
                    let mut size = SIZE::default();
                    if n > 0 {
                        GetTextExtentPoint32W(dc, run.text.as_ptr(), n as i32, &mut size);
                    }
                    size.cx
                };
                let x = left + run.x - shift;
                let right = x + pen(b) + if eol { self.s(6) } else { 0 };
                Self::fill(
                    dc,
                    &RECT {
                        left: x + pen(a),
                        right,
                        ..rect
                    },
                    p.text_highlight(),
                );
                SelectObject(dc, old);
            }
            if let Some(bar) = row.bar {
                let x = left + bar - shift;
                Self::fill(
                    dc,
                    &RECT {
                        left: x,
                        right: x + self.s(3),
                        ..rect
                    },
                    bar_color,
                );
            }
            if row.kind == markdown::Kind::Rule {
                let middle = y + row.height / 2;
                Self::fill(
                    dc,
                    &RECT {
                        left: left + row.indent,
                        top: middle - self.s(1),
                        right: left + body_width,
                        bottom: middle - self.s(1) + self.s(2).max(1),
                    },
                    p.edge,
                );
            }
            self.hits.push(HitBox {
                rect,
                action: Hit::Row(index),
            });
            let quoted = row.kind == markdown::Kind::Quote;
            let muted_heading = row.kind == markdown::Kind::Heading(6);
            for (first, run) in select::runs(row) {
                let x = left + run.x - shift;
                // Selected characters: one band per run, over chips, under glyphs.
                let band = cover.and_then(|range| {
                    let fonts = &mut self.reading_fonts;
                    select::extent(run, first, range, |r| pens(dc, fonts, r))
                });
                if let Some(picture) = &run.picture {
                    let target = RECT {
                        left: x,
                        top: y + picture.top,
                        right: x + run.width,
                        bottom: y + picture.top + picture.height,
                    };
                    if self.pictures.draw(dc, &picture.key, target) {
                        self.painted_pictures.push(target);
                    }
                    if band.is_some() {
                        // A selected image gets an accent frame.
                        let w = self.s(2).max(2);
                        for edge in [
                            RECT {
                                bottom: target.top + w,
                                ..target
                            },
                            RECT {
                                top: target.bottom - w,
                                ..target
                            },
                            RECT {
                                right: target.left + w,
                                ..target
                            },
                            RECT {
                                left: target.right - w,
                                ..target
                            },
                        ] {
                            Self::fill(dc, &edge, p.accent);
                        }
                    }
                    if let Some(url) = &run.link {
                        self.hits.push(HitBox {
                            rect: target,
                            action: Hit::Link(url.clone()),
                        });
                    }
                    continue;
                }
                let text_y = y + row.baseline - run.ascent - run.rise;
                let plain = !found;
                if (run.code || run.kbd) && plain {
                    let chip = RECT {
                        left: left + run.chip.0 - shift,
                        top: y + row.baseline - run.ascent - self.s(2),
                        right: left + run.chip.1 - shift,
                        bottom: y + row.baseline + run.descent + self.s(2),
                    };
                    if run.kbd {
                        chrome_round(dc, chip, p.edge, self.s(4));
                        chrome_round(
                            dc,
                            RECT {
                                left: chip.left + 1,
                                top: chip.top + 1,
                                right: chip.right - 1,
                                bottom: chip.bottom - self.s(2).max(2),
                            },
                            p.page,
                            self.s(3),
                        );
                    } else {
                        chrome_round(dc, chip, p.code, self.s(4));
                    }
                } else if run.mark && plain {
                    let wash = if p.is_dark() {
                        mix(p.page, rgb(200, 160, 40), 32)
                    } else {
                        mix(p.page, rgb(255, 214, 64), 55)
                    };
                    Self::fill(
                        dc,
                        &RECT {
                            left: x,
                            top: y + row.baseline - run.ascent,
                            right: x + run.width,
                            bottom: y + row.baseline + run.descent,
                        },
                        wash,
                    );
                }
                for &(a, b, current) in row_marks {
                    let fonts = &mut self.reading_fonts;
                    if let Some((from, to)) =
                        select::extent(run, first, (a, b), |r| pens(dc, fonts, r))
                    {
                        Self::fill(
                            dc,
                            &RECT {
                                left: left + from - shift,
                                top: y + row.baseline - run.ascent - self.s(1),
                                right: left + to - shift,
                                bottom: y + row.baseline + run.descent + self.s(1),
                            },
                            p.find_mark(current),
                        );
                    }
                }
                if let Some((a, b)) = band {
                    Self::fill(
                        dc,
                        &RECT {
                            left: left + a - shift,
                            right: left + b - shift,
                            ..band_rect
                        },
                        p.text_highlight(),
                    );
                }
                if run.disclosure {
                    let side = self.s(14);
                    let middle = y + row.baseline - (run.ascent - run.descent) / 2;
                    smooth::icon(
                        dc,
                        RECT {
                            left: x,
                            top: middle - side / 2,
                            right: x + side,
                            bottom: middle - side / 2 + side,
                        },
                        THEME_PICKER,
                        p.muted,
                        self.fonts.dpi as f32 / 96.0 * 1.4,
                    );
                    continue;
                }
                if let Some(done) = run.check {
                    let side = (self.s(16) * 7 + 4) / 8;
                    let middle = y + row.baseline - (run.ascent - run.descent) / 2;
                    let r = RECT {
                        left: x,
                        top: middle - side / 2,
                        right: x + side,
                        bottom: middle - side / 2 + side,
                    };
                    if done {
                        chrome_round(dc, r, p.accent, self.s(3));
                        smooth::icon(
                            dc,
                            r,
                            smooth::CHECK,
                            p.page,
                            self.fonts.dpi as f32 / 96.0 * 0.78,
                        );
                    } else {
                        chrome_round(dc, r, p.muted, self.s(3));
                        chrome_round(
                            dc,
                            RECT {
                                left: r.left + self.s(1).max(1),
                                top: r.top + self.s(1).max(1),
                                right: r.right - self.s(1).max(1),
                                bottom: r.bottom - self.s(1).max(1),
                            },
                            p.page,
                            self.s(2),
                        );
                    }
                    continue;
                }
                let color = if run.link.is_some() || run.load_image.is_some() {
                    p.accent
                } else if let Some(token) = run.token {
                    p.token(token)
                } else if run.muted || quoted || muted_heading || row.kind == markdown::Kind::Fence
                {
                    p.muted
                } else {
                    p.fg
                };
                let old = SelectObject(dc, self.reading_fonts.get(run.font));
                SetTextColor(dc, color);
                TextOutW(dc, x, text_y, run.text.as_ptr(), run.text.len() as i32);
                SelectObject(dc, old);
                if let Some(url) = run.load_image.as_ref().or(run.link.as_ref()) {
                    self.hits.push(HitBox {
                        rect: RECT {
                            left: x.max(left),
                            top: y,
                            right: (x + run.width).min(left + body_width),
                            bottom: y + row.height,
                        },
                        action: if run.load_image.is_some() {
                            Hit::LoadImage(url.clone())
                        } else {
                            Hit::Link(url.clone())
                        },
                    });
                }
            }
            if matches!(row.kind, markdown::Kind::Heading(1 | 2)) && row.last {
                Self::fill(
                    dc,
                    &RECT {
                        left: left + row.indent,
                        top: y + row.height - 1,
                        right: left + body_width,
                        bottom: y + row.height,
                    },
                    p.edge,
                );
            }
            y += row.height;
        }
    }
}
