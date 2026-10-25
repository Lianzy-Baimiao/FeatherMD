//! Custom window chrome and quiet reading controls.
use super::*;

/// The outline's count line for what it lists.
fn outline_count(kind: OutlineKind, total: usize) -> String {
    let (none, one, many) = match kind {
        OutlineKind::Headings => (key!("文档暂无标题"), key!("1 个标题"), key!("{} 个标题")),
        OutlineKind::Chapters => (key!("未识别到章节"), key!("1 个章节"), key!("{} 个章节")),
        OutlineKind::Time => (
            key!("没有时间戳，也没有错误行"),
            key!("1 个时间段"),
            key!("{} 个时间段"),
        ),
        OutlineKind::Problems => (
            key!("没有时间戳，也没有错误行"),
            key!("1 处错误"),
            key!("{} 处错误"),
        ),
    };
    match total {
        0 => tr(none).to_string(),
        1 => tr(one).to_string(),
        _ => trf(many, &[&total]),
    }
}

/// Title-bar glyphs shrink with the header (34 px, formerly 40 px).
const HEADER_ICON: f32 = 0.86;
/// Caption buttons keep the familiar Windows width; only their height shrank.
const CAPTION_BUTTON: i32 = 44;
/// Open, Find, Appearance and More: 32 px buttons 4 px apart, then an 8 px gap
/// before the caption buttons, in logical pixels.
const ACTIONS: [usize; 4] = [OPEN, FIND, THEME_PICKER, SETTINGS];
const ACTION_PITCH: i32 = 36;
const ACTIONS_WIDTH: i32 = ACTION_PITCH * ACTIONS.len() as i32 + 4;
/// Close-button red of the Windows title bar, and the paler red while pressed.
const CLOSE_HOVER: u32 = rgb(196, 43, 28);
const CLOSE_PRESSED: u32 = rgb(201, 72, 60);

/// `percent` of the way from colour `a` to colour `b`, per channel.
pub(super) fn mix(a: u32, b: u32, percent: u32) -> u32 {
    let channel = |shift: u32| {
        let (a, b) = ((a >> shift) & 255, (b >> shift) & 255);
        ((a * (100 - percent) + b * percent + 50) / 100) << shift
    };
    channel(0) | channel(8) | channel(16)
}

/// Header appearance picker: three basic choices plus the full skin gallery.
pub(super) const PICKER: [(usize, &str); 4] = [
    (THEME_LIGHT, key!("浅色")),
    (THEME_DARK, key!("深色")),
    (THEME_SYSTEM, key!("跟随系统")),
    (THEME_MORE, key!("更多皮肤…")),
];
pub(super) fn picker_index(theme: Theme) -> usize {
    match theme {
        Theme::Light => 0,
        Theme::Dark => 1,
        Theme::System => 2,
        _ => 3,
    }
}

#[derive(Clone, Copy)]
enum ChromeTone {
    Quiet,
    Primary,
}

fn chrome_rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

pub(super) unsafe fn chrome_round(dc: HDC, rect: RECT, color: u32, radius: i32) {
    smooth::round(dc, rect, color, radius);
}

