//! Custom window chrome and quiet reading controls.
use super::*;

#[derive(Clone, Copy)]
enum ChromeTone {
    Quiet,
    Selected,
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
            ChromeTone::Selected => Some(p.selection),
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
            chrome_round(
                dc,
                rect,
                color,
                self.s(if matches!(tone, ChromeTone::Primary) {
                    8
                } else {
                    6
                }),
            );
        }
        let color = match tone {
            ChromeTone::Selected => p.accent,
            ChromeTone::Primary => p.bg,
            ChromeTone::Quiet => p.fg,
        };
        self.label(
            dc,
            text,
            rect,
            12,
            matches!(tone, ChromeTone::Primary | ChromeTone::Selected),
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

    pub(super) fn header_left(&self) -> i32 {
        self.width - self.s(132) - self.s(260) - 1
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
        let right = self.header_left() + self.s(216);
        // Stay below the native search EDIT HWND; parent painting cannot cover a child window.
        let gap = if self.edit_mode.is_some() { 56 } else { 6 };
        let top = self.s(HEADER_HEIGHT + gap);
        chrome_rect(right - self.s(224), top, right, top + self.s(198))
    }

    pub(super) fn header_tip(&self) -> Option<&'static str> {
        if self.theme_menu.is_some() || self.mouse.is_none_or(|(_, y)| y >= self.s(HEADER_HEIGHT)) {
            return None;
        }
        Some(match self.hover? {
            PREVIEW => "预览 · 阅读排版",
            SOURCE => "源码 · Ctrl+R",
            OPEN => "打开文档 · Ctrl+O",
            FIND => "查找 · Ctrl+F",
            SETTINGS => "更多选项",
            THEME_PICKER => match self.settings.theme {
                Theme::System => "外观：跟随系统 · Ctrl+T",
                Theme::Light => "外观：浅色 · Ctrl+T",
                Theme::Dark => "外观：深色 · Ctrl+T",
                Theme::Sepia => "外观：护眼 · Ctrl+T",
            },
            MINIMIZE => "最小化",
            MAXIMIZE => "最大化 / 还原",
            EXIT => "关闭",
            _ => return None,
        })
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
                "外观",
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
            for (i, (id, text)) in [
                (THEME_LIGHT, "浅色"),
                (THEME_DARK, "深色"),
                (THEME_SYSTEM, "跟随系统"),
            ]
            .into_iter()
            .enumerate()
            {
                let y = r.top + self.s(38 + i as i32 * 42);
                let row = chrome_rect(r.left + self.s(7), y, r.right - self.s(7), y + self.s(38));
                let selected = matches!(
                    (id, self.settings.theme),
                    (THEME_LIGHT, Theme::Light)
                        | (THEME_DARK, Theme::Dark)
                        | (THEME_SYSTEM, Theme::System)
                );
                if selected || i == focus || self.hover == Some(id) {
                    chrome_round(
                        dc,
                        row,
                        if selected { p.selection } else { p.code },
                        self.s(6),
                    );
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
                    id,
                    color,
                );
                self.label(
                    dc,
                    text,
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
                "跟随系统将自动切换明暗",
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
            let r = chrome_rect(
                left,
                self.s(HEADER_HEIGHT + 7),
                left + width,
                self.s(HEADER_HEIGHT + 41),
            );
            chrome_round(dc, r, p.fg, self.s(6));
            self.label(
                dc,
                text,
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
        let controls_left = self.width - self.s(132) - 1;
        let brand = self.document.is_none() || self.width >= self.s(900);
        self.feather_mark(
            dc,
            chrome_rect(
                self.s(16),
                self.s(middle - 12),
                self.s(40),
                self.s(middle + 12),
            ),
            p.accent,
        );
        if brand {
            self.label(
                dc,
                "FeatherMD",
                chrome_rect(self.s(48), 1, self.s(142), bottom),
                13,
                true,
                p.fg,
                DT_SINGLELINE | DT_VCENTER,
            );
        }
        if !self.document_name.is_empty() {
            let x = self.s(if brand { 161 } else { 52 });
            if brand {
                Self::fill(
                    dc,
                    &chrome_rect(
                        self.s(147),
                        self.s(middle - 7),
                        self.s(147) + 1,
                        self.s(middle + 7),
                    ),
                    p.edge,
                );
            }
            let name = self.document_name.clone();
            self.label(
                dc,
                &name,
                chrome_rect(x, 1, left - self.s(16), bottom),
                12,
                false,
                p.muted,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        chrome_round(
            dc,
            chrome_rect(
                left,
                self.s(middle - 15),
                left + self.s(76),
                self.s(middle + 15),
            ),
            p.code,
            self.s(8),
        );
        for (offset, id, selected) in [
            (2, PREVIEW, !self.settings.raw),
            (38, SOURCE, self.settings.raw),
        ] {
            let rect = chrome_rect(
                left + self.s(offset),
                self.s(middle - 13),
                left + self.s(offset + 36),
                self.s(middle + 13),
            );
            self.chrome_button(
                dc,
                "",
                rect,
                id,
                if selected {
                    ChromeTone::Selected
                } else {
                    ChromeTone::Quiet
                },
                p,
            );
            self.chrome_glyph(dc, rect, id, if selected { p.accent } else { p.muted });
        }
        Self::fill(
            dc,
            &chrome_rect(
                left + self.s(86),
                self.s(middle - 7),
                left + self.s(86) + 1,
                self.s(middle + 7),
            ),
            p.edge,
        );
        for (offset, width, id) in [
            (96, 36, OPEN),
            (134, 36, FIND),
            (172, 44, THEME_PICKER),
            (218, 36, SETTINGS),
        ] {
            let rect = chrome_rect(
                left + self.s(offset),
                self.s(middle - 15),
                left + self.s(offset + width),
                self.s(middle + 15),
            );
            self.chrome_button(
                dc,
                "",
                rect,
                id,
                if id == THEME_PICKER && self.theme_menu.is_some() {
                    ChromeTone::Selected
                } else {
                    ChromeTone::Quiet
                },
                p,
            );
            let glyph = if id == THEME_PICKER {
                match self.settings.theme {
                    Theme::Light => THEME_LIGHT,
                    Theme::Dark => THEME_DARK,
                    Theme::System => THEME_SYSTEM,
                    Theme::Sepia => THEME_SEPIA,
                }
            } else {
                id
            };
            let mut icon = rect;
            if id == THEME_PICKER {
                icon.right -= self.s(10);
            }
            self.chrome_glyph(dc, icon, glyph, p.muted);
            if id == THEME_PICKER {
                self.chrome_glyph(
                    dc,
                    chrome_rect(
                        rect.right - self.s(15),
                        rect.top,
                        rect.right - self.s(3),
                        rect.bottom,
                    ),
                    THEME_PICKER,
                    p.muted,
                );
            }
        }
        for (index, id) in [MINIMIZE, MAXIMIZE, EXIT].into_iter().enumerate() {
            let x = controls_left + self.s(index as i32 * 44);
            let rect = chrome_rect(x, 1, x + self.s(44), bottom);
            self.chrome_button(dc, "", rect, id, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, rect, id, p.muted);
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
                    "跳转 %"
                } else {
                    "下一个"
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
            "目录",
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
                .map(|(index, heading)| (index, heading.level, heading.text.clone()))
                .collect();
            (entries, outline.headings.len(), outline.limited)
        } else {
            (Vec::new(), 0, false)
        };
        let bottom = self.height - self.s(62);
        let saved = SaveDC(dc);
        IntersectClipRect(dc, 1, self.s(OUTLINE_ROWS_TOP), width - 1, bottom);
        for (row, (index, level, text)) in entries.into_iter().enumerate() {
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

        let status = if let Some(error) = &self.outline_error {
            format!("目录读取失败：{error}")
        } else if limited {
            "目录已达显示上限".to_string()
        } else if self.outline_loading {
            "正在提取标题…".to_string()
        } else if total == 0 {
            if self.document.is_none() {
                "打开文档后显示目录"
            } else {
                "文档暂无标题"
            }
            .to_string()
        } else {
            format!("{total} 个标题")
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
            "把注意力，留给文字。",
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
            "一个轻量、离线的 Markdown 阅读空间",
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
        let action = chrome_rect(
            center - self.s(104),
            top + self.s(222),
            center + self.s(104),
            top + self.s(268),
        );
        self.chrome_button(dc, "", action, OPEN, ChromeTone::Primary, p);
        let foreground = p.bg;
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
                "正在打开…"
            } else {
                "打开文档"
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
            "或拖入 .md / .markdown / .txt 文件",
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
        Self::fill(
            dc,
            &chrome_rect(1, top, self.width - 1, self.height - 1),
            p.panel,
        );
        Self::fill(dc, &chrome_rect(1, top, self.width - 1, top + 1), p.edge);
        let mut metadata = "只读  ·  离线".to_string();
        let mut status = if self.status == "就绪"
            || self.status == "只读 · 离线阅读"
            || self.status.starts_with("离线阅读")
        {
            String::new()
        } else {
            self.status.clone()
        };
        if let Some(page) = &self.page {
            metadata = format!(
                "{}  ·  UTF-8{}",
                human_size(page.size),
                if page.invalid_utf8 { "*" } else { "" }
            );
            let mut warnings = Vec::new();
            if page.invalid_utf8 {
                warnings.push("已替换无效字符");
            }
            if page.partial_context && !self.settings.raw {
                warnings.push("语法上下文不完整");
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
            chrome_rect(self.s(16), top, self.s(176), self.height),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
        self.label(
            dc,
            &status,
            chrome_rect(self.s(184), top, self.width - self.s(218), self.height),
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
}
