//! Native Win32 shell. All file I/O and searches run off the window thread.
#![allow(unsafe_op_in_unsafe_fn)]
mod chrome;
use crate::layout::{FontKey, Fonts, Layout, Row};
use feathermd::{
    document::{Document, Page, SearchResult, human_size},
    markdown,
    scrollbar::ScrollTrack,
    settings::{Settings, Theme},
    viewport::Viewport,
    worker::{Reply, Request, SearchRequest, Workers},
};
use std::{
    cell::RefCell,
    ffi::OsString,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{Arc, atomic::Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{DataExchange::*, LibraryLoader::*, Memory::*},
    UI::{
        Controls::{Dialogs::*, WM_MOUSELEAVE},
        HiDpi::*,
        Input::KeyboardAndMouse::*,
        Shell::*,
        WindowsAndMessaging::*,
    },
};
const OPEN: usize = 101;
const FIND: usize = 102;
const GOTO: usize = 103;
const RELOAD: usize = 104;
const COPY: usize = 105;
const EXIT: usize = 106;
const THEME: usize = 201;
const ZOOM_IN: usize = 202;
const ZOOM_OUT: usize = 203;
const WRAP: usize = 204;
const RAW: usize = 205;
const WIDE: usize = 206;
const RESET: usize = 207;
const REGISTER: usize = 301;
const UNREGISTER: usize = 302;
const ABOUT: usize = 401;
const NEXT: usize = 402;
const SETTINGS: usize = 403;
const LICENSES: usize = 404;
const FILE_MENU: usize = 405;
const HELP_MENU: usize = 406;
const MINIMIZE: usize = 407;
const MAXIMIZE: usize = 408;
const PREVIEW: usize = 409;
const SOURCE: usize = 410;
const OUTLINE: usize = 411;
const fn make_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
#[derive(Clone, Copy)]
struct Palette {
    bg: u32,
    fg: u32,
    muted: u32,
    panel: u32,
    edge: u32,
    accent: u32,
    selection: u32,
    code: u32,
}
impl Palette {
    fn new(theme: Theme) -> Self {
        match theme {
            Theme::Light => Self {
                bg: rgb(252, 252, 250),
                fg: rgb(35, 43, 48),
                muted: rgb(108, 117, 124),
                panel: rgb(244, 246, 243),
                edge: rgb(224, 229, 225),
                accent: rgb(20, 113, 99),
                selection: rgb(215, 236, 228),
                code: rgb(239, 242, 238),
            },
            Theme::Dark => Self {
                bg: rgb(25, 29, 34),
                fg: rgb(222, 229, 233),
                muted: rgb(147, 160, 168),
                panel: rgb(31, 37, 42),
                edge: rgb(49, 58, 65),
                accent: rgb(112, 211, 181),
                selection: rgb(42, 79, 72),
                code: rgb(35, 42, 48),
            },
            Theme::Sepia => Self {
                bg: rgb(247, 239, 221),
                fg: rgb(68, 57, 42),
                muted: rgb(128, 111, 86),
                panel: rgb(241, 230, 208),
                edge: rgb(220, 205, 177),
                accent: rgb(131, 91, 44),
                selection: rgb(228, 212, 172),
                code: rgb(237, 225, 199),
            },
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
enum Hit {
    Command(usize),
    Link(String),
    Row(usize),
    Heading(usize),
}
struct HitBox {
    rect: RECT,
    action: Hit,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}
#[derive(Clone, Copy)]
struct ScrollDrag {
    axis: Axis,
    grab: i32,
}
struct Bar {
    track: RECT,
    thumb: RECT,
    scale: ScrollTrack,
    pixels: bool,
}
fn contains(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}
struct App {
    hwnd: HWND,
    edit: HWND,
    menu: HMENU,
    settings_menu: HMENU,
    fonts: Fonts,
    settings: Settings,
    settings_path: PathBuf,
    workers: Workers,
    io_ticket: u64,
    document: Option<Arc<Document>>,
    page: Option<Page>,
    layout: Option<Layout>,
    top: usize,
    anchor: u64,
    loading: bool,
    pending_scroll: Option<(usize, i32)>,
    width: i32,
    height: i32,
    hits: Vec<HitBox>,
    selection: Option<(usize, usize)>,
    selecting: bool,
    mouse: Option<(i32, i32)>,
    hover: Option<usize>,
    pressed: Option<Hit>,
    drag: Option<ScrollDrag>,
    wheel_remainder: i32,
    horizontal_wheel_remainder: i32,
    document_name: String,
    outline: Option<feathermd::outline::Outline>,
    outline_ticket: u64,
    outline_loading: bool,
    outline_error: Option<String>,
    outline_top: usize,
    horizontal: i32,
    status: String,
    edit_mode: Option<usize>,
    query: String,
    found: Option<u64>,
    search_ticket: u64,
}
impl App {
    fn new() -> Self {
        let settings_path = Settings::path();
        let settings = Settings::load(&settings_path);
        Self {
            hwnd: null_mut(),
            edit: null_mut(),
            menu: null_mut(),
            settings_menu: null_mut(),
            fonts: Fonts::new(96),
            settings,
            settings_path,
            workers: Workers::new(),
            io_ticket: 0,
            document: None,
            page: None,
            layout: None,
            top: 0,
            anchor: 0,
            loading: false,
            pending_scroll: None,
            width: 1000,
            height: 760,
            hits: vec![],
            selection: None,
            selecting: false,
            mouse: None,
            hover: None,
            pressed: None,
            drag: None,
            wheel_remainder: 0,
            horizontal_wheel_remainder: 0,
            document_name: String::new(),
            outline: None,
            outline_ticket: 0,
            outline_loading: false,
            outline_error: None,
            outline_top: 0,
            horizontal: 0,
            status: "只读 · 离线阅读".into(),
            edit_mode: None,
            query: String::new(),
            found: None,
            search_ticket: 0,
        }
    }
    fn s(&self, n: i32) -> i32 {
        self.fonts.scale(n)
    }
    fn content_top(&self) -> i32 {
        self.s(if self.edit_mode.is_some() { 104 } else { 56 })
    }
    fn content_bottom(&self) -> i32 {
        self.height - self.s(if self.horizontal_max() > 0 { 50 } else { 34 })
    }
    fn body_width(&self) -> i32 {
        let usable = (self.width - self.sidebar_width() - self.s(80)).max(self.s(80));
        if self.settings.wide {
            usable
        } else {
            usable.min(self.s(840))
        }
    }
    fn left(&self) -> i32 {
        self.sidebar_width()
            + ((self.width - self.sidebar_width() - self.body_width()) / 2).max(self.s(20))
    }
    fn sidebar_width(&self) -> i32 {
        if self.settings.outline {
            self.s(if self.width < self.s(800) { 192 } else { 232 })
        } else {
            0
        }
    }
    fn outline_visible_rows(&self) -> usize {
        ((self.height - self.s(62) - self.s(104)) / self.s(30).max(1)).max(1) as usize
    }
    fn reading_percent(&self) -> f64 {
        let Some(page) = &self.page else {
            return 0.0;
        };
        if page.size == 0 {
            return 0.0;
        }
        if page.end >= page.size && self.top >= self.viewport().bottom_top() {
            return 100.0;
        }
        self.current() as f64 * 100.0 / page.size as f64
    }
    unsafe fn start_outline(&mut self) {
        if !self.settings.outline || self.outline.is_some() || self.outline_loading {
            return;
        }
        if let Some(document) = &self.document {
            self.outline_ticket = self.workers.cancel_outline();
            self.outline_loading = true;
            self.outline_error = None;
            let _ = self
                .workers
                .outline
                .send(feathermd::worker::OutlineRequest {
                    document: document.clone(),
                    ticket: self.outline_ticket,
                });
        }
    }
    unsafe fn toggle_outline(&mut self) {
        self.settings.outline = !self.settings.outline;
        if !self.settings.outline {
            self.outline_ticket = self.workers.cancel_outline();
            self.outline_loading = false;
        }
        self.persist();
        self.start_outline();
        if let Some(mode) = self.edit_mode {
            self.position_edit(mode);
        }
    }
    unsafe fn scroll_outline(&mut self, delta: i32) {
        let count = self.outline.as_ref().map_or(0, |o| o.headings.len());
        self.outline_top = (self.outline_top as i64 + i64::from(delta))
            .clamp(0, count.saturating_sub(self.outline_visible_rows()) as i64)
            as usize;
        self.invalidate();
    }
    fn current(&self) -> u64 {
        self.layout
            .as_ref()
            .and_then(|l| l.rows.get(self.top))
            .map_or(self.anchor, |r| r.start)
    }
    unsafe fn invalidate(&self) {
        InvalidateRect(self.hwnd, null(), 0);
    }
    unsafe fn open(&mut self, path: PathBuf) {
        self.outline_ticket = self.workers.cancel_outline();
        self.outline = None;
        self.outline_loading = false;
        self.outline_error = None;
        self.outline_top = 0;
        self.pending_scroll = None;
        self.workers.cancel_search();
        self.io_ticket += 1;
        self.loading = true;
        self.document = None;
        self.page = None;
        self.layout = None;
        self.top = 0;
        self.anchor = 0;
        self.horizontal = 0;
        self.selection = None;
        self.drag = None;
        self.wheel_remainder = 0;
        self.document_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.found = None;
        self.status = "正在读取首屏…".into();
        SetWindowTextW(
            self.hwnd,
            w(&format!(
                "{} — FeatherMD",
                path.file_name().unwrap_or_default().to_string_lossy()
            ))
            .as_ptr(),
        );
        let _ = self.workers.io.send(Request::Open(path, self.io_ticket));
        self.invalidate();
    }
    unsafe fn request_page(&mut self, anchor: u64) {
        self.pending_scroll = None;
        if let Some(doc) = &self.document {
            self.io_ticket += 1;
            self.loading = true;
            self.anchor = anchor;
            self.status = "正在读取分页…".into();
            let _ = self
                .workers
                .io
                .send(Request::Page(doc.clone(), anchor, self.io_ticket));
            self.invalidate();
        }
    }
    unsafe fn relayout(&mut self, anchor: u64) {
        if let Some(page) = &self.page {
            let blocks = markdown::blocks(&page.lines, self.settings.raw);
            let dc = GetDC(self.hwnd);
            let width = self.body_width();
            self.layout = Some(Layout::build(
                dc,
                &blocks,
                &self.settings,
                &mut self.fonts,
                width,
            ));
            ReleaseDC(self.hwnd, dc);
            self.position(anchor, 0);
            self.selection = None;
            self.scrollbar();
        }
        self.invalidate();
    }
    unsafe fn receive(&mut self) {
        while let Ok(reply) = self.workers.replies.try_recv() {
            match reply {
                Reply::Opened(id, result) if id == self.io_ticket => {
                    self.loading = false;
                    match result {
                        Ok((doc, page)) => {
                            self.document = Some(doc);
                            self.page = Some(page);
                            self.anchor = 0;
                            self.status = "就绪".into();
                            self.relayout(0);
                            self.start_outline();
                        }
                        Err(e) => {
                            self.status = format!("打开失败：{e}");
                            self.error(&format!("无法打开文件。\n\n{e}"));
                        }
                    }
                }
                Reply::Page(id, anchor, result) if id == self.io_ticket => {
                    self.loading = false;
                    let pending = self.pending_scroll.take();
                    match result {
                        Ok(page) => {
                            self.page = Some(page);
                            self.status = "就绪".into();
                            self.relayout(anchor);
                            if let Some((wrapped_row, delta)) = pending {
                                self.position(anchor, wrapped_row);
                                self.scroll(delta);
                            }
                        }
                        Err(e) => {
                            self.status = format!("读取失败：{e}");
                        }
                    }
                }
                Reply::Search(id, result)
                    if id == self.search_ticket
                        && id == self.workers.epoch.load(Ordering::Relaxed) =>
                {
                    match result {
                        Ok(SearchResult::Found(offset)) => {
                            self.found = Some(offset);
                            self.status = format!("找到匹配 · 字节位置 {offset}");
                            self.jump(offset);
                        }
                        Ok(SearchResult::NotFound) => {
                            self.status = "未找到匹配（区分大小写）".into();
                        }
                        Ok(SearchResult::Cancelled) => {}
                        Err(e) => self.status = format!("搜索失败：{e}"),
                    }
                }
                Reply::Outline(id, result) if id == self.outline_ticket => {
                    self.outline_loading = false;
                    match result {
                        Ok(outline) => self.outline = Some(outline),
                        Err(error) => self.outline_error = Some(error),
                    }
                    self.outline_top = 0;
                }
                _ => continue,
            }
            self.invalidate();
        }
    }
    unsafe fn jump(&mut self, anchor: u64) {
        if let Some(page) = &self.page
            && anchor >= page.start
            && anchor < page.end
        {
            // A drag can return to the cached page while an earlier seek is in flight.
            // Invalidate that reply so it cannot move the reader back after release.
            self.io_ticket += 1;
            self.loading = false;
            self.pending_scroll = None;
            self.anchor = anchor;
            self.position(anchor, 0);
            self.selection = None;
            self.scrollbar();
            self.invalidate();
            return;
        }
        self.request_page(anchor);
    }
    fn viewport(&self) -> Viewport<'_, Row> {
        Viewport::new(
            self.layout.as_ref().map_or(&[], |l| l.rows.as_slice()),
            (self.content_bottom() - self.content_top() - self.s(12)).max(1),
        )
    }
    fn clamp_top(&mut self) {
        if self.page.as_ref().is_some_and(|p| p.end >= p.size) {
            self.top = self.top.min(self.viewport().bottom_top());
        }
    }
    fn position(&mut self, anchor: u64, wrapped_row: usize) {
        if let Some(l) = &self.layout {
            let first = l.rows.partition_point(|r| r.end <= anchor);
            self.top = first.min(l.rows.len().saturating_sub(1));
            if let Some(row) = l.rows.get(self.top) {
                let count = l.rows[self.top..]
                    .iter()
                    .take_while(|r| r.start == row.start)
                    .count();
                self.top += wrapped_row.min(count.saturating_sub(1));
            }
        }
        if self
            .page
            .as_ref()
            .is_some_and(|p| p.end >= p.size && anchor >= p.size.saturating_sub(1))
        {
            // The final physical line can wrap into many rows. End targets its tail,
            // not the first row sharing that source range.
            self.top = self.viewport().bottom_top();
        } else {
            self.clamp_top();
        }
    }
    fn wrapped_row(&self, index: usize) -> usize {
        self.layout
            .as_ref()
            .and_then(|l| {
                let row = l.rows.get(index)?;
                Some(index - l.rows.partition_point(|r| r.end <= row.start))
            })
            .unwrap_or(0)
    }
    unsafe fn page_scroll(&mut self, forward: bool) {
        let next = self.viewport().page_top(self.top, forward);
        let delta = next as i32 - self.top as i32;
        self.scroll(if !forward && delta == 0 { -1 } else { delta });
    }
    unsafe fn scroll(&mut self, delta: i32) {
        if self.loading {
            return;
        }
        let Some(l) = &self.layout else {
            return;
        };
        if l.rows.is_empty() {
            return;
        }
        if delta < 0
            && self.top < delta.unsigned_abs() as usize
            && self.page.as_ref().is_some_and(|p| p.start > 3)
        {
            // Refill behind the current row, then apply the original row movement.
            // Subtracting an arbitrary byte count would skip short lines.
            let current = self.current();
            let wrapped_row = self.wrapped_row(self.top);
            self.request_page(current);
            self.pending_scroll = Some((wrapped_row, delta));
            return;
        }
        let requested = self.top as i64 + delta as i64;
        if delta > 0
            && requested >= l.rows.len() as i64
            && let Some(page) = &self.page
            && page.end < page.size
        {
            self.request_page(page.end);
            return;
        }
        let next = requested.clamp(0, l.rows.len().saturating_sub(1) as i64) as usize;
        if delta > 0
            && next + 64 >= l.rows.len()
            && self.page.as_ref().is_some_and(|p| p.end < p.size)
        {
            let anchor = l.rows[next].start;
            let wrapped_row = self.wrapped_row(next);
            self.request_page(anchor);
            self.pending_scroll = Some((wrapped_row, 0));
            return;
        }
        self.top = next;
        self.clamp_top();
        self.anchor = self.current();
        self.scrollbar();
        self.invalidate();
    }
    fn horizontal_max(&self) -> i32 {
        if self.settings.wrap {
            0
        } else {
            self.layout
                .as_ref()
                .map_or(0, |l| (l.max_width - self.body_width()).max(0))
        }
    }
    unsafe fn scrollbar(&mut self) {
        self.horizontal = self.horizontal.clamp(0, self.horizontal_max());
    }
    fn scroll_bar(&self, axis: Axis) -> Option<Bar> {
        let layout = self.layout.as_ref()?;
        let page = self.page.as_ref()?;
        let (track, maximum, visible, position, pixels) = match axis {
            Axis::Horizontal => (
                RECT {
                    left: self.left(),
                    top: self.height - self.s(46),
                    right: self.left() + self.body_width(),
                    bottom: self.height - self.s(32),
                },
                self.horizontal_max() as u64,
                self.body_width() as u64,
                self.horizontal as u64,
                true,
            ),
            Axis::Vertical => {
                let track = RECT {
                    left: self.width - self.s(20),
                    top: self.content_top() + self.s(12),
                    right: self.width - self.s(5),
                    bottom: self.content_bottom(),
                };
                let viewport = self.viewport();
                let (maximum, visible, position, pixels) = if page.start <= 3
                    && page.end >= page.size
                {
                    let sum = |end: usize| {
                        layout.rows[..end.min(layout.rows.len())]
                            .iter()
                            .map(|r| r.height.max(1) as u64)
                            .sum::<u64>()
                    };
                    (
                        sum(viewport.bottom_top()),
                        (track.bottom - track.top).max(1) as u64,
                        sum(self.top),
                        true,
                    )
                } else {
                    let maximum = page.size.saturating_sub(1);
                    let at_end = page.end >= page.size && self.top >= viewport.bottom_top();
                    let position = if at_end {
                        maximum
                    } else {
                        layout.rows.get(self.top).map_or(self.current(), |row| {
                            let wrapped = self.wrapped_row(self.top);
                            let count = layout
                                .rows
                                .iter()
                                .filter(|r| r.start == row.start)
                                .count()
                                .max(1);
                            row.start
                                + (row.end.saturating_sub(row.start) as u128 * wrapped as u128
                                    / count as u128) as u64
                        })
                    };
                    let visible = layout
                        .rows
                        .get(viewport.visible_end(self.top).saturating_sub(1))
                        .map_or(1, |r| r.end.saturating_sub(self.current()).max(1));
                    (maximum, visible, position, false)
                };
                (track, maximum, visible, position, pixels)
            }
        };
        if maximum == 0 || track.bottom <= track.top || track.right <= track.left {
            return None;
        }
        let (start, length) = match axis {
            Axis::Vertical => (track.top, track.bottom - track.top),
            Axis::Horizontal => (track.left, track.right - track.left),
        };
        let scale = ScrollTrack::new(start, length, maximum, visible, position, self.s(32));
        let thumb = match axis {
            Axis::Vertical => RECT {
                top: scale.thumb_start,
                bottom: scale.thumb_start + scale.thumb_length,
                ..track
            },
            Axis::Horizontal => RECT {
                left: scale.thumb_start,
                right: scale.thumb_start + scale.thumb_length,
                ..track
            },
        };
        Some(Bar {
            track,
            thumb,
            scale,
            pixels,
        })
    }
    unsafe fn drag_scrollbar(&mut self, x: i32, y: i32) {
        let Some(drag) = self.drag else {
            return;
        };
        let Some(bar) = self.scroll_bar(drag.axis) else {
            return;
        };
        let pointer = if drag.axis == Axis::Vertical { y } else { x };
        let value = bar.scale.value_at(pointer, drag.grab);
        if drag.axis == Axis::Horizontal {
            self.horizontal = value as i32;
        } else if bar.pixels {
            if let Some(layout) = &self.layout {
                let mut used = 0;
                self.top = 0;
                for row in layout.rows.iter().take(self.viewport().bottom_top()) {
                    let next = used + row.height.max(1) as u64;
                    if next > value {
                        break;
                    }
                    used = next;
                    self.top += 1;
                }
                self.anchor = self.current();
                self.selection = None;
            }
        } else {
            self.jump(value);
        }
        self.scrollbar();
        self.invalidate();
    }
    unsafe fn begin_scrollbar(&mut self, x: i32, y: i32) -> bool {
        for axis in [Axis::Vertical, Axis::Horizontal] {
            let Some(bar) = self.scroll_bar(axis) else {
                continue;
            };
            if !contains(bar.track, x, y) {
                continue;
            }
            if contains(bar.thumb, x, y) {
                let grab = if axis == Axis::Vertical {
                    y - bar.thumb.top
                } else {
                    x - bar.thumb.left
                };
                self.drag = Some(ScrollDrag { axis, grab });
                SetCapture(self.hwnd);
            } else if axis == Axis::Vertical {
                self.page_scroll(y >= bar.thumb.bottom);
            } else {
                self.horizontal += if x < bar.thumb.left {
                    -self.body_width()
                } else {
                    self.body_width()
                };
                self.scrollbar();
            }
            self.invalidate();
            return true;
        }
        false
    }
    unsafe fn persist(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            self.status = format!("设置未能保存：{e}");
        }
        self.update_checks();
        let a = self.current();
        self.relayout(a);
    }
    unsafe fn update_checks(&self) {
        for (id, checked) in [
            (WRAP, self.settings.wrap),
            (RAW, self.settings.raw),
            (WIDE, self.settings.wide),
            (OUTLINE, self.settings.outline),
        ] {
            CheckMenuItem(
                self.menu,
                id as u32,
                MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
            );
        }
    }
    unsafe fn error(&self, text: &str) {
        MessageBoxW(
            self.hwnd,
            w(text).as_ptr(),
            w("FeatherMD").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
    unsafe fn dialog(&mut self) {
        let mut file = vec![0u16; 32768];
        let mut ofn: OPENFILENAMEW = zeroed();
        let filter = w("Markdown / Text\0*.md;*.markdown;*.mdown;*.mkd;*.txt\0所有文件\0*.*\0");
        ofn.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut ofn) != 0 {
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            self.open(PathBuf::from(OsString::from_wide(&file[..len])));
        }
        self.invalidate();
    }
    unsafe fn export_licenses(&mut self) {
        let mut file = vec![0u16; 32768];
        let name = w("FeatherMD-LICENSES.txt");
        file[..name.len()].copy_from_slice(&name);
        let mut dialog: OPENFILENAMEW = zeroed();
        let filter = w("Text\0*.txt\0");
        let extension = w("txt");
        dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = self.hwnd;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = file.as_mut_ptr();
        dialog.nMaxFile = file.len() as u32;
        dialog.lpstrDefExt = extension.as_ptr();
        dialog.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
        if GetSaveFileNameW(&mut dialog) != 0 {
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            let path = PathBuf::from(OsString::from_wide(&file[..len]));
            let text = concat!(
                include_str!("../LICENSE"),
                "\n\n",
                include_str!("../THIRD_PARTY_NOTICES.md")
            );
            match std::fs::write(path, text) {
                Ok(()) => self.status = "已导出内置许可证".into(),
                Err(e) => self.error(&e.to_string()),
            }
        }
    }
    unsafe fn position_edit(&self, _mode: usize) {
        let left = (self.sidebar_width() + self.s(24)).max(self.s(48));
        MoveWindow(
            self.edit,
            left,
            self.s(60),
            (self.width - left - self.s(164)).max(self.s(80)),
            self.s(32),
            1,
        );
    }
    unsafe fn show_edit(&mut self, mode: usize) {
        self.edit_mode = Some(mode);
        self.position_edit(mode);
        ShowWindow(self.edit, SW_SHOW);
        SetWindowTextW(
            self.edit,
            w(if mode == GOTO { "50" } else { &self.query }).as_ptr(),
        );
        SetFocus(self.edit);
        SendMessageW(self.edit, 0x00B1, 0, -1);
        self.clamp_top();
        self.invalidate();
    }
    unsafe fn hide_edit(&mut self) {
        self.workers.cancel_search();
        self.edit_mode = None;
        ShowWindow(self.edit, SW_HIDE);
        SetFocus(self.hwnd);
        self.status = "就绪".into();
        self.clamp_top();
        self.invalidate();
    }
    unsafe fn submit(&mut self) {
        let len = GetWindowTextLengthW(self.edit).clamp(0, 16384) as usize;
        let mut buf = vec![0u16; len + 1];
        GetWindowTextW(self.edit, buf.as_mut_ptr(), buf.len() as i32);
        let text = String::from_utf16_lossy(&buf[..len]);
        if self.edit_mode == Some(GOTO) {
            match text.trim().trim_end_matches('%').parse::<f64>() {
                Ok(value) if value.is_finite() && (0.0..=100.0).contains(&value) => {
                    let size = self.page.as_ref().map_or(0, |p| p.size);
                    self.hide_edit();
                    self.jump(((size as f64 * value / 100.0) as u64).min(size.saturating_sub(1)));
                }
                _ => {
                    self.status = "请输入 0～100 之间的百分比".into();
                    self.invalidate();
                }
            }
        } else {
            if text != self.query {
                self.found = None;
            }
            self.query = text;
            self.search();
        }
    }
    unsafe fn search(&mut self) {
        if self.query.is_empty() {
            return;
        }
        if let Some(doc) = &self.document {
            self.search_ticket = self.workers.cancel_search();
            let from = self.found.map_or(self.current(), |n| n.saturating_add(1));
            let _ = self.workers.search.send(SearchRequest {
                document: doc.clone(),
                query: self.query.clone(),
                from,
                ticket: self.search_ticket,
            });
            self.status = "正在后台搜索…（Esc 取消，区分大小写）".into();
            self.invalidate();
        }
    }
    unsafe fn copy(&mut self) {
        let Some(layout) = &self.layout else {
            return;
        };
        let range = self.selection.unwrap_or((
            self.top,
            (self.top + self.visible_rows().saturating_sub(1))
                .min(layout.rows.len().saturating_sub(1)),
        ));
        let (a, b) = (range.0.min(range.1), range.0.max(range.1));
        let mut text = String::new();
        for row in layout.rows.get(a..=b).unwrap_or(&[]) {
            for run in &row.runs {
                text.push_str(&String::from_utf16_lossy(&run.text));
            }
            text.push_str("\r\n");
        }
        let data = w(&text);
        if OpenClipboard(self.hwnd) == 0 {
            self.status = "剪贴板暂不可用，请重试".into();
            return;
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
        if !memory.is_null() {
            let dest = GlobalLock(memory);
            if !dest.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), dest.cast(), data.len());
                GlobalUnlock(memory);
                if EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null() {
                    self.status = "已复制选中行（未选择时复制可见内容）".into();
                } else {
                    GlobalFree(memory);
                }
            } else {
                GlobalFree(memory);
            }
        }
        CloseClipboard();
        self.invalidate();
    }
    fn visible_rows(&self) -> usize {
        self.viewport()
            .visible_end(self.top)
            .saturating_sub(self.top)
    }
    #[allow(
        clippy::collapsible_match,
        reason = "Keep user-confirmation side effects inside the selected command"
    )]
    unsafe fn command(&mut self, id: usize) {
        match id {
            OUTLINE => self.toggle_outline(),
            MINIMIZE => { PostMessageW(self.hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0); },
            MAXIMIZE => { PostMessageW(self.hwnd, WM_SYSCOMMAND, if IsZoomed(self.hwnd) != 0 { SC_RESTORE } else { SC_MAXIMIZE } as usize, 0); },
            PREVIEW | SOURCE => { self.settings.raw = id == SOURCE; self.persist(); },
            FILE_MENU | HELP_MENU => {
                let menu = GetSubMenu(self.menu, if id == FILE_MENU { 0 } else { 2 });
                let mut point = POINT { x: self.s(20), y: self.s(94) };
                ClientToScreen(self.hwnd, &mut point);
                let selected = TrackPopupMenu(menu, TPM_RETURNCMD, point.x, point.y, 0, self.hwnd, null());
                if selected > 0 { self.command(selected as usize); }
            },
            LICENSES=>self.export_licenses(),
            OPEN=>self.dialog(), FIND|GOTO=>self.show_edit(id), NEXT=>self.search(), COPY=>self.copy(), EXIT=>{DestroyWindow(self.hwnd);},
            RELOAD=>{if let Some(doc)=&self.document {let p=doc.path.clone();self.open(p);}},
            THEME=>{self.settings.theme=match self.settings.theme {Theme::Light=>Theme::Dark,Theme::Dark=>Theme::Sepia,Theme::Sepia=>Theme::Light};self.persist();},
            ZOOM_IN=>{self.settings.font_size=(self.settings.font_size+2).min(36);self.persist();},
            ZOOM_OUT=>{self.settings.font_size=(self.settings.font_size-2).max(12);self.persist();},
            WRAP=>{self.settings.wrap= !self.settings.wrap;self.persist();}, RAW=>{self.settings.raw= !self.settings.raw;self.persist();}, WIDE=>{self.settings.wide= !self.settings.wide;self.persist();},
            RESET=>{self.settings=Settings::default();self.persist();self.start_outline();},
            SETTINGS=>{let mut point=POINT{x:self.width-self.s(145),y:self.s(46)};ClientToScreen(self.hwnd,&mut point);let id=TrackPopupMenu(self.settings_menu,TPM_RIGHTALIGN|TPM_RETURNCMD,point.x,point.y,0,self.hwnd,null());if id>0{self.command(id as usize);}},
            REGISTER=>{
                if MessageBoxW(self.hwnd,w("将 FeatherMD 添加到 Windows 的打开方式列表。\n不会强行更改当前默认应用；下一步由你在系统设置中选择 .md → FeatherMD。\n\n建议先把 EXE 放在固定位置。是否继续？").as_ptr(),w("注册 Markdown 查看器").as_ptr(),MB_OKCANCEL|MB_ICONINFORMATION)==IDOK {
                    match std::env::current_exe().and_then(|p|crate::association::register(&p)) { Ok(())=>{self.status="已注册，请在系统默认应用中选择 FeatherMD".into();ShellExecuteW(self.hwnd,w("open").as_ptr(),w("ms-settings:defaultapps").as_ptr(),null(),null(),SW_SHOWNORMAL);},Err(e)=>self.error(&e.to_string()) }
                }
            },
            UNREGISTER=>{if MessageBoxW(self.hwnd,w("移除 FeatherMD 的打开方式注册？不会删除文件或其他应用的关联。").as_ptr(),w("移除注册").as_ptr(),MB_OKCANCEL|MB_ICONQUESTION)==IDOK {match crate::association::unregister(){Ok(())=>self.status="已移除打开方式注册".into(),Err(e)=>self.error(&e.to_string())}}},
            ABOUT=>{MessageBoxW(self.hwnd,w(concat!("FeatherMD 羽读  ",env!("CARGO_PKG_VERSION"),"\n\n原生 Rust / Win32 · 单文件 · 无联网请求\n\nCtrl+O 打开　Ctrl+F 搜索　F3 下一个\nCtrl+G 百分比跳转　F5 重新载入\nCtrl+鼠标滚轮 缩放　Ctrl+R 源码模式\n拖选整行，Ctrl+C 复制；Ctrl+A 选择当前分页\n\n采用有界 UTF-8 分页预览，不做全文件预解析。\n随机跳转后的跨页代码围栏 / 列表上下文可能不完整，\n可切换源码模式。图片显示替代文本，不执行 HTML。\n\nLianzy-Baimiao / FeatherMD · MIT License")).as_ptr(),w("关于 / 快捷键").as_ptr(),MB_OK|MB_ICONINFORMATION);},
            _=>{}
        }
        self.invalidate();
    }
    unsafe fn keyboard(&mut self, key: usize, ctrl: bool) -> bool {
        if ctrl {
            match key {
                0x4f => self.command(OPEN),
                0x46 => self.command(FIND),
                0x47 => self.command(GOTO),
                0x43 => self.command(COPY),
                0x52 => self.command(RAW),
                0x42 => self.command(OUTLINE),
                0x41 => {
                    if let Some(l) = &self.layout {
                        self.selection = Some((0, l.rows.len().saturating_sub(1)));
                        self.invalidate();
                    }
                }
                0xbb | 0x6b => self.command(ZOOM_IN),
                0xbd | 0x6d => self.command(ZOOM_OUT),
                0x30 => self.command(RESET),
                0x24 => self.jump(0),
                0x23 => self.jump(self.page.as_ref().map_or(0, |p| p.size.saturating_sub(1))),
                _ => return false,
            }
            return true;
        }
        match key {
            0x26 => self.scroll(-1),
            0x28 => self.scroll(1),
            0x21 => self.page_scroll(false),
            0x22 | 0x20 => self.page_scroll(true),
            0x24 => self.jump(0),
            0x23 => self.jump(self.page.as_ref().map_or(0, |p| p.size.saturating_sub(1))),
            0x25 => {
                self.horizontal = (self.horizontal - self.s(60)).max(0);
                self.scrollbar();
                self.invalidate();
            }
            0x27 => {
                self.horizontal += self.s(60);
                self.scrollbar();
                self.invalidate();
            }
            0x72 => self.command(NEXT),
            0x74 => self.command(RELOAD),
            0x70 => self.command(ABOUT),
            0x1b => self.hide_edit(),
            _ => return false,
        }
        true
    }

    unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
        let brush = CreateSolidBrush(color);
        FillRect(dc, rect, brush);
        DeleteObject(brush);
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Small GDI drawing primitive: position, font, color and flags are independent"
    )]
    unsafe fn label(
        &mut self,
        dc: HDC,
        text: &str,
        rect: RECT,
        size: i32,
        bold: bool,
        color: u32,
        flags: u32,
    ) {
        // DrawTextW may inspect the first UTF-16 code unit even with a zero count.
        // Icon-only controls have no label and must not pass a dangling empty Vec.
        if text.is_empty() {
            return;
        }
        let old = SelectObject(
            dc,
            self.fonts.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
            }),
        );
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        let mut r = rect;
        let text: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(dc, text.as_ptr(), text.len() as i32, &mut r, flags);
        SelectObject(dc, old);
    }
    unsafe fn paint(&mut self, supplied: Option<HDC>) {
        let mut ps: PAINTSTRUCT = zeroed();
        let target = supplied.unwrap_or_else(|| BeginPaint(self.hwnd, &mut ps));
        let dc = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, self.width.max(1), self.height.max(1));
        let old_bitmap = SelectObject(dc, bitmap);
        let p = Palette::new(self.settings.theme);
        let all = RECT {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        };
        Self::fill(dc, &all, p.bg);
        self.hits.clear();
        SetBkMode(dc, TRANSPARENT as i32);
        self.paint_header(dc, p);
        self.paint_outline(dc, p);
        if self.document.is_none() {
            self.paint_welcome(dc, p);
        } else if self.page.as_ref().is_some_and(|p| p.size == 0) {
            self.label(
                dc,
                "这是一份空白文档。",
                RECT {
                    left: self.left(),
                    top: self.content_top() + self.s(45),
                    right: self.width,
                    bottom: self.height - self.s(50),
                },
                24,
                false,
                p.muted,
                DT_SINGLELINE,
            );
        } else {
            let left = self.left();
            let body_width = self.body_width();
            let mut y = self.content_top() + self.s(12);
            let bottom = self.content_bottom();
            if let Some(layout) = &self.layout {
                let saved = SaveDC(dc);
                IntersectClipRect(dc, left, y, left + body_width, bottom);
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
                    let selected = self
                        .selection
                        .is_some_and(|(a, b)| index >= a.min(b) && index <= a.max(b));
                    let found = self.found.is_some_and(|n| n >= row.start && n < row.end);
                    if selected || found {
                        Self::fill(dc, &rect, p.selection);
                    } else if matches!(
                        row.kind,
                        markdown::Kind::Code
                            | markdown::Kind::Fence
                            | markdown::Kind::Table
                            | markdown::Kind::Quote
                    ) {
                        Self::fill(dc, &rect, p.code);
                    }
                    if row.kind == markdown::Kind::Quote {
                        Self::fill(
                            dc,
                            &RECT {
                                right: left + self.s(3),
                                ..rect
                            },
                            p.accent,
                        );
                    }
                    if row.kind == markdown::Kind::Rule {
                        Self::fill(
                            dc,
                            &RECT {
                                left,
                                top: y + row.height / 2,
                                right: left + body_width,
                                bottom: y + row.height / 2 + 1,
                            },
                            p.edge,
                        );
                    }
                    self.hits.push(HitBox {
                        rect,
                        action: Hit::Row(index),
                    });
                    for run in &row.runs {
                        let x = left + run.x - self.horizontal;
                        let text_y = y + (row.height - self.fonts.scale(run.font.size)) / 2 - 1;
                        let old = SelectObject(dc, self.fonts.get(run.font));
                        if run.code && !selected && !found {
                            Self::fill(
                                dc,
                                &RECT {
                                    left: x - self.s(2),
                                    top: text_y - self.s(1),
                                    right: x + run.width + self.s(2),
                                    bottom: y + row.height - self.s(3),
                                },
                                p.code,
                            );
                        }
                        SetTextColor(
                            dc,
                            if run.link.is_some() {
                                p.accent
                            } else if row.kind == markdown::Kind::Fence {
                                p.muted
                            } else {
                                p.fg
                            },
                        );
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
                    if matches!(row.kind, markdown::Kind::Heading(1 | 2)) {
                        Self::fill(
                            dc,
                            &RECT {
                                left,
                                top: y + row.height - 1,
                                right: left + body_width,
                                bottom: y + row.height,
                            },
                            p.edge,
                        );
                    }
                    y += row.height;
                }
                RestoreDC(dc, saved);
            }
        }
        self.paint_footer(dc, p);
        self.paint_scrollbars(dc, p);
        BitBlt(target, 0, 0, self.width, self.height, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        if supplied.is_none() {
            EndPaint(self.hwnd, &ps);
        }
    }
    // Opt-in integration-test capture: all HDCs stay in this process (cross-process
    // hidden-window PrintWindow is unreliable on some Windows/DWM versions).
    unsafe fn test_snapshot(&mut self) -> bool {
        let Some(path) = std::env::var_os("FEATHERMD_TEST_SNAPSHOT") else {
            return false;
        };
        let screen = GetDC(self.hwnd);
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, self.width, self.height);
        let old = SelectObject(dc, bitmap);
        self.paint(Some(dc));
        if self.edit_mode.is_some() {
            let mut rect = RECT::default();
            GetWindowRect(self.edit, &mut rect);
            let mut origin = POINT {
                x: rect.left,
                y: rect.top,
            };
            ScreenToClient(self.hwnd, &mut origin);
            let saved = SaveDC(dc);
            SetViewportOrgEx(dc, origin.x, origin.y, null_mut());
            SendMessageW(
                self.edit,
                WM_PRINT,
                dc as usize,
                (PRF_CLIENT | PRF_NONCLIENT | PRF_ERASEBKGND) as isize,
            );
            RestoreDC(dc, saved);
        }
        SelectObject(dc, old);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = self.width;
        info.bmiHeader.biHeight = -self.height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; self.width as usize * self.height as usize * 4];
        let read = GetDIBits(
            screen,
            bitmap,
            0,
            self.height as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(self.hwnd, screen);
        if read == 0 {
            return false;
        }
        let mut out = Vec::with_capacity(pixels.len() + 54);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&((pixels.len() + 54) as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]);
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&(-self.height).to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&[0u8; 24]);
        out.extend_from_slice(&pixels);
        std::fs::write(path, out).is_ok()
    }
    fn hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|h| x >= h.rect.left && x < h.rect.right && y >= h.rect.top && y < h.rect.bottom)
            .map(|h| h.action.clone())
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // The entire window is our client surface; Windows still owns move/resize/snap.
    if msg == WM_NCCALCSIZE || msg == WM_NCPAINT {
        return 0;
    }
    if msg == WM_NCACTIVATE {
        return 1;
    }
    if msg == WM_DESTROY {
        KillTimer(hwnd, 1);
        PostQuitMessage(0);
        return 0;
    }
    if msg == WM_NCCREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<App>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    // Modal Win32 calls may re-enter this procedure. Never alias a mutable App reference.
    let Ok(mut app) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, wp, lp);
    };
    app.hwnd = hwnd;
    match msg {
        m if m == WM_APP + 42 => match wp {
            0 => app.width as isize,
            1 => app.height as isize,
            2 => app.layout.as_ref().map_or(0, |l| l.rows.len() as isize),
            3 => app.fonts.dpi as isize,
            4 => app.test_snapshot() as isize,
            5 => app.found.map_or(-1, |n| n as isize),
            6 => app.query.len() as isize,
            7 => app.edit as isize,
            8 => app.search_ticket as isize,
            9 => app.current() as isize,
            10 => app.page.as_ref().map_or(0, |p| p.size as isize),
            11 => app.loading as isize,
            12 => app.settings.font_size as isize,
            13 => app.top as isize,
            14 => app.wrapped_row(app.top) as isize,
            18 => app.horizontal as isize,
            19..=24 => app.scroll_bar(Axis::Vertical).map_or(-1, |bar| match wp {
                19 => bar.track.left,
                20 => bar.track.top,
                21 => bar.track.right,
                22 => bar.track.bottom,
                23 => bar.thumb.top,
                _ => bar.thumb.bottom,
            } as isize),
            25..=30 => app.scroll_bar(Axis::Horizontal).map_or(-1, |bar| match wp {
                25 => bar.track.left,
                26 => bar.track.top,
                27 => bar.track.right,
                28 => bar.track.bottom,
                29 => bar.thumb.left,
                _ => bar.thumb.right,
            } as isize),
            31 => app.horizontal_max() as isize,
            32 => app.settings.outline as isize,
            33 => app.outline_loading as isize,
            34 => app
                .outline
                .as_ref()
                .map_or(0, |o| o.headings.len() as isize),
            35 => app.sidebar_width() as isize,
            36 => app.outline_top as isize,
            37 => app
                .outline
                .as_ref()
                .and_then(|o| o.headings.get(lp as usize))
                .map_or(-1, |h| h.offset as isize),
            17 => app
                .hits
                .iter()
                .rev()
                .find_map(|h| match h.action {
                    Hit::Row(i) => Some(app.wrapped_row(i) as isize),
                    _ => None,
                })
                .unwrap_or(-1),
            15 => app
                .hits
                .iter()
                .rev()
                .find_map(|h| match h.action {
                    Hit::Row(i) => app.layout.as_ref()?.rows.get(i).map(|r| r.start as isize),
                    _ => None,
                })
                .unwrap_or(-1),
            16 => app
                .hits
                .iter()
                .filter(|h| matches!(h.action, Hit::Row(_)))
                .count() as isize,
            _ => 0,
        },
        WM_ERASEBKGND => 1,
        WM_PRINTCLIENT | WM_PRINT => {
            app.paint(Some(wp as HDC));
            0
        }
        WM_PAINT => {
            app.paint(None);
            0
        }
        WM_TIMER => {
            app.receive();
            0
        }
        WM_SIZE => {
            if wp != SIZE_MINIMIZED as usize {
                app.width = (lp as u32 & 0xffff) as i32;
                app.height = ((lp as u32 >> 16) & 0xffff) as i32;
                app.drag = None;
                app.hover = None;
                app.pressed = None;
                let anchor = app.current();
                let wrapped = app.wrapped_row(app.top);
                app.relayout(anchor);
                app.position(anchor, wrapped);
                if let Some(mode) = app.edit_mode {
                    app.position_edit(mode);
                }
            }
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lp as *mut MINMAXINFO);
            info.ptMinTrackSize = POINT {
                x: app.s(640),
                y: app.s(480),
            };
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut monitor_info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..zeroed()
            };
            if GetMonitorInfoW(monitor, &mut monitor_info) != 0 {
                let work = monitor_info.rcWork;
                info.ptMaxPosition = POINT {
                    x: work.left - monitor_info.rcMonitor.left,
                    y: work.top - monitor_info.rcMonitor.top,
                };
                info.ptMaxSize = POINT {
                    x: work.right - work.left,
                    y: work.bottom - work.top,
                };
            }
            0
        }
        WM_NCHITTEST => {
            let mut point = POINT {
                x: lp as u16 as i16 as i32,
                y: (lp >> 16) as u16 as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            let (x, y) = (point.x, point.y);
            let edge = app.s(6);
            if IsZoomed(hwnd) == 0 {
                let left = x < edge;
                let right = x >= app.width - edge;
                let top = y < edge;
                let bottom = y >= app.height - edge;
                let hit = match (left, right, top, bottom) {
                    (true, _, true, _) => HTTOPLEFT,
                    (_, true, true, _) => HTTOPRIGHT,
                    (true, _, _, true) => HTBOTTOMLEFT,
                    (_, true, _, true) => HTBOTTOMRIGHT,
                    (true, _, _, _) => HTLEFT,
                    (_, true, _, _) => HTRIGHT,
                    (_, _, true, _) => HTTOP,
                    (_, _, _, true) => HTBOTTOM,
                    _ => HTCLIENT,
                };
                if hit != HTCLIENT {
                    return hit as isize;
                }
            }
            if y < app.s(48) && !matches!(app.hit(x, y), Some(Hit::Command(_))) {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_DPICHANGED => {
            let anchor = app.current();
            app.fonts = Fonts::new((wp & 0xffff) as u32);
            let r = &*(lp as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            app.width = rect.right;
            app.height = rect.bottom;
            let font = app.fonts.get(FontKey {
                size: 16,
                bold: false,
                italic: false,
                mono: false,
                strike: false,
            });
            SendMessageW(app.edit, WM_SETFONT, font as usize, 1);
            app.relayout(anchor);
            if let Some(mode) = app.edit_mode {
                app.position_edit(mode);
            }
            0
        }
        WM_COMMAND => {
            app.command(wp & 0xffff);
            0
        }
        WM_KEYDOWN => {
            if app.keyboard(wp, GetKeyState(VK_CONTROL as i32) < 0) {
                0
            } else {
                drop(app);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp >> 16) as u16 as i16) as i32;
            app.wheel_remainder += delta;
            let steps = app.wheel_remainder / 120;
            app.wheel_remainder %= 120;
            if steps == 0 {
                return 0;
            }
            let mut point = POINT {
                x: lp as u16 as i16 as i32,
                y: (lp >> 16) as u16 as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            if point.x >= 0 && point.x < app.sidebar_width() && point.y >= app.s(48) {
                app.scroll_outline(-steps * 3);
                return 0;
            }
            if GetKeyState(VK_CONTROL as i32) < 0 {
                app.command(if steps > 0 { ZOOM_IN } else { ZOOM_OUT });
            } else if GetKeyState(VK_SHIFT as i32) < 0 {
                app.horizontal -= steps * app.s(60);
                app.scrollbar();
                app.invalidate();
            } else {
                app.scroll(-steps * 3);
            }
            0
        }
        WM_MOUSEHWHEEL => {
            app.horizontal_wheel_remainder += ((wp >> 16) as u16 as i16) as i32;
            let steps = app.horizontal_wheel_remainder / 120;
            app.horizontal_wheel_remainder %= 120;
            app.horizontal += steps * app.s(60);
            app.scrollbar();
            app.invalidate();
            0
        }
        WM_VSCROLL => {
            match (wp & 0xffff) as i32 {
                SB_LINEUP => app.scroll(-1),
                SB_LINEDOWN => app.scroll(1),
                SB_PAGEUP => app.page_scroll(false),
                SB_PAGEDOWN => app.page_scroll(true),
                SB_TOP => app.jump(0),
                SB_BOTTOM => {
                    let n = app.page.as_ref().map_or(0, |p| p.size.saturating_sub(1));
                    app.jump(n);
                }
                _ => {}
            }
            0
        }
        WM_HSCROLL => {
            match (wp & 0xffff) as i32 {
                SB_LINELEFT => app.horizontal -= app.s(40),
                SB_LINERIGHT => app.horizontal += app.s(40),
                SB_PAGELEFT => app.horizontal -= app.body_width(),
                SB_PAGERIGHT => app.horizontal += app.body_width(),
                SB_LEFT => app.horizontal = 0,
                SB_RIGHT => app.horizontal = app.horizontal_max(),
                _ => {}
            }
            app.scrollbar();
            app.invalidate();
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            SetFocus(hwnd);
            let x = lp as u16 as i16 as i32;
            let y = (lp >> 16) as u16 as i16 as i32;
            app.pressed = None;
            if app.begin_scrollbar(x, y) {
                return 0;
            }
            app.pressed = app.hit(x, y);
            if matches!(
                app.pressed,
                Some(Hit::Command(_) | Hit::Link(_) | Hit::Heading(_))
            ) {
                SetCapture(hwnd);
                app.invalidate();
            }
            if let Some(Hit::Row(row)) = app.hit(x, y) {
                app.selection = Some((row, row));
                app.selecting = true;
                SetCapture(hwnd);
                app.invalidate();
            }
            0
        }
        WM_MOUSEMOVE => {
            let x = lp as u16 as i16 as i32;
            let y = (lp >> 16) as u16 as i16 as i32;
            let hover = if let Some(Hit::Command(id)) = app.hit(x, y) {
                Some(id)
            } else {
                None
            };
            let previous_mouse = app.mouse;
            app.mouse = Some((x, y));
            if app.hover != hover || previous_mouse != app.mouse {
                app.hover = hover;
                app.invalidate();
            }
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            if app.drag.is_some() {
                app.drag_scrollbar(x, y);
            } else if app.selecting {
                let row = app.hits.iter().find_map(|h| {
                    if y >= h.rect.top && y < h.rect.bottom {
                        if let Hit::Row(i) = h.action {
                            Some(i)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                if let (Some((start, _)), Some(end)) = (app.selection, row) {
                    app.selection = Some((start, end));
                    app.invalidate();
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let x = lp as u16 as i16 as i32;
            let y = (lp >> 16) as u16 as i16 as i32;
            let pressed = app.pressed.take();
            if app.drag.is_some() {
                app.drag_scrollbar(x, y);
                app.drag = None;
                ReleaseCapture();
                app.invalidate();
            } else if app.selecting {
                app.selecting = false;
                ReleaseCapture();
            } else {
                ReleaseCapture();
                let hit = app.hit(x, y);
                if pressed != hit {
                    app.invalidate();
                    return 0;
                }
                match hit {
                    Some(Hit::Heading(index)) => {
                        if let Some(heading) =
                            app.outline.as_ref().and_then(|o| o.headings.get(index))
                        {
                            let offset = heading.offset;
                            app.jump(offset);
                        }
                    }
                    Some(Hit::Command(0)) => app.hide_edit(),
                    Some(Hit::Command(NEXT)) if app.edit_mode.is_some() => app.submit(),
                    Some(Hit::Command(id)) => app.command(id),
                    Some(Hit::Link(url)) => {
                        if markdown::safe_web_link(&url) {
                            if MessageBoxW(
                                hwnd,
                                w(&format!("使用默认浏览器打开此链接？\n\n{url}")).as_ptr(),
                                w("打开外部链接").as_ptr(),
                                MB_OKCANCEL | MB_ICONQUESTION,
                            ) == IDOK
                            {
                                ShellExecuteW(
                                    hwnd,
                                    w("open").as_ptr(),
                                    w(&url).as_ptr(),
                                    null(),
                                    null(),
                                    SW_SHOWNORMAL,
                                );
                            }
                        } else {
                            app.status = "仅允许 http / https 链接；本地链接和脚本不会执行".into();
                            app.invalidate();
                        }
                    }
                    _ => {}
                }
            }
            0
        }
        WM_CAPTURECHANGED | WM_CANCELMODE => {
            app.selecting = false;
            app.drag = None;
            app.pressed = None;
            if msg == WM_CANCELMODE {
                ReleaseCapture();
            }
            app.invalidate();
            0
        }
        WM_MOUSELEAVE => {
            app.hover = None;
            app.mouse = None;
            app.invalidate();
            0
        }
        WM_SETCURSOR if lp as u16 as u32 == HTCLIENT => {
            let clickable = app.mouse.is_some_and(|(x, y)| {
                matches!(
                    app.hit(x, y),
                    Some(Hit::Command(_) | Hit::Link(_) | Hit::Heading(_))
                )
            });
            SetCursor(LoadCursorW(
                null_mut(),
                if clickable { IDC_HAND } else { IDC_ARROW },
            ));
            1
        }
        WM_CTLCOLOREDIT => {
            let p = Palette::new(app.settings.theme);
            SetTextColor(wp as HDC, p.fg);
            SetBkColor(wp as HDC, p.bg);
            SetDCBrushColor(wp as HDC, p.bg);
            GetStockObject(DC_BRUSH) as isize
        }
        WM_DROPFILES => {
            let drop = wp as HDROP;
            let count = DragQueryFileW(drop, 0xffffffff, null_mut(), 0);
            if count > 0 {
                let len = DragQueryFileW(drop, 0, null_mut(), 0);
                let mut buf = vec![0u16; len as usize + 1];
                DragQueryFileW(drop, 0, buf.as_mut_ptr(), len + 1);
                app.open(PathBuf::from(OsString::from_wide(&buf[..len as usize])));
            }
            DragFinish(drop);
            0
        }
        WM_DESTROY => {
            app.workers.cancel_search();
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
            0
        }
        _ => {
            // Native move/resize/scroll tracking runs a nested message loop.
            // Let those callbacks borrow App instead of silently bypassing them.
            drop(app);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}
unsafe fn add(menu: HMENU, id: usize, text: &str) {
    AppendMenuW(menu, MF_STRING, id, w(text).as_ptr());
}
unsafe fn menus() -> (HMENU, HMENU) {
    let menu = CreateMenu();
    let file = CreatePopupMenu();
    for (id, text) in [
        (OPEN, "打开…\tCtrl+O"),
        (FIND, "查找…\tCtrl+F"),
        (GOTO, "跳转到百分比…\tCtrl+G"),
        (RELOAD, "重新载入\tF5"),
        (COPY, "复制选中行 / 可见内容\tCtrl+C"),
        (EXIT, "退出"),
    ] {
        add(file, id, text);
    }
    AppendMenuW(menu, MF_POPUP, file as usize, w("文件(&F)").as_ptr());
    let settings = CreatePopupMenu();
    for (id, text) in [
        (GOTO, "跳转到百分比…\tCtrl+G"),
        (OUTLINE, "显示目录\tCtrl+B"),
        (THEME, "切换主题：浅色 / 深色 / 护眼"),
        (ZOOM_IN, "放大字号\tCtrl++"),
        (ZOOM_OUT, "缩小字号\tCtrl+-"),
        (WRAP, "自动换行"),
        (WIDE, "宽屏阅读"),
        (RAW, "源码模式\tCtrl+R"),
        (RESET, "恢复默认阅读设置"),
        (RELOAD, "重新载入\tF5"),
        (COPY, "复制选中行 / 可见内容\tCtrl+C"),
        (ABOUT, "帮助 / 快捷键\tF1"),
        (LICENSES, "导出内置许可证…"),
        (REGISTER, "注册为 Markdown 查看器…"),
        (UNREGISTER, "移除打开方式注册…"),
    ] {
        add(settings, id, text);
    }
    AppendMenuW(menu, MF_POPUP, settings as usize, w("设置(&S)").as_ptr());
    let help = CreatePopupMenu();
    add(help, ABOUT, "关于 / 快捷键\tF1");
    add(help, LICENSES, "导出内置许可证…");
    AppendMenuW(menu, MF_POPUP, help as usize, w("帮助(&H)").as_ptr());
    (menu, settings)
}
pub fn run(path: Option<PathBuf>) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class = w("FeatherMD.Native.Reader");
        let icon = LoadIconW(instance, make_resource(1));
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hIcon: icon,
            hIconSm: icon,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let app = Box::new(RefCell::new(App::new()));
        let (menu, settings_menu) = menus();
        let hwnd = CreateWindowExW(
            WS_EX_ACCEPTFILES,
            class.as_ptr(),
            w("FeatherMD 羽读").as_ptr(),
            (WS_OVERLAPPEDWINDOW & !WS_CAPTION) | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1080,
            820,
            null_mut(),
            null_mut(),
            instance,
            (&*app as *const RefCell<App>).cast(),
        );
        if hwnd.is_null() {
            DestroyMenu(menu);
            return Err(std::io::Error::last_os_error().to_string());
        }
        // CreateWindow may add caption bits to an overlapped top-level window.
        // Remove them explicitly before calculating the final client geometry.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (style & !(WS_CAPTION | WS_VSCROLL | WS_HSCROLL)) as isize,
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        {
            let mut a = app.borrow_mut();
            a.hwnd = hwnd;
            a.menu = menu;
            a.settings_menu = settings_menu;
            a.fonts = Fonts::new(GetDpiForWindow(hwnd));
            a.edit = CreateWindowExW(
                0,
                w("EDIT").as_ptr(),
                w("").as_ptr(),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                0,
                0,
                100,
                30,
                hwnd,
                501usize as HMENU,
                instance,
                null(),
            );
            let font = a.fonts.get(FontKey {
                size: 16,
                bold: false,
                italic: false,
                mono: false,
                strike: false,
            });
            SendMessageW(a.edit, WM_SETFONT, font as usize, 1);
            SendMessageW(a.edit, 0x00c5, 8192, 0);
            a.update_checks();
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            a.width = rect.right;
            a.height = rect.bottom;
            if let Some(path) = path {
                a.open(path);
            }
        }
        let no_border = 0xffff_fffe_u32;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&no_border as *const u32).cast(),
            size_of::<u32>() as u32,
        );
        let round = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&round as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        SetTimer(hwnd, 1, 30, None);
        ShowWindow(hwnd, SW_SHOWDEFAULT);
        UpdateWindow(hwnd);
        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            let edit = app.borrow().edit;
            if msg.message == WM_KEYDOWN && msg.hwnd == edit {
                let mut a = app.borrow_mut();
                if msg.wParam == VK_RETURN as usize {
                    a.submit();
                    continue;
                }
                if msg.wParam == VK_ESCAPE as usize {
                    a.hide_edit();
                    continue;
                }
                if msg.wParam == VK_F3 as usize {
                    a.submit();
                    continue;
                }
                if GetKeyState(VK_CONTROL as i32) < 0 && matches!(msg.wParam, 0x4f | 0x46 | 0x47) {
                    a.keyboard(msg.wParam, true);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        DestroyMenu(menu);
        Ok(())
    }
}