impl App {
    #[allow(
        clippy::too_many_arguments,
        reason = "Control geometry, action, palette and emphasis are independent"
    )]
    unsafe fn chrome_button(
        &mut self,
        dc: HDC,
        text: &str,
        rect: RECT,
        id: usize,
        tone: ChromeTone,
        p: Palette,
    ) {
        let hover = self.hover == Some(id);
        let pressed = self.pressed == Some(Hit::Command(id));
        let background = match tone {
            ChromeTone::Primary => Some(if pressed {
                p.muted
            } else if hover {
                p.fg
            } else {
                p.accent
            }),
            ChromeTone::Quiet if pressed => Some(p.selection),
            ChromeTone::Quiet if hover => Some(p.code),
            ChromeTone::Quiet => None,
        };
        if let Some(color) = background {
            // Radius follows the control height so small header pills stay round.
            let radius = if matches!(tone, ChromeTone::Primary) {
                self.s(8)
            } else {
                ((rect.bottom - rect.top) / 4).clamp(self.s(4), self.s(6))
            };
            chrome_round(dc, rect, color, radius);
        }
        let color = match tone {
            ChromeTone::Primary => p.page,
            ChromeTone::Quiet => p.fg,
        };
        self.label(
            dc,
            text,
            rect,
            12,
            matches!(tone, ChromeTone::Primary),
            color,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        self.hits.push(HitBox {
            rect,
            action: Hit::Command(id),
        });
    }

    unsafe fn chrome_window_icon(&self, dc: HDC, rect: RECT, id: usize, color: u32) {
        let id = if id == MAXIMIZE && IsZoomed(self.hwnd) != 0 {
            smooth::RESTORE
        } else {
            id
        };
        smooth::icon(dc, rect, id, color, self.fonts.dpi as f32 / 96.0);
    }

    /// Title-bar and footer glyph: `size` times the DPI scale, drawn with the line
    /// weight every header glyph shares.
    unsafe fn header_glyph(&self, dc: HDC, rect: RECT, id: usize, color: u32, size: f32) {
        let dpi = self.fonts.dpi as f32 / 96.0;
        smooth::icon_weighted(dc, rect, id, color, dpi * size, smooth::header_weight(dpi));
    }

    /// Hover and pressed fills of the quiet title-bar and footer buttons: a light
    /// veil of the text colour over the panel, as Windows draws its own.
    fn veil(&self, p: Palette, pressed: bool) -> u32 {
        let dark = self.active_theme().is_dark();
        let percent = match (dark, pressed) {
            (false, false) => 7,
            (false, true) => 12,
            (true, false) => 9,
            (true, true) => 14,
        };
        mix(p.panel, p.fg, percent)
    }

    /// Caption buttons fill their whole column on hover, Close in Windows red.
    unsafe fn caption_button(&mut self, dc: HDC, rect: RECT, id: usize, p: Palette) {
        let hover = self.hover == Some(id);
        let pressed = self.pressed == Some(Hit::Command(id));
        let close = id == EXIT;
        let fill = match (close, hover || pressed) {
            (_, false) => None,
            (true, true) => Some(if pressed { CLOSE_PRESSED } else { CLOSE_HOVER }),
            (false, true) => Some(self.veil(p, pressed)),
        };
        if let Some(color) = fill {
            Self::fill(dc, &rect, color);
        }
        self.hits.push(HitBox {
            rect,
            action: Hit::Command(id),
        });
        let glyph = if id == MAXIMIZE && IsZoomed(self.hwnd) != 0 {
            smooth::RESTORE
        } else {
            id
        };
        let color = match (close && (hover || pressed), hover) {
            (true, _) => rgb(255, 255, 255),
            (false, true) => p.fg,
            (false, false) => p.muted,
        };
        self.header_glyph(dc, rect, glyph, color, 1.0);
    }

    /// One of the quiet title-bar actions: a rounded hover pill and its glyph.
    /// `open` keeps the pressed look while the action's panel is showing.
    unsafe fn header_button(
        &mut self,
        dc: HDC,
        rect: RECT,
        id: usize,
        glyph: usize,
        open: bool,
        p: Palette,
    ) {
        let hover = self.hover == Some(id);
        let pressed = self.pressed == Some(Hit::Command(id)) || open;
        if hover || pressed {
            chrome_round(dc, rect, self.veil(p, pressed), self.s(6));
        }
        self.hits.push(HitBox {
            rect,
            action: Hit::Command(id),
        });
        let color = if hover || pressed { p.fg } else { p.muted };
        self.header_glyph(dc, rect, glyph, color, HEADER_ICON);
    }

    pub(super) fn header_left(&self) -> i32 {
        self.width - self.s(CAPTION_BUTTON * 3) - self.s(ACTIONS_WIDTH) - 1
    }

    unsafe fn feather_mark(&self, dc: HDC, rect: RECT, color: u32) {
        smooth::icon(
            dc,
            rect,
            smooth::FEATHER,
            color,
            self.fonts.dpi as f32 / 96.0,
        );
    }

    unsafe fn chrome_glyph(&self, dc: HDC, rect: RECT, id: usize, color: u32) {
        smooth::icon(dc, rect, id, color, self.fonts.dpi as f32 / 96.0);
    }

    pub(super) fn theme_menu_rect(&self) -> RECT {
        // Right-aligned with the appearance button, the third action.
        let right = self.header_left() + self.s(2 * ACTION_PITCH + 32);
        // Stay below the native search EDIT HWND; parent painting cannot cover a child window.
        let gap = if self.edit_mode.is_some() { 56 } else { 6 };
        let top = self.s(HEADER_HEIGHT + gap);
        chrome_rect(right - self.s(224), top, right, top + self.s(250))
    }

    /// Tooltip of the title-bar or footer control under the pointer.
    pub(super) fn header_tip(&self) -> Option<String> {
        let bars = |y: i32| y < self.s(HEADER_HEIGHT) || y >= self.height - self.s(30);
        if self.theme_menu.is_some() || self.mouse.is_none_or(|(_, y)| !bars(y)) {
            return None;
        }
        Some(
            match self.hover? {
                PREVIEW => tr("预览 · 阅读排版"),
                SOURCE => tr("源码 · Ctrl+R"),
                OPEN => tr("打开文档 · Ctrl+O"),
                FIND => tr("查找 · Ctrl+F"),
                SETTINGS => {
                    return Some(match self.update_available() {
                        Some(tag) => trf("更多选项 · 发现新版本 {}", &[&tag]),
                        None => tr("更多选项").to_string(),
                    });
                }
                THEME_PICKER => {
                    return Some(trf("外观：{} · Ctrl+T", &[&skin_name(self.settings.theme)]));
                }
                MINIMIZE => tr("最小化"),
                MAXIMIZE => tr("最大化 / 还原"),
                EXIT => tr("关闭"),
                _ => return None,
            }
            .to_string(),
        )
    }

    pub(super) unsafe fn paint_header_overlay(&mut self, dc: HDC, p: Palette) {
        if let Some(focus) = self.theme_menu {
            let r = self.theme_menu_rect();
            chrome_round(
                dc,
                chrome_rect(
                    r.left - self.s(2),
                    r.top + self.s(2),
                    r.right + self.s(2),
                    r.bottom + self.s(4),
                ),
                p.code,
                self.s(12),
            );
            chrome_round(dc, r, p.edge, self.s(10));
            chrome_round(
                dc,
                chrome_rect(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
                p.bg,
                self.s(10),
            );
            // Consume the panel's whitespace so clicks cannot activate content below it.
            self.hits.push(HitBox {
                rect: r,
                action: Hit::Command(THEME_PANEL),
            });
            self.label(
                dc,
                tr("外观"),
                chrome_rect(
                    r.left + self.s(16),
                    r.top + self.s(7),
                    r.right - self.s(12),
                    r.top + self.s(33),
                ),
                12,
                true,
                p.fg,
                DT_SINGLELINE | DT_VCENTER,
            );
            for (i, (id, text)) in PICKER.into_iter().enumerate() {
                // A divider separates the basic choices from the full skin gallery.
                let gallery = id == THEME_MORE;
                let y = r.top + self.s(38 + i as i32 * 42 + if gallery { 9 } else { 0 });
                if gallery {
                    Self::fill(
                        dc,
                        &chrome_rect(
                            r.left + self.s(16),
                            y - self.s(6),
                            r.right - self.s(16),
                            y - self.s(6) + 1,
                        ),
                        p.edge,
                    );
                }
                let row = chrome_rect(r.left + self.s(7), y, r.right - self.s(7), y + self.s(38));
                let selected = i == picker_index(self.settings.theme);
                // The check mark and accent text mark the choice; rows only veil.
                if i == focus || self.hover == Some(id) {
                    chrome_round(dc, row, mix(p.bg, p.fg, 7), self.s(6));
                }
                let color = if selected { p.accent } else { p.fg };
                self.chrome_glyph(
                    dc,
                    chrome_rect(
                        row.left + self.s(9),
                        row.top,
                        row.left + self.s(37),
                        row.bottom,
                    ),
                    if gallery { smooth::PALETTE } else { id },
                    color,
                );
                self.label(
                    dc,
                    tr(text),
                    chrome_rect(
                        row.left + self.s(47),
                        row.top,
                        row.right - self.s(28),
                        row.bottom,
                    ),
                    12,
                    selected,
                    color,
                    DT_SINGLELINE | DT_VCENTER,
                );
                if selected && gallery {
                    // Name the active gallery skin beside its check mark.
                    self.label(
                        dc,
                        skin_name(self.settings.theme),
                        chrome_rect(
                            row.left + self.s(118),
                            row.top,
                            row.right - self.s(30),
                            row.bottom,
                        ),
                        11,
                        false,
                        p.muted,
                        DT_SINGLELINE | DT_VCENTER | DT_RIGHT,
                    );
                }
                if selected {
                    smooth::icon(
                        dc,
                        chrome_rect(
                            row.right - self.s(27),
                            y,
                            row.right - self.s(7),
                            y + self.s(38),
                        ),
                        smooth::CHECK,
                        p.accent,
                        self.fonts.dpi as f32 / 96.0,
                    );
                }
                self.hits.push(HitBox {
                    rect: row,
                    action: Hit::Command(id),
                });
            }
            self.label(
                dc,
                tr("跟随系统将自动切换明暗"),
                chrome_rect(
                    r.left + self.s(16),
                    r.bottom - self.s(29),
                    r.right - self.s(12),
                    r.bottom - self.s(7),
                ),
                10,
                false,
                p.muted,
                DT_SINGLELINE | DT_VCENTER,
            );
        } else if self.tooltip_visible
            && self.pressed.is_none()
            && let Some(text) = self.header_tip()
            && let Some(hit) = self
                .hits
                .iter()
                .find(|h| h.action == Hit::Command(self.hover.unwrap_or(0)))
        {
            let width = self.s((text
                .chars()
                .map(|c| if c.is_ascii() { 6 } else { 11 })
                .sum::<i32>()
                + 26)
                .max(80));
            let left = ((hit.rect.left + hit.rect.right - width) / 2)
                .clamp(self.s(8), self.width - width - self.s(8));
            // Below the title bar, or above the footer for its controls.
            let top = if hit.rect.top >= self.height - self.s(30) {
                self.height - self.s(30 + 41)
            } else {
                self.s(HEADER_HEIGHT + 7)
            };
            let r = chrome_rect(left, top, left + width, top + self.s(34));
            chrome_round(dc, r, p.fg, self.s(6));
            self.label(
                dc,
                &text,
                r,
                11,
                false,
                p.bg,
                DT_SINGLELINE | DT_CENTER | DT_VCENTER,
            );
        }
    }

    pub(super) unsafe fn paint_header(&mut self, dc: HDC, p: Palette) {
        let bottom = self.s(HEADER_HEIGHT);
        let middle = HEADER_HEIGHT / 2;
        Self::fill(dc, &chrome_rect(1, 1, self.width - 1, bottom), p.panel);
        let left = self.header_left();
        let controls_left = self.width - self.s(CAPTION_BUTTON * 3) - 1;
        // Only the feather mark and the document name: the title bar stays quiet.
        self.feather_mark(
            dc,
            chrome_rect(
                self.s(14),
                self.s(middle - 10),
                self.s(34),
                self.s(middle + 10),
            ),
            p.accent,
        );
        if !self.document_name.is_empty() {
            let name = self.document_name.clone();
            self.label(
                dc,
                &name,
                chrome_rect(self.s(44), 1, left - self.s(16), bottom),
                12,
                false,
                p.fg,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        // Four equal actions; Appearance keeps its pressed look while its panel is open.
        for (index, id) in ACTIONS.into_iter().enumerate() {
            let x = left + self.s(index as i32 * ACTION_PITCH);
            let rect = chrome_rect(x, self.s(middle - 13), x + self.s(32), self.s(middle + 13));
            let glyph = if id == THEME_PICKER {
                smooth::CONTRAST
            } else {
                id
            };
            let open = id == THEME_PICKER && self.theme_menu.is_some();
            self.header_button(dc, rect, id, glyph, open, p);
            if id == SETTINGS && self.update_available().is_some() {
                // An accent dot over the last of the three dots: a newer release
                // is available (More → 发现新版本).
                let (x, y) = (rect.right - self.s(9), rect.top + self.s(7));
                let r = self.s(3);
                let ring = if self.hover == Some(id) {
                    self.veil(p, false)
                } else {
                    p.panel
                };
                chrome_round(
                    dc,
                    chrome_rect(x - r - 1, y - r - 1, x + r + 1, y + r + 1),
                    ring,
                    r + 1,
                );
                chrome_round(dc, chrome_rect(x - r, y - r, x + r, y + r), p.accent, r);
            }
        }
        for (index, id) in [MINIMIZE, MAXIMIZE, EXIT].into_iter().enumerate() {
            let x = controls_left + self.s(index as i32 * CAPTION_BUTTON);
            let rect = chrome_rect(x, 1, x + self.s(CAPTION_BUTTON), bottom);
            self.caption_button(dc, rect, id, p);
        }
        Self::fill(
            dc,
            &chrome_rect(1, bottom, self.width - 1, bottom + 1),
            p.edge,
        );

        if self.edit_mode.is_some() {
            let search_bottom = self.s(HEADER_HEIGHT + 53);
            Self::fill(
                dc,
                &chrome_rect(
                    self.sidebar_width().max(1),
                    bottom + 1,
                    self.width - 1,
                    search_bottom,
                ),
                p.panel,
            );
            self.chrome_button(
                dc,
                if self.edit_mode == Some(GOTO) {
                    tr("跳转 %")
                } else {
                    tr("下一个")
                },
                chrome_rect(
                    self.width - self.s(146),
                    self.s(HEADER_HEIGHT + 12),
                    self.width - self.s(66),
                    self.s(HEADER_HEIGHT + 44),
                ),
                NEXT,
                ChromeTone::Quiet,
                p,
            );
            let close = chrome_rect(
                self.width - self.s(56),
                self.s(HEADER_HEIGHT + 12),
                self.width - self.s(24),
                self.s(HEADER_HEIGHT + 44),
            );
            self.chrome_button(dc, "", close, 0, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, close, 0, p.muted);
            Self::fill(
                dc,
                &chrome_rect(
                    self.sidebar_width().max(1),
                    search_bottom,
                    self.width - 1,
                    search_bottom + 1,
                ),
                p.edge,
            );
        }
    }

    pub(super) unsafe fn paint_outline(&mut self, dc: HDC, p: Palette) {
        if self.document.is_none() {
            return;
        }
        let width = self.sidebar_width();
        if width == 0 {
            let expand = chrome_rect(
                self.s(6),
                self.s(HEADER_HEIGHT + 14),
                self.s(38),
                self.s(HEADER_HEIGHT + 46),
            );
            chrome_round(dc, expand, p.panel, self.s(5));
            self.chrome_button(dc, "", expand, OUTLINE, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, expand, OUTLINE, p.muted);
            return;
        }
        let footer = self.height - self.s(30);
        let panel_top = self.s(HEADER_HEIGHT) + 1;
        Self::fill(dc, &chrome_rect(1, panel_top, width, footer), p.panel);
        Self::fill(
            dc,
            &chrome_rect(width - 1, panel_top, width, footer),
            p.edge,
        );
        self.label(
            dc,
            tr("目录"),
            chrome_rect(
                self.s(18),
                self.s(HEADER_HEIGHT + 14),
                width - self.s(52),
                self.s(HEADER_HEIGHT + 42),
            ),
            12,
            true,
            p.fg,
            DT_SINGLELINE | DT_VCENTER,
        );
        let collapse = chrome_rect(
            width - self.s(42),
            self.s(HEADER_HEIGHT + 14),
            width - self.s(14),
            self.s(HEADER_HEIGHT + 42),
        );
        self.chrome_button(dc, "", collapse, OUTLINE, ChromeTone::Quiet, p);
        self.chrome_window_icon(dc, collapse, OUTLINE, p.muted);
        let visible = self.outline_visible_rows();
        let first = self.outline_top;
        let active = self.outline_active();
        // Copy only visible labels so GDI font lookup can borrow App mutably.
        let (entries, total, limited) = if let Some(outline) = &self.outline {
            let entries: Vec<_> = outline
                .headings
                .iter()
                .enumerate()
                .skip(first)
                .take(visible)
                .map(|(index, heading)| {
                    // A log time entry says how many error and warning lines it holds.
                    let mut text = heading.text.clone();
                    if heading.errors > 0 && outline.kind == OutlineKind::Time {
                        text.push_str(" · ");
                        text.push_str(&trf("{} 错误", &[&heading.errors]));
                    }
                    if heading.warnings > 0 {
                        text.push_str(" · ");
                        text.push_str(&trf("{} 警告", &[&heading.warnings]));
                    }
                    (index, heading.level, text, heading.errors > 0)
                })
                .collect();
            (entries, outline.headings.len(), outline.limited)
        } else {
            (Vec::new(), 0, false)
        };
        let bottom = self.height - self.s(62);
        let saved = SaveDC(dc);
        IntersectClipRect(dc, 1, self.s(OUTLINE_ROWS_TOP), width - 1, bottom);
        let error_tone = p.token(feathermd::highlight::Token::Deleted);
        for (row, (index, level, text, errors)) in entries.into_iter().enumerate() {
            let top = self.s(OUTLINE_ROWS_TOP + row as i32 * 30);
            if top >= bottom {
                break;
            }
            let rect = chrome_rect(
                self.s(6),
                top,
                width - self.s(10),
                (top + self.s(30)).min(bottom),
            );
            let selected = active == Some(index);
            if selected || self.pressed == Some(Hit::Heading(index)) {
                chrome_round(dc, rect, p.selection, self.s(4));
            }
            if selected {
                chrome_round(
                    dc,
                    chrome_rect(self.s(7), top + self.s(8), self.s(9), top + self.s(22)),
                    p.accent,
                    self.s(1),
                );
            }
            self.label(
                dc,
                &text,
                chrome_rect(
                    self.s(12 + i32::from(level.saturating_sub(1).min(5)) * 12),
                    top,
                    width - self.s(16),
                    top + self.s(30),
                ),
                12,
                selected || level == 1,
                if selected {
                    p.accent
                } else if errors {
                    error_tone
                } else if level <= 2 {
                    p.fg
                } else {
                    p.muted
                },
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
            self.hits.push(HitBox {
                rect,
                action: Hit::Heading(index),
            });
        }
        RestoreDC(dc, saved);

        let format = self.format();
        let kind = self.outline.as_ref().map(|o| o.kind);
        let status = if let Some(error) = &self.outline_error {
            trf("目录读取失败：{}", &[error])
        } else if limited {
            tr("目录已达显示上限").to_string()
        } else if self.outline_loading {
            match format {
                Format::Markdown => tr("正在提取标题…").to_string(),
                Format::Text => tr("正在识别章节…").to_string(),
                // A large log takes a while: say how far the index has come.
                Format::Log => match self.outline_percent.filter(|_| {
                    self.page
                        .as_ref()
                        .is_some_and(|p| p.size > 32 * 1024 * 1024)
                }) {
                    Some(percent) => trf("正在建立时间索引… {}%", &[&percent]),
                    None => tr("正在建立时间索引…").to_string(),
                },
            }
        } else if self.document.is_none() {
            tr("打开文档后显示目录").to_string()
        } else {
            outline_count(kind.unwrap_or_default(), total)
        };
        let status_top = if total == 0 {
            self.s(OUTLINE_ROWS_TOP + 8)
        } else {
            footer - self.s(27)
        };
        self.label(
            dc,
            &status,
            chrome_rect(
                self.s(18),
                status_top,
                width - self.s(16),
                status_top + self.s(24),
            ),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        if total > visible && visible > 0 {
            let track_height = (bottom - self.s(OUTLINE_ROWS_TOP)).max(1);
            let thumb_height = ((track_height as i64 * visible as i64 / total as i64) as i32)
                .max(self.s(18))
                .min(track_height);
            let offset = ((track_height - thumb_height) as i64 * first.min(total - visible) as i64
                / (total - visible) as i64) as i32;
            chrome_round(
                dc,
                chrome_rect(
                    width - self.s(5),
                    self.s(OUTLINE_ROWS_TOP) + offset,
                    width - self.s(3),
                    self.s(OUTLINE_ROWS_TOP) + offset + thumb_height,
                ),
                p.edge,
                self.s(1),
            );
        }
    }

    pub(super) unsafe fn paint_welcome(&mut self, dc: HDC, p: Palette) {
        let center = self.width / 2;
        let room = self.content_bottom() - self.content_top();
        let top = self.content_top() + ((room - self.s(322)) / 2).max(self.s(8));
        let mark = chrome_rect(
            center - self.s(32),
            top,
            center + self.s(32),
            top + self.s(64),
        );
        chrome_round(dc, mark, p.selection, self.s(20));
        self.feather_mark(
            dc,
            chrome_rect(
                mark.left + self.s(15),
                mark.top + self.s(14),
                mark.right - self.s(14),
                mark.bottom - self.s(15),
            ),
            p.accent,
        );
        self.label(
            dc,
            "F E A T H E R M D",
            chrome_rect(
                center - self.s(220),
                top + self.s(79),
                center + self.s(220),
                top + self.s(102),
            ),
            10,
            true,
            p.muted,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        self.label(
            dc,
            tr("把注意力，留给文字。"),
            chrome_rect(
                center - self.s(260),
                top + self.s(113),
                center + self.s(260),
                top + self.s(156),
            ),
            28,
            true,
            p.fg,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        self.label(
            dc,
            tr("一个轻量、离线的 Markdown 阅读空间"),
            chrome_rect(
                center - self.s(240),
                top + self.s(168),
                center + self.s(240),
                top + self.s(196),
            ),
            13,
            false,
            p.muted,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        // 208 logical px, wider when a translation needs it (both labels, so the
        // button keeps its size while a document opens).
        let text = self
            .text_width(dc, tr("打开文档"), 13, true)
            .max(self.text_width(dc, tr("正在打开…"), 13, true));
        let half = (self.s(52 + 67 + 4) + text).max(self.s(208)) / 2;
        let action = chrome_rect(
            center - half,
            top + self.s(222),
            center + half,
            top + self.s(268),
        );
        self.chrome_button(dc, "", action, OPEN, ChromeTone::Primary, p);
        let foreground = p.page;
        self.chrome_glyph(
            dc,
            chrome_rect(
                action.left + self.s(15),
                action.top,
                action.left + self.s(43),
                action.bottom,
            ),
            OPEN,
            foreground,
        );
        self.label(
            dc,
            if self.loading {
                tr("正在打开…")
            } else {
                tr("打开文档")
            },
            chrome_rect(
                action.left + self.s(52),
                action.top,
                action.right - self.s(67),
                action.bottom,
            ),
            13,
            true,
            foreground,
            DT_SINGLELINE | DT_VCENTER,
        );
        self.label(
            dc,
            "Ctrl O",
            chrome_rect(
                action.right - self.s(61),
                action.top,
                action.right - self.s(12),
                action.bottom,
            ),
            10,
            false,
            foreground,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        self.label(
            dc,
            tr("或拖入 .md / .txt / .log 文件"),
            chrome_rect(
                center - self.s(230),
                top + self.s(286),
                center + self.s(230),
                top + self.s(312),
            ),
            11,
            false,
            p.muted,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
    }

    pub(super) unsafe fn paint_footer(&mut self, dc: HDC, p: Palette) {
        let top = self.height - self.s(30);
        let format = self.format();
        Self::fill(
            dc,
            &chrome_rect(1, top, self.width - 1, self.height - 1),
            p.panel,
        );
        Self::fill(dc, &chrome_rect(1, top, self.width - 1, top + 1), p.edge);
        self.paint_mode_switch(dc, top, p);
        let mut metadata = tr("只读  ·  离线").to_string();
        let mut status = if self.status == tr("就绪")
            || self.status == tr("只读 · 离线阅读")
            || self.status.starts_with(tr("离线阅读"))
        {
            String::new()
        } else {
            self.status.clone()
        };
        if let Some(page) = &self.page {
            metadata = format!(
                "{}  ·  {}{}",
                human_size(page.size),
                page.encoding.name(),
                if page.invalid_utf8 { "*" } else { "" }
            );
            match format {
                Format::Markdown => {}
                Format::Text => metadata.push_str(&format!("  ·  {}", tr("文本"))),
                Format::Log => metadata.push_str(&format!("  ·  {}", tr("日志"))),
            }
            let mut warnings = Vec::new();
            if page.invalid_utf8 {
                warnings.push(tr("已替换无效字符"));
            }
            // Only Markdown has syntax that a page starting mid-file can cut.
            if page.partial_context && !self.settings.raw && format == Format::Markdown {
                warnings.push(tr("语法上下文不完整"));
            }
            if !warnings.is_empty() {
                if !status.is_empty() {
                    status.push_str(" · ");
                }
                status.push_str(&warnings.join(" · "));
            }
            let percent = self.reading_percent();
            let progress = chrome_rect(
                self.width - self.s(206),
                top + self.s(3),
                self.width - self.s(130),
                self.height - self.s(3),
            );
            self.chrome_button(dc, "", progress, GOTO, ChromeTone::Quiet, p);
            self.label(
                dc,
                &format!("{percent:.1}%"),
                progress,
                11,
                false,
                p.muted,
                DT_SINGLELINE | DT_VCENTER | DT_CENTER,
            );
        }
        self.label(
            dc,
            &metadata,
            chrome_rect(self.s(82), top, self.s(242), self.height),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
        self.label(
            dc,
            &status,
            chrome_rect(self.s(250), top, self.width - self.s(218), self.height),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        Self::fill(
            dc,
            &chrome_rect(
                self.width - self.s(121),
                top + self.s(9),
                self.width - self.s(121) + 1,
                self.height - self.s(9),
            ),
            p.edge,
        );
        for (left, right, id) in [(111, 85, ZOOM_OUT), (40, 14, ZOOM_IN)] {
            let rect = chrome_rect(
                self.width - self.s(left),
                top + self.s(3),
                self.width - self.s(right),
                self.height - self.s(3),
            );
            self.chrome_button(dc, "", rect, id, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, rect, id, p.muted);
        }
        self.label(
            dc,
            &format!("{} px", self.settings.font_size),
            chrome_rect(
                self.width - self.s(83),
                top,
                self.width - self.s(42),
                self.height,
            ),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER | DT_CENTER,
        );
    }

    /// Preview / source switch at the left of the footer: a sunken track with a
    /// raised thumb under the active mode.
    unsafe fn paint_mode_switch(&mut self, dc: HDC, top: i32, p: Palette) {
        let dark = self.active_theme().is_dark();
        let (y0, y1) = (top + self.s(5), self.height - self.s(4));
        let track = chrome_rect(self.s(10), y0, self.s(70), y1);
        let sunken = if dark {
            mix(p.panel, p.bg, 60)
        } else {
            mix(p.panel, p.fg, 6)
        };
        chrome_round(dc, track, sunken, self.s(6));
        for (index, id) in [PREVIEW, SOURCE].into_iter().enumerate() {
            let x = track.left + self.s(2 + index as i32 * 28);
            let rect = chrome_rect(x, y0 + self.s(2), x + self.s(28), y1 - self.s(2));
            let active = (id == SOURCE) == self.settings.raw;
            let hover = self.hover == Some(id);
            if active {
                let face = if dark { mix(p.panel, p.fg, 14) } else { p.page };
                chrome_round(dc, rect, p.edge, self.s(4));
                chrome_round(
                    dc,
                    chrome_rect(rect.left + 1, rect.top + 1, rect.right - 1, rect.bottom - 1),
                    face,
                    self.s(4) - 1,
                );
            }
            self.hits.push(HitBox {
                rect,
                action: Hit::Command(id),
            });
            let color = if active || hover { p.fg } else { p.muted };
            self.header_glyph(dc, rect, id, color, 0.66);
        }
    }

    pub(super) unsafe fn paint_scrollbars(&mut self, dc: HDC, p: Palette) {
        for axis in [Axis::Vertical, Axis::Horizontal] {
            let Some(bar) = self.scroll_bar(axis) else {
                continue;
            };
            let hover = self.mouse.is_some_and(|(x, y)| contains(bar.track, x, y));
            let active = self.drag.as_ref().is_some_and(|drag| drag.axis == axis);
            Self::fill(dc, &bar.track, p.bg);
            let mut thumb = bar.thumb;
            let thickness = self.s(if hover || active { 8 } else { 6 }).max(3);
            match axis {
                Axis::Vertical => {
                    let center = (bar.track.left + bar.track.right) / 2;
                    thumb.left = center - thickness / 2;
                    thumb.right = thumb.left + thickness;
                    if hover || active {
                        Self::fill(
                            dc,
                            &chrome_rect(center, bar.track.top, center + 1, bar.track.bottom),
                            p.edge,
                        );
                    }
                }
                Axis::Horizontal => {
                    let center = (bar.track.top + bar.track.bottom) / 2;
                    thumb.top = center - thickness / 2;
                    thumb.bottom = thumb.top + thickness;
                    if hover || active {
                        Self::fill(
                            dc,
                            &chrome_rect(bar.track.left, center, bar.track.right, center + 1),
                            p.edge,
                        );
                    }
                }
            }
            chrome_round(
                dc,
                thumb,
                if active {
                    p.accent
                } else if hover {
                    p.muted
                } else {
                    p.edge
                },
                thickness / 2,
            );
        }
        let edge = CreateSolidBrush(p.edge);
        FrameRect(dc, &chrome_rect(0, 0, self.width, self.height), edge);
        DeleteObject(edge);
    }

    /// Book-like skins lay the reading column on a sheet; flat skins draw nothing.
    pub(super) unsafe fn paint_page(&mut self, dc: HDC, p: Palette) {
        if p.page == p.bg {
            return;
        }
        let left = self.left();
        let right = left + self.body_width();
        // Leave the vertical scrollbar track (width - 20) and the sidebar uncovered.
        let pad = self
            .s(24)
            .min(left - self.sidebar_width() - self.s(4))
            .min(self.width - self.s(22) - right)
            .max(0);
        let sheet = chrome_rect(
            left - pad,
            self.content_top(),
            right + pad,
            self.content_bottom() - self.s(4),
        );
        if sheet.right <= sheet.left || sheet.bottom <= sheet.top {
            return;
        }
        // A soft shadow below the sheet, then the paper itself.
        chrome_round(
            dc,
            chrome_rect(
                sheet.left,
                sheet.top + self.s(2),
                sheet.right,
                sheet.bottom + self.s(2),
            ),
            p.edge,
            self.s(6),
        );
        chrome_round(dc, sheet, p.page, self.s(6));
        // Wide or narrow layouts without a real margin keep the plain sheet.
        if let Some(rule) = p.ornament.filter(|_| pad >= self.s(12)) {
            // Double ruled margins, as on a woodblock-printed page.
            let inset = (pad / 2).max(self.s(4));
            for (x, width) in [
                (sheet.left + inset - self.s(4), self.s(2)),
                (sheet.left + inset, 1),
                (sheet.right - inset - 1, 1),
                (sheet.right - inset + self.s(2), self.s(2)),
            ] {
                Self::fill(
                    dc,
                    &chrome_rect(
                        x,
                        sheet.top + self.s(10),
                        x + width.max(1),
                        sheet.bottom - self.s(10),
                    ),
                    rule,
                );
            }
        }
    }

    /// The box at the pointer's height, the visible part of its rows (the reading
    /// column, or a narrower table's own width) and the column's right edge. None
    /// while the pointer is busy elsewhere.
    pub(super) fn copy_target(&self) -> Option<(usize, RECT, i32)> {
        let (x, y) = self.mouse?;
        if self.boxes.is_empty()
            || self.settings.raw
            || self.selecting
            || self.drag.is_some()
            || self.theme_menu.is_some()
            || !self.reading_area(x, y)
        {
            return None;
        }
        let row = self.hits.iter().find_map(|h| match h.action {
            Hit::Row(i) if y >= h.rect.top && y < h.rect.bottom => Some(i),
            _ => None,
        })?;
        let index = self.boxes.partition_point(|b| b.rows.1 < row);
        let b = self.boxes.get(index).filter(|b| b.rows.0 <= row)?;
        let mut area: Option<RECT> = None;
        for h in &self.hits {
            if let Hit::Row(i) = h.action
                && (b.rows.0..=b.rows.1).contains(&i)
            {
                let r = area.get_or_insert(h.rect);
                r.top = r.top.min(h.rect.top);
                r.bottom = r.bottom.max(h.rect.bottom);
            }
        }
        let mut area = area?;
        let column = area.right;
        area.bottom = area.bottom.min(self.content_bottom());
        let first = self.layout.as_ref()?.rows.get(b.rows.0)?;
        if let Some(edge) = first.table.as_ref().and_then(|t| t.edges.last()) {
            area.right = area.right.min(self.left() + edge + 1 - self.shift(first));
        }
        // A table scrolled out of the column has nothing to show a button for.
        (x >= area.left && area.right > area.left).then_some((index, area, column))
    }

    /// Width of `text` in the interface font at logical `size`.
    unsafe fn text_width(&mut self, dc: HDC, text: &str, size: i32, bold: bool) -> i32 {
        let units: Vec<u16> = text.encode_utf16().collect();
        let old = SelectObject(
            dc,
            self.fonts.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
                underline: false,
            }),
        );
        let mut extent = SIZE::default();
        GetTextExtentPoint32W(dc, units.as_ptr(), units.len() as i32, &mut extent);
        SelectObject(dc, old);
        extent.cx
    }

    /// Copy button in the top-right corner of the box under the pointer. It keeps to
    /// the top of the box's visible part, so long code blocks need no scrolling back.
    /// A table narrower than the column gets it just outside its corner instead of
    /// over its header.
    pub(super) unsafe fn paint_copy_button(&mut self, dc: HDC, p: Palette) {
        let (Some((index, area, column)), Some((x, y))) = (self.copy_target(), self.mouse) else {
            return;
        };
        if self.boxes[index].content.is_none() {
            return;
        }
        let copied = self.just_copied(index);
        let label = if copied {
            tr("已复制")
        } else {
            tr("复制")
        };
        let room = area.bottom - area.top;
        let height = self.s(22).min(room - self.s(4));
        if height < self.s(16) {
            return;
        }
        let width = self.s(30) + self.text_width(dc, label, 11, false);
        // The wider label decides, so the button stays put when it changes.
        let widest = self.s(30)
            + self
                .text_width(dc, tr("复制"), 11, false)
                .max(self.text_width(dc, tr("已复制"), 11, false));
        let gap = self.s(6);
        let outside = self.boxes[index].kind == markdown::BoxKind::Table
            && area.right + gap + widest <= column;
        let top = area.top + self.s(4).min((room - height) / 2);
        let r = if outside {
            chrome_rect(
                area.right + gap,
                top,
                area.right + gap + width,
                top + height,
            )
        } else {
            chrome_rect(
                area.right - gap - width,
                top,
                area.right - gap,
                top + height,
            )
        };
        // The strip beside a narrow table belongs to it while its button is there.
        let reach = if outside { r.right } else { area.right };
        if x >= reach {
            return;
        }
        let hover = contains(r, x, y);
        let pressed = self.pressed == Some(Hit::CopyBlock(index));
        let radius = self.s(5);
        // A one-pixel outline, then the face.
        chrome_round(dc, r, if copied { p.accent } else { p.edge }, radius);
        chrome_round(
            dc,
            chrome_rect(r.left + 1, r.top + 1, r.right - 1, r.bottom - 1),
            if pressed {
                p.selection
            } else if hover {
                p.panel
            } else {
                p.page
            },
            radius - 1,
        );
        let color = if copied {
            p.accent
        } else if hover {
            p.fg
        } else {
            p.muted
        };
        // The open check mark keeps the full glyph scale so its stroke matches the
        // label; the closed sheets read as heavy at a smaller one.
        smooth::icon(
            dc,
            chrome_rect(r.left + self.s(4), r.top, r.left + self.s(22), r.bottom),
            if copied { smooth::CHECK } else { smooth::COPY },
            color,
            self.fonts.dpi as f32 / 96.0 * if copied { 1.0 } else { 0.72 },
        );
        self.label(
            dc,
            label,
            chrome_rect(r.left + self.s(22), r.top, r.right - self.s(6), r.bottom),
            11,
            false,
            color,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        self.hits.push(HitBox {
            rect: r,
            action: Hit::CopyBlock(index),
        });
    }
}
