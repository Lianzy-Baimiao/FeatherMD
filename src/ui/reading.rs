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
}

impl App {
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
        let stripe = soft(p.page, p.code, 55);
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
            // Preview selection: the units of this row it covers. Table rows are
            // selected whole; other rows highlight only the selected characters.
            let cover = preview.and_then(|(from, to)| select::covered(row, from, to));
            let selected = cover.is_some() && row.table.is_some();
            let found = self.found.is_some_and(|n| n >= row.start && n < row.end);
            let shift = self.shift(row);
            if selected || found {
                Self::fill(dc, &rect, p.selection);
            } else if raw {
                Self::fill(dc, &rect, p.code);
            }
            let cover = cover.filter(|_| row.table.is_none() && !found);
            if let Some(table) = &row.table {
                let (x0, x1) = (
                    left + table.edges[0] - shift,
                    left + table.edges[table.edges.len() - 1] - shift,
                );
                if !selected && !found && (table.header || table.stripe) {
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
            let mut unit = row.unit;
            for run in &row.runs {
                let x = left + run.x - shift;
                let first = unit;
                unit += run.text.len();
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
                let plain = !selected && !found;
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
                if let Some((a, b)) = band {
                    Self::fill(
                        dc,
                        &RECT {
                            left: left + a - shift,
                            right: left + b - shift,
                            ..rect
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
                let color = if run.link.is_some() {
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
                if let Some(url) = &run.link {
                    self.hits.push(HitBox {
                        rect: RECT {
                            left: x.max(left),
                            top: y,
                            right: (x + run.width).min(left + body_width),
                            bottom: y + row.height,
                        },
                        action: Hit::Link(url.clone()),
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
