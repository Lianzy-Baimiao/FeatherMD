//! Custom window chrome and quiet reading controls.
use super::*;

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

unsafe fn chrome_round(dc: HDC, rect: RECT, color: u32, radius: i32) {
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    RoundRect(
        dc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius * 2,
        radius * 2,
    );
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    DeleteObject(brush);
}

unsafe fn chrome_path(dc: HDC, points: &[(i32, i32)]) {
    if let Some(&(x, y)) = points.first() {
        MoveToEx(dc, x, y, null_mut());
        for &(x, y) in &points[1..] {
            LineTo(dc, x, y);
        }
    }
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
            chrome_round(dc, rect, color, self.s(5));
        }
        let color = match tone {
            ChromeTone::Primary => p.bg,
            ChromeTone::Quiet => p.fg,
        };
        self.label(
            dc,
            text,
            rect,
            13,
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
        let x = (rect.left + rect.right) / 2;
        let y = (rect.top + rect.bottom) / 2;
        let n = self.s(5);
        let pen = CreatePen(PS_SOLID, self.s(1).max(1), color);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        match id {
            MINIMIZE => chrome_path(dc, &[(x - n, y), (x + n + 1, y)]),
            MAXIMIZE if IsZoomed(self.hwnd) != 0 => {
                chrome_path(
                    dc,
                    &[
                        (x - n + self.s(3), y - n + self.s(2)),
                        (x - n + self.s(3), y - n),
                        (x + n, y - n),
                        (x + n, y + n - self.s(3)),
                        (x + n - self.s(2), y + n - self.s(3)),
                    ],
                );
                Rectangle(dc, x - n, y - n + self.s(3), x + n - self.s(2), y + n + 1);
            }
            MAXIMIZE => {
                Rectangle(dc, x - n, y - n, x + n + 1, y + n + 1);
            }
            EXIT | 0 => {
                chrome_path(dc, &[(x - n, y - n), (x + n, y + n)]);
                chrome_path(dc, &[(x + n, y - n), (x - n, y + n)]);
            }
            ZOOM_OUT | ZOOM_IN => {
                chrome_path(dc, &[(x - n, y), (x + n + 1, y)]);
                if id == ZOOM_IN {
                    chrome_path(dc, &[(x, y - n), (x, y + n + 1)]);
                }
            }
            OUTLINE => {
                Rectangle(
                    dc,
                    x - self.s(7),
                    y - self.s(6),
                    x + self.s(8),
                    y + self.s(7),
                );
                chrome_path(
                    dc,
                    &[
                        (x - self.s(2), y - self.s(6)),
                        (x - self.s(2), y + self.s(6)),
                    ],
                );
            }
            _ => {}
        }
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    pub(super) unsafe fn paint_header(&mut self, dc: HDC, p: Palette) {
        Self::fill(dc, &chrome_rect(1, 1, self.width - 1, self.s(48)), p.panel);
        let brand = self.width >= self.s(760) || self.document_name.is_empty();
        if brand {
            self.label(
                dc,
                "FeatherMD",
                chrome_rect(self.s(18), 1, self.s(129), self.s(48)),
                13,
                true,
                p.fg,
                DT_SINGLELINE | DT_VCENTER,
            );
        }
        let controls_left = self.width - self.s(132) - 1;
        let actions_left = controls_left - self.s(152);
        if !self.document_name.is_empty() {
            let tab = chrome_rect(
                self.s(if brand { 144 } else { 21 }),
                self.s(9),
                actions_left - self.s(12),
                self.s(39),
            );
            if tab.right - tab.left > self.s(60) {
                let badge = chrome_rect(tab.left, self.s(16), tab.left + self.s(25), self.s(32));
                chrome_round(dc, badge, p.code, self.s(3));
                self.label(
                    dc,
                    "MD",
                    badge,
                    9,
                    true,
                    p.accent,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                let name = self.document_name.clone();
                self.label(
                    dc,
                    &name,
                    chrome_rect(tab.left + self.s(35), tab.top, tab.right, tab.bottom),
                    12,
                    false,
                    p.fg,
                    DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                );
            }
        }
        for (left, right, text, id) in [
            (0, 48, "查找", FIND),
            (50, 98, "打开", OPEN),
            (102, 146, "更多", SETTINGS),
        ] {
            self.chrome_button(
                dc,
                text,
                chrome_rect(
                    actions_left + self.s(left),
                    self.s(9),
                    actions_left + self.s(right),
                    self.s(39),
                ),
                id,
                ChromeTone::Quiet,
                p,
            );
        }
        for (index, id) in [MINIMIZE, MAXIMIZE, EXIT].into_iter().enumerate() {
            let x = controls_left + self.s(index as i32 * 44);
            let rect = chrome_rect(x, 1, x + self.s(44), self.s(48));
            self.chrome_button(dc, "", rect, id, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, rect, id, p.fg);
        }

        Self::fill(
            dc,
            &chrome_rect(1, self.s(48), self.width - 1, self.s(48) + 1),
            p.edge,
        );

        if self.edit_mode.is_some() {
            Self::fill(
                dc,
                &chrome_rect(
                    self.sidebar_width().max(1),
                    self.s(49),
                    self.width - 1,
                    self.s(101),
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
                    self.s(60),
                    self.width - self.s(66),
                    self.s(92),
                ),
                NEXT,
                ChromeTone::Quiet,
                p,
            );
            let close = chrome_rect(
                self.width - self.s(56),
                self.s(60),
                self.width - self.s(24),
                self.s(92),
            );
            self.chrome_button(dc, "", close, 0, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, close, 0, p.muted);
            Self::fill(
                dc,
                &chrome_rect(
                    self.sidebar_width().max(1),
                    self.s(101),
                    self.width - 1,
                    self.s(101) + 1,
                ),
                p.edge,
            );
        }
    }

    pub(super) unsafe fn paint_outline(&mut self, dc: HDC, p: Palette) {
        let width = self.sidebar_width();
        if width == 0 {
            let expand = chrome_rect(self.s(6), self.s(62), self.s(38), self.s(94));
            chrome_round(dc, expand, p.panel, self.s(5));
            self.chrome_button(dc, "", expand, OUTLINE, ChromeTone::Quiet, p);
            self.chrome_window_icon(dc, expand, OUTLINE, p.muted);
            return;
        }
        let footer = self.height - self.s(30);
        Self::fill(dc, &chrome_rect(1, self.s(49), width, footer), p.panel);
        Self::fill(
            dc,
            &chrome_rect(width - 1, self.s(49), width, footer),
            p.edge,
        );
        self.label(
            dc,
            "目录",
            chrome_rect(self.s(18), self.s(62), width - self.s(52), self.s(90)),
            12,
            true,
            p.fg,
            DT_SINGLELINE | DT_VCENTER,
        );
        let collapse = chrome_rect(
            width - self.s(42),
            self.s(62),
            width - self.s(14),
            self.s(90),
        );
        self.chrome_button(dc, "", collapse, OUTLINE, ChromeTone::Quiet, p);
        self.chrome_window_icon(dc, collapse, OUTLINE, p.muted);
        let current = self.current();
        let visible = self.outline_visible_rows();
        let first = self.outline_top;
        // Copy only visible labels so GDI font lookup can borrow App mutably.
        let (entries, total, active, limited) = if let Some(outline) = &self.outline {
            let entries: Vec<_> = outline
                .headings
                .iter()
                .enumerate()
                .skip(first)
                .take(visible)
                .map(|(index, heading)| (index, heading.level, heading.text.clone()))
                .collect();
            (
                entries,
                outline.headings.len(),
                outline
                    .headings
                    .iter()
                    .rposition(|heading| heading.offset <= current),
                outline.limited,
            )
        } else {
            (Vec::new(), 0, None, false)
        };
        let bottom = self.height - self.s(62);
        let saved = SaveDC(dc);
        IntersectClipRect(dc, 1, self.s(104), width - 1, bottom);
        for (row, (index, level, text)) in entries.into_iter().enumerate() {
            let top = self.s(104 + row as i32 * 30);
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
            self.s(112)
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
            let track_height = (bottom - self.s(104)).max(1);
            let thumb_height = ((track_height as i64 * visible as i64 / total as i64) as i32)
                .max(self.s(18))
                .min(track_height);
            let offset = ((track_height - thumb_height) as i64 * first.min(total - visible) as i64
                / (total - visible) as i64) as i32;
            chrome_round(
                dc,
                chrome_rect(
                    width - self.s(5),
                    self.s(104) + offset,
                    width - self.s(3),
                    self.s(104) + offset + thumb_height,
                ),
                p.edge,
                self.s(1),
            );
        }
    }

    pub(super) unsafe fn paint_welcome(&mut self, dc: HDC, p: Palette) {
        let sidebar = self.sidebar_width();
        let left = sidebar + ((self.width - sidebar - self.s(360)) / 2).max(self.s(24));
        let room = self.content_bottom() - self.content_top();
        let compact = room < self.s(260);
        let (line, description, action, hint, block) = if compact {
            (36, 82, 120, 174, 200)
        } else {
            (42, 101, 147, 207, 236)
        };
        let top = self.content_top() + ((room - self.s(block)) / 2).max(self.s(6));
        for (i, text) in ["打开一份文档，", "开始阅读。"].into_iter().enumerate() {
            self.label(
                dc,
                text,
                chrome_rect(
                    left,
                    top + self.s(i as i32 * line),
                    self.width - self.s(36),
                    top + self.s(line + i as i32 * line),
                ),
                if compact { 26 } else { 28 },
                true,
                p.fg,
                DT_SINGLELINE | DT_VCENTER,
            );
        }
        self.label(
            dc,
            "拖入 Markdown 文件，或从这里打开。",
            chrome_rect(
                left,
                top + self.s(description),
                self.width - self.s(30),
                top + self.s(description + 25),
            ),
            13,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER,
        );
        self.chrome_button(
            dc,
            if self.loading {
                "正在打开…"
            } else {
                "打开文档"
            },
            chrome_rect(
                left,
                top + self.s(action),
                left + self.s(140),
                top + self.s(action + 40),
            ),
            OPEN,
            ChromeTone::Primary,
            p,
        );
        self.label(
            dc,
            "Ctrl + O",
            chrome_rect(
                left + self.s(157),
                top + self.s(action),
                left + self.s(244),
                top + self.s(action + 40),
            ),
            12,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER,
        );
        self.label(
            dc,
            "只读文档   ·   离线阅读",
            chrome_rect(
                left,
                top + self.s(hint),
                self.width - self.s(30),
                top + self.s(hint + 25),
            ),
            11,
            false,
            p.muted,
            DT_SINGLELINE | DT_VCENTER,
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
        let mut status = if self.status == "就绪" || self.status.starts_with("离线阅读") {
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
