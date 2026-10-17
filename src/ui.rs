//! Native Win32 shell. All file I/O and searches run off the window thread.
#![allow(unsafe_op_in_unsafe_fn)]
mod about;
mod chrome;
mod pictures;
mod preferences;
mod privacy;
mod reading;
mod select;
mod smooth;
mod snapshot;
mod test_host;
use crate::layout::{FontKey, Fonts, Layout, Picture, Row, Run, View};
use feathermd::{
    distribution,
    document::{Document, Page, SearchResult, human_size},
    i18n::{self, Lang, tr, trf},
    key, markdown,
    scrollbar::ScrollTrack,
    settings::{Editor, Settings, Theme},
    text::{self, Caret},
    viewport::Viewport,
    worker::{Reply, Request, SearchRequest, Workers},
};
use select::Spot;
use std::{
    cell::RefCell,
    ffi::OsString,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{
        Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize},
        DataExchange::*,
        LibraryLoader::*,
        Memory::*,
        Registry::*,
    },
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
const EDIT_DOCUMENT: usize = 107;
const SELECT_ALL: usize = 108;
const THEME: usize = 201;
const ZOOM_IN: usize = 202;
const ZOOM_OUT: usize = 203;
const WRAP: usize = 204;
const RAW: usize = 205;
const WIDE: usize = 206;
const RESET: usize = 207;
const THEME_LIGHT: usize = 208;
const THEME_DARK: usize = 209;
const THEME_SYSTEM: usize = 210;
const THEME_SEPIA: usize = 211;
const THEME_MORE: usize = 212;
const REGISTER: usize = 301;
const UNREGISTER: usize = 302;
const DEFAULT_APP: usize = 303;
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
const THEME_PICKER: usize = 412;
const THEME_PANEL: usize = 413;
const TYPOGRAPHY: usize = 414;
const PREFERENCES: usize = 415;
const EDITOR_SETTINGS: usize = 416;
const CHECK_UPDATES: usize = 417;
const LANGUAGE: usize = 418;
const HEADER_HEIGHT: i32 = 34;
const OUTLINE_ROWS_TOP: i32 = HEADER_HEIGHT + 56;
/// Default and minimum window sizes in logical (96 DPI) pixels.
const DEFAULT_SIZE: (i32, i32) = (1080, 820);
const MIN_SIZE: (i32, i32) = (640, 480);
/// Posted to open Settings once the App borrow is released; WPARAM is the page.
const OPEN_PREFERENCES: u32 = WM_APP + 44;
/// Sent by Settings for live skin/opacity drafts. WPARAM: theme index or
/// usize::MAX to clear the draft; LPARAM: opacity percent.
const PREVIEW_APPEARANCE: u32 = WM_APP + 45;
/// Sent (same thread) by Settings with a finished manual check; LPARAM: `*const Outcome`.
const UPDATE_RESULT: u32 = WM_APP + 46;
/// One-shot timer for the optional startup update check.
const UPDATE_TIMER: usize = 2;
/// Shift held during a mouse message (WPARAM flag).
const MK_SHIFT: usize = 0x0004;
const UPDATE_DELAY_MS: u32 = 3000;
const fn make_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
/// First frame for a new reader: the default size at the window's DPI, kept
/// inside the monitor work area (never below the minimum size), at the
/// system-chosen position moved back inside the work area when it overflows.
fn initial_frame(dpi: u32, origin: POINT, work: RECT) -> RECT {
    let scale = |n: i32| (n as i64 * dpi.max(96) as i64 / 96) as i32;
    let fit = |size: i32, min: i32, span: i32| scale(size).min(span).max(scale(min));
    let width = fit(DEFAULT_SIZE.0, MIN_SIZE.0, work.right - work.left);
    let height = fit(DEFAULT_SIZE.1, MIN_SIZE.1, work.bottom - work.top);
    let left = origin.x.min(work.right - width).max(work.left);
    let top = origin.y.min(work.bottom - height).max(work.top);
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}
/// Loaded source line a layout row belongs to (rows share their line's start).
fn line_of(
    lines: &[feathermd::document::SourceLine],
    start: u64,
) -> Option<&feathermd::document::SourceLine> {
    let i = lines.partition_point(|l| l.start < start);
    lines.get(i).filter(|l| l.start == start)
}
/// Copyable boxes of a preview page, mapped from blocks to the layout rows built from
/// them. Every block has at least one row and rows keep their block's start byte.
fn copy_boxes(blocks: &[markdown::Block], rows: &[Row], page: &Page) -> Vec<CopyBox> {
    let first = |block: usize| rows.partition_point(|r| r.start < blocks[block].start);
    let last = |block: usize| {
        rows.partition_point(|r| r.start <= blocks[block].start)
            .saturating_sub(1)
    };
    markdown::boxes(blocks, page.partial_context, page.end < page.size)
        .into_iter()
        .map(|b| CopyBox {
            kind: b.kind,
            rows: (first(b.first), last(b.last)),
            content: b.content.map(|(a, z)| (first(a), last(z))),
            partial: b.partial,
        })
        .collect()
}
/// Displayed units `[from, to)` of `row` inside a source selection, and whether the
/// selection continues past the row's line break.
fn selected_units(
    lines: &[feathermd::document::SourceLine],
    (a, b): (Caret, Caret),
    row: &Row,
) -> Option<(usize, usize, bool)> {
    let (from, to) = (a.min(b), a.max(b));
    if from == to || row.start < from.line || row.start > to.line {
        return None;
    }
    let line = line_of(lines, row.start)?;
    let start = if line.start == from.line {
        text::units_before(&line.text, from.index)
    } else {
        0
    };
    let end = if line.start == to.line {
        text::units_before(&line.text, to.index)
    } else {
        usize::MAX
    };
    let span: usize = row.runs.iter().map(|r| r.text.len()).sum();
    let (first, last) = (row.unit, row.unit + span);
    let tail = last == text::units_before(&line.text, line.text.len());
    let (a, b) = (start.max(first), end.min(last));
    let eol = tail && end == usize::MAX && !line.continued;
    (a < b || eol).then(|| (a.min(b) - first, b - first, eol))
}
/// Pen position after each displayed unit of `run`, in its font.
unsafe fn pens(dc: HDC, fonts: &mut Fonts, run: &Run) -> Vec<i32> {
    let mut dx = vec![0; run.text.len()];
    if run.text.is_empty() {
        return dx;
    }
    let old = SelectObject(dc, fonts.get(run.font));
    let (mut fit, mut size) = (0, SIZE::default());
    GetTextExtentExPointW(
        dc,
        run.text.as_ptr(),
        run.text.len() as i32,
        i32::MAX / 4,
        &mut fit,
        dx.as_mut_ptr(),
        &mut size,
    );
    SelectObject(dc, old);
    dx
}
/// Copy text of a preview run: an image copies its alternative text.
fn push_run(out: &mut String, run: &Run) {
    match &run.picture {
        Some(picture) => out.push_str(&picture.alt),
        None => out.push_str(&String::from_utf16_lossy(&run.text)),
    }
}
const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
fn system_uses_dark_theme() -> bool {
    let mut light = 1u32;
    let mut bytes = size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            w("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut light as *mut u32).cast(),
            &mut bytes,
        )
    };
    result == 0 && light == 0
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
    /// Reading column. Equal to `bg` for flat skins; book-like skins lay a sheet on a desk.
    page: u32,
    /// Ruled column lines for the Ancient skin.
    ornament: Option<u32>,
}
impl Palette {
    /// Source-mode text selection: the accent blended into the code background, so a
    /// selection stands out on code rows in every skin.
    fn text_highlight(&self) -> u32 {
        let mix = |shift: u32| {
            let (a, b) = ((self.accent >> shift) & 255, (self.code >> shift) & 255);
            (a * 28 + b * 72) / 100
        };
        mix(0) | (mix(8) << 8) | (mix(16) << 16)
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "One positional constructor keeps twenty palettes scannable"
    )]
    const fn flat(
        bg: u32,
        fg: u32,
        muted: u32,
        panel: u32,
        edge: u32,
        accent: u32,
        selection: u32,
        code: u32,
    ) -> Self {
        Self {
            bg,
            fg,
            muted,
            panel,
            edge,
            accent,
            selection,
            code,
            page: bg,
            ornament: None,
        }
    }
    fn new(theme: Theme) -> Self {
        match theme {
            Theme::Light | Theme::System => Self::flat(
                rgb(252, 252, 250),
                rgb(35, 43, 48),
                rgb(108, 117, 124),
                rgb(244, 246, 243),
                rgb(224, 229, 225),
                rgb(20, 113, 99),
                rgb(215, 236, 228),
                rgb(239, 242, 238),
            ),
            Theme::Dark => Self::flat(
                rgb(25, 29, 34),
                rgb(222, 229, 233),
                rgb(147, 160, 168),
                rgb(31, 37, 42),
                rgb(49, 58, 65),
                rgb(112, 211, 181),
                rgb(42, 79, 72),
                rgb(35, 42, 48),
            ),
            Theme::Sepia => Self::flat(
                rgb(247, 239, 221),
                rgb(68, 57, 42),
                rgb(116, 99, 75),
                rgb(241, 230, 208),
                rgb(220, 205, 177),
                rgb(131, 91, 44),
                rgb(228, 212, 172),
                rgb(237, 225, 199),
            ),
            // Warm paper on a linen desk, leather-brown accent.
            Theme::Book => Self {
                page: rgb(250, 247, 240),
                ..Self::flat(
                    rgb(226, 221, 211),
                    rgb(43, 40, 36),
                    rgb(104, 96, 86),
                    rgb(236, 232, 224),
                    rgb(204, 197, 185),
                    rgb(122, 74, 48),
                    rgb(232, 217, 196),
                    rgb(241, 236, 226),
                )
            },
            // Aged rice paper, ink and cinnabar, with faint ruled columns.
            Theme::Ancient => Self {
                page: rgb(236, 224, 197),
                ornament: Some(rgb(205, 150, 124)),
                ..Self::flat(
                    rgb(212, 195, 160),
                    rgb(48, 38, 28),
                    rgb(92, 72, 50),
                    rgb(222, 206, 174),
                    rgb(186, 163, 124),
                    rgb(158, 52, 36),
                    rgb(224, 197, 160),
                    rgb(228, 214, 184),
                )
            },
            Theme::Green => Self::flat(
                rgb(204, 232, 207),
                rgb(31, 52, 38),
                rgb(70, 98, 79),
                rgb(192, 223, 196),
                rgb(167, 203, 172),
                rgb(26, 110, 72),
                rgb(170, 214, 182),
                rgb(190, 222, 195),
            ),
            Theme::Sky => Self::flat(
                rgb(236, 244, 251),
                rgb(28, 44, 61),
                rgb(84, 103, 124),
                rgb(225, 236, 246),
                rgb(200, 216, 232),
                rgb(28, 99, 173),
                rgb(204, 224, 245),
                rgb(222, 233, 244),
            ),
            Theme::Pink => Self::flat(
                rgb(252, 240, 242),
                rgb(61, 36, 44),
                rgb(126, 90, 101),
                rgb(246, 229, 233),
                rgb(232, 203, 211),
                rgb(176, 58, 96),
                rgb(244, 212, 222),
                rgb(244, 226, 231),
            ),
            Theme::Gray => Self::flat(
                rgb(232, 234, 236),
                rgb(37, 40, 44),
                rgb(90, 96, 104),
                rgb(222, 225, 228),
                rgb(200, 204, 209),
                rgb(64, 96, 140),
                rgb(203, 212, 226),
                rgb(220, 223, 227),
            ),
            Theme::Midnight => Self::flat(
                rgb(20, 27, 45),
                rgb(214, 222, 240),
                rgb(140, 153, 180),
                rgb(26, 34, 56),
                rgb(44, 55, 84),
                rgb(130, 170, 255),
                rgb(41, 59, 102),
                rgb(30, 39, 64),
            ),
            // True black for OLED; slightly dimmed text avoids glare.
            Theme::Black => Self::flat(
                rgb(0, 0, 0),
                rgb(220, 220, 220),
                rgb(150, 150, 150),
                rgb(14, 14, 14),
                rgb(42, 42, 42),
                rgb(232, 168, 76),
                rgb(56, 46, 30),
                rgb(20, 20, 20),
            ),
            // Plain white paper with crisp ink and a clear blue link.
            Theme::Paper => Self::flat(
                rgb(255, 255, 255),
                rgb(31, 35, 40),
                rgb(96, 103, 112),
                rgb(246, 247, 249),
                rgb(222, 226, 231),
                rgb(20, 98, 196),
                rgb(221, 234, 252),
                rgb(244, 246, 248),
            ),
            // Warm grey newsprint, near-black ink and navy links.
            Theme::Newsprint => Self::flat(
                rgb(242, 240, 234),
                rgb(28, 27, 25),
                rgb(98, 94, 86),
                rgb(234, 231, 223),
                rgb(212, 207, 196),
                rgb(44, 72, 116),
                rgb(219, 226, 236),
                rgb(232, 229, 221),
            ),
            Theme::Lilac => Self::flat(
                rgb(247, 243, 251),
                rgb(46, 36, 60),
                rgb(107, 94, 124),
                rgb(239, 233, 246),
                rgb(220, 209, 233),
                rgb(118, 62, 160),
                rgb(230, 216, 244),
                rgb(238, 231, 246),
            ),
            // A soft charcoal for evening reading, lower contrast than Dark.
            Theme::Dusk => Self::flat(
                rgb(40, 44, 52),
                rgb(214, 218, 224),
                rgb(152, 160, 172),
                rgb(46, 51, 60),
                rgb(62, 68, 80),
                rgb(234, 150, 128),
                rgb(70, 74, 88),
                rgb(48, 53, 63),
            ),
            // Cool slate and frost blue.
            Theme::Polar => Self::flat(
                rgb(42, 49, 61),
                rgb(228, 232, 238),
                rgb(158, 168, 184),
                rgb(49, 57, 70),
                rgb(68, 78, 95),
                rgb(136, 176, 216),
                rgb(62, 80, 102),
                rgb(52, 60, 74),
            ),
            // Deep teal water with sea-green links.
            Theme::DeepSea => Self::flat(
                rgb(4, 38, 48),
                rgb(214, 222, 216),
                rgb(136, 160, 162),
                rgb(8, 46, 57),
                rgb(26, 68, 80),
                rgb(72, 196, 180),
                rgb(16, 78, 92),
                rgb(10, 50, 62),
            ),
            Theme::Violet => Self::flat(
                rgb(36, 32, 52),
                rgb(234, 230, 244),
                rgb(166, 158, 190),
                rgb(42, 38, 60),
                rgb(62, 56, 86),
                rgb(198, 158, 252),
                rgb(68, 58, 108),
                rgb(46, 41, 66),
            ),
            // Roasted brown with cream text and an amber accent.
            Theme::Mocha => Self::flat(
                rgb(40, 34, 30),
                rgb(232, 220, 200),
                rgb(168, 152, 132),
                rgb(48, 41, 36),
                rgb(70, 60, 52),
                rgb(226, 160, 90),
                rgb(84, 62, 44),
                rgb(50, 43, 37),
            ),
        }
    }
}
fn skin_name(theme: Theme) -> &'static str {
    match theme {
        Theme::System => tr("跟随系统"),
        Theme::Light => tr("浅色"),
        Theme::Dark => tr("深色"),
        Theme::Sepia => tr("护眼"),
        Theme::Book => tr("书本"),
        Theme::Ancient => tr("古籍"),
        Theme::Green => tr("豆沙绿"),
        Theme::Sky => tr("天青"),
        Theme::Pink => tr("樱粉"),
        Theme::Gray => tr("雾灰"),
        Theme::Midnight => tr("夜蓝"),
        Theme::Black => tr("纯黑"),
        Theme::Paper => tr("素白"),
        Theme::Newsprint => tr("报纸"),
        Theme::Lilac => tr("丁香"),
        Theme::Dusk => tr("暮色"),
        Theme::Polar => tr("极夜"),
        Theme::DeepSea => tr("深海"),
        Theme::Violet => tr("紫夜"),
        Theme::Mocha => tr("咖啡"),
    }
}
#[derive(Clone, PartialEq, Eq)]
enum Hit {
    Command(usize),
    Link(String),
    LoadImage(String),
    Row(usize),
    Heading(usize),
    /// Copy button of the box with this index in `App::boxes`.
    CopyBlock(usize),
}
/// A code block, quote or table of the preview page, in layout rows.
struct CopyBox {
    kind: markdown::BoxKind,
    /// Inclusive layout rows of the whole box.
    rows: (usize, usize),
    /// Inclusive layout rows to copy; None when the box is empty.
    content: Option<(usize, usize)>,
    partial: bool,
}
/// How long a copy button shows its check mark.
const COPIED_FOR: Duration = Duration::from_millis(1500);
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
    reading_fonts: Fonts,
    settings: Settings,
    system_dark: bool,
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
    /// Preview mode: character selection as (anchor, active) positions.
    selection: Option<(Spot, Spot)>,
    /// Source mode: character selection as (anchor, active) carets.
    text_selection: Option<(Caret, Caret)>,
    selecting: bool,
    /// Pointer while dragging a source selection; outside the text it scrolls.
    drag_point: Option<(i32, i32)>,
    mouse: Option<(i32, i32)>,
    hover: Option<usize>,
    pressed: Option<Hit>,
    theme_menu: Option<usize>,
    hover_since: Option<Instant>,
    tooltip_visible: bool,
    drag: Option<ScrollDrag>,
    wheel_remainder: i32,
    horizontal_wheel_remainder: i32,
    document_name: String,
    outline: Option<feathermd::outline::Outline>,
    outline_ticket: u64,
    outline_loading: bool,
    outline_error: Option<String>,
    outline_top: usize,
    outline_selected: Option<(usize, usize)>,
    outline_pending: Option<usize>,
    horizontal: i32,
    status: String,
    edit_mode: Option<usize>,
    query: String,
    found: Option<u64>,
    search_ticket: u64,
    /// Unsaved skin/opacity draft from the open Settings dialog.
    preview: Option<(Theme, u8)>,
    /// F5 restores this source byte and wrapped row once the reopened document is ready.
    reload_anchor: Option<(u64, usize)>,
    /// Automatic update check in flight (manual checks belong to Settings).
    update_check: Option<crate::updater::Pending>,
    /// Automatic checks started in this session (diagnostics).
    update_checks: usize,
    /// Copyable boxes of the preview page, in row order.
    boxes: Vec<CopyBox>,
    /// The box whose copy button was just used, by its first row's source byte.
    copied: Option<(u64, Instant)>,
    /// Local images when enabled, plus individually confirmed remote images.
    pictures: pictures::Pictures,
    /// An image finished while the pointer was dragging; relayout when it ends.
    pictures_changed: bool,
    /// Client rectangles of the images the last paint drew (diagnostics).
    painted_pictures: Vec<RECT>,
}
impl App {
    fn new(settings_path: PathBuf) -> Self {
        let mut settings = Settings::load(&settings_path);
        if distribution::store() {
            settings.update_check = false;
            settings.update_latest.clear();
        }
        // Before any font or string below is created.
        i18n::set(Lang::resolve(settings.language));
        Self {
            hwnd: null_mut(),
            edit: null_mut(),
            menu: null_mut(),
            settings_menu: null_mut(),
            fonts: Fonts::new(96),
            reading_fonts: Fonts::with_family(96, settings.reading_font()),
            settings,
            system_dark: system_uses_dark_theme(),
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
            text_selection: None,
            selecting: false,
            drag_point: None,
            mouse: None,
            hover: None,
            pressed: None,
            theme_menu: None,
            hover_since: None,
            tooltip_visible: false,
            drag: None,
            wheel_remainder: 0,
            horizontal_wheel_remainder: 0,
            document_name: String::new(),
            outline: None,
            outline_ticket: 0,
            outline_loading: false,
            outline_error: None,
            outline_top: 0,
            outline_selected: None,
            outline_pending: None,
            horizontal: 0,
            status: tr("只读 · 离线阅读").into(),
            edit_mode: None,
            query: String::new(),
            found: None,
            search_ticket: 0,
            preview: None,
            reload_anchor: None,
            update_check: None,
            update_checks: 0,
            boxes: Vec::new(),
            copied: None,
            pictures: pictures::Pictures::default(),
            pictures_changed: false,
            painted_pictures: Vec::new(),
        }
    }
    /// Folder that relative image paths of the open document start from.
    fn document_folder(&self) -> PathBuf {
        self.document
            .as_ref()
            .and_then(|d| pictures::folder(&d.path))
            .unwrap_or_default()
    }
    /// A newer published release recorded by the last successful check.
    fn update_available(&self) -> Option<&str> {
        if distribution::store() {
            return None;
        }
        let latest = feathermd::update::Version::parse(&self.settings.update_latest)?;
        (latest > feathermd::update::current()).then_some(self.settings.update_latest.as_str())
    }
    unsafe fn start_update_check(&mut self) {
        if !distribution::store() && self.update_check.is_none() {
            self.update_check = Some(crate::updater::start());
            self.update_checks += 1;
        }
    }
    /// Record a finished check. Automatic failures stay silent and retry next launch.
    unsafe fn record_update(&mut self, outcome: &crate::updater::Outcome, manual: bool) {
        let Ok(release) = outcome else {
            return;
        };
        self.settings.update_checked = feathermd::update::now();
        self.settings.update_latest = release.tag.clone();
        self.save_preferences();
        if !manual && self.update_available().is_some() {
            self.status = trf("发现新版本 {} · 更多 → 检查更新", &[&release.tag]);
        }
        self.invalidate();
    }
    fn s(&self, n: i32) -> i32 {
        self.fonts.scale(n)
    }
    /// The resolved skin currently on screen, including an unsaved Settings draft.
    fn active_theme(&self) -> Theme {
        self.preview
            .map_or(self.settings.theme, |(theme, _)| theme)
            .resolve(self.system_dark)
    }
    fn palette(&self) -> Palette {
        Palette::new(self.active_theme())
    }
    fn opacity(&self) -> u8 {
        self.preview
            .map_or(self.settings.opacity, |(_, opacity)| opacity)
    }
    /// Whole-window opacity. Fully opaque windows drop WS_EX_LAYERED entirely.
    unsafe fn apply_opacity(&self) {
        let percent = Settings::clamp_opacity(i64::from(self.opacity()));
        let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
        if percent >= 100 {
            if style & WS_EX_LAYERED != 0 {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, (style & !WS_EX_LAYERED) as isize);
                RedrawWindow(
                    self.hwnd,
                    null(),
                    null_mut(),
                    RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN,
                );
            }
            return;
        }
        if style & WS_EX_LAYERED == 0 {
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, (style | WS_EX_LAYERED) as isize);
        }
        let alpha = ((u32::from(percent) * 255 + 50) / 100) as u8;
        SetLayeredWindowAttributes(self.hwnd, 0, alpha, LWA_ALPHA);
    }
    fn reading_area(&self, x: i32, y: i32) -> bool {
        self.document.is_some()
            && x >= self.sidebar_width()
            && x < self.width - self.s(22)
            && y >= self.content_top()
            && y < self.content_bottom()
    }
    fn content_top(&self) -> i32 {
        let gap = if self.edit_mode.is_some() { 56 } else { 8 };
        self.s(HEADER_HEIGHT + gap)
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
        if self.settings.outline && self.document.is_some() {
            self.s(if self.width < self.s(800) { 192 } else { 232 })
        } else {
            0
        }
    }
    fn outline_visible_rows(&self) -> usize {
        ((self.height - self.s(62) - self.s(OUTLINE_ROWS_TOP)) / self.s(30).max(1)).max(1) as usize
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
    fn outline_active(&self) -> Option<usize> {
        let outline = self.outline.as_ref()?;
        // Near the end the clicked heading cannot become the top row, so keep the
        // user's choice until the reader moves away from the position it produced.
        if let Some((index, top)) = self.outline_selected
            && top == self.top
            && index < outline.headings.len()
        {
            return Some(index);
        }
        let current = self.current();
        outline
            .headings
            .iter()
            .rposition(|heading| heading.offset <= current)
    }
    unsafe fn select_heading(&mut self, index: usize) {
        let Some(offset) = self
            .outline
            .as_ref()
            .and_then(|o| o.headings.get(index))
            .map(|h| h.offset)
        else {
            return;
        };
        self.outline_selected = None;
        self.jump(offset);
        if self.loading {
            self.outline_pending = Some(index);
        } else {
            self.outline_selected = Some((index, self.top));
        }
    }
    unsafe fn invalidate(&self) {
        InvalidateRect(self.hwnd, null(), 0);
    }
    unsafe fn open(&mut self, path: PathBuf) {
        self.reload_anchor = None;
        self.outline_ticket = self.workers.cancel_outline();
        self.outline = None;
        self.outline_loading = false;
        self.outline_error = None;
        self.outline_top = 0;
        self.outline_selected = None;
        self.outline_pending = None;
        self.pending_scroll = None;
        self.workers.cancel_search();
        self.io_ticket += 1;
        self.loading = true;
        self.document = None;
        self.page = None;
        self.layout = None;
        self.boxes.clear();
        self.copied = None;
        // F5 and another document read their images afresh.
        self.pictures.clear();
        self.top = 0;
        self.anchor = 0;
        self.horizontal = 0;
        self.selection = None;
        self.text_selection = None;
        self.drag = None;
        self.wheel_remainder = 0;
        self.document_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.found = None;
        self.status = tr("正在读取首屏…").into();
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
        self.outline_pending = None;
        if let Some(doc) = &self.document {
            self.io_ticket += 1;
            self.loading = true;
            self.anchor = anchor;
            self.status = tr("正在读取分页…").into();
            let _ = self
                .workers
                .io
                .send(Request::Page(doc.clone(), anchor, self.io_ticket));
            self.invalidate();
        }
    }
    unsafe fn relayout(&mut self, anchor: u64) {
        self.outline_selected = None;
        self.pictures_changed = false;
        if let Some(page) = &self.page {
            let blocks = markdown::blocks(&page.lines, self.settings.raw);
            let dc = GetDC(self.hwnd);
            let width = self.body_width();
            // Queue this page's local images first, so layout sees every one.
            let folder = self.document_folder();
            let images = self.settings.images && !self.settings.raw;
            self.pictures.begin();
            if !self.settings.raw {
                for image in blocks
                    .iter()
                    .flat_map(|b| &b.spans)
                    .filter_map(|s| s.image.as_ref())
                {
                    self.pictures.touch_web(&image.src);
                }
            }
            if images {
                for span in blocks.iter().flat_map(|b| &b.spans) {
                    if let Some(image) = &span.image
                        && let feathermd::image::Source::Local(path) =
                            feathermd::image::resolve(&image.src, &folder)
                    {
                        self.pictures.want(&path);
                    }
                }
            }
            let pictures = &self.pictures;
            let lookup = |image: &markdown::Image| {
                if !self.settings.raw {
                    pictures.lookup(&image.src, &folder, images)
                } else {
                    Picture::Off
                }
            };
            let view = View {
                settings: &self.settings,
                pictures: &lookup,
                // An image row stays shorter than the reading area, so row-wise
                // scrolling can always step past it.
                max_height: (self.content_bottom() - self.content_top() - self.s(36))
                    .max(self.s(48)),
            };
            let layout = Layout::build(dc, &blocks, &view, &mut self.reading_fonts, width);
            ReleaseDC(self.hwnd, dc);
            // Source mode shows every line as code; only the preview has boxes.
            self.boxes = if self.settings.raw {
                Vec::new()
            } else {
                copy_boxes(&blocks, &layout.rows, page)
            };
            let mut layout = layout;
            for b in &self.boxes {
                if matches!(
                    b.kind,
                    markdown::BoxKind::Fenced | markdown::BoxKind::Indented
                ) {
                    for row in layout.rows.get_mut(b.rows.0..=b.rows.1).unwrap_or(&mut []) {
                        row.boxed = true;
                    }
                }
            }
            self.layout = Some(layout);
            self.position(anchor, 0);
            self.scrollbar();
        }
        self.invalidate();
    }
    unsafe fn receive(&mut self) {
        if let Some(outcome) = self.update_check.as_ref().and_then(|p| p.poll()) {
            self.update_check = None;
            self.record_update(&outcome, false);
        }
        // Decoded images of this page: lay it out again around the same top row,
        // but not under a drag, which would lose its selection.
        self.pictures_changed |= self.pictures.receive();
        if self.pictures_changed && !self.selecting && self.drag.is_none() && !self.loading {
            let anchor = self.current();
            let wrapped = self.wrapped_row(self.top);
            self.relayout(anchor);
            self.position(anchor, wrapped);
        }
        while let Ok(reply) = self.workers.replies.try_recv() {
            match reply {
                Reply::Opened(id, result) if id == self.io_ticket => {
                    self.loading = false;
                    match result {
                        Ok((doc, page)) => {
                            let size = doc.size;
                            self.document = Some(doc);
                            self.page = Some(page);
                            self.anchor = 0;
                            self.status = tr("就绪").into();
                            self.relayout(0);
                            self.start_outline();
                            // F5 after an external edit keeps the reader near the same place.
                            if let Some((anchor, wrapped_row)) = self.reload_anchor.take()
                                && (anchor > 0 || wrapped_row > 0)
                            {
                                let anchor = anchor.min(size.saturating_sub(1));
                                self.jump(anchor);
                                if self.loading {
                                    self.pending_scroll = Some((wrapped_row, 0));
                                } else {
                                    self.position(anchor, wrapped_row);
                                    self.status = tr("已重新载入").into();
                                }
                            }
                        }
                        Err(e) => {
                            self.reload_anchor = None;
                            self.status = trf("打开失败：{}", &[&e]);
                            self.error(&trf("无法打开文件。\n\n{}", &[&e]));
                        }
                    }
                }
                Reply::Page(id, anchor, result) if id == self.io_ticket => {
                    self.loading = false;
                    let pending = self.pending_scroll.take();
                    let heading = self.outline_pending.take();
                    match result {
                        Ok(page) => {
                            self.page = Some(page);
                            self.status = tr("就绪").into();
                            self.relayout(anchor);
                            if let Some((wrapped_row, delta)) = pending {
                                self.position(anchor, wrapped_row);
                                self.scroll(delta);
                            }
                            if let Some(index) = heading {
                                self.outline_selected = Some((index, self.top));
                            }
                        }
                        Err(e) => {
                            self.status = trf("读取失败：{}", &[&e]);
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
                            self.status = trf("找到匹配 · 字节位置 {}", &[&offset]);
                            self.jump(offset);
                        }
                        Ok(SearchResult::NotFound) => {
                            self.status = tr("未找到匹配（区分大小写）").into();
                        }
                        Ok(SearchResult::Cancelled) => {}
                        Err(e) => self.status = trf("搜索失败：{}", &[&e]),
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
        self.outline_selected = None;
        self.outline_pending = None;
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
        // Wrapped text always fits; only a table whose narrowest columns do not fit scrolls.
        self.layout.as_ref().map_or(0, |l| {
            let right = if self.settings.wrap {
                l.table_width
            } else {
                l.max_width
            };
            (right - self.body_width()).max(0)
        })
    }
    /// Diagnostics per table of the page: (right edge, visual lines, rows, columns,
    /// first layout row). Lines of one table share their rule positions.
    fn tables(&self) -> Vec<(i32, usize, usize, usize, usize)> {
        let mut out: Vec<(i32, usize, usize, usize, usize)> = Vec::new();
        let mut current: Option<(&std::rc::Rc<[i32]>, u64)> = None;
        for (index, row) in self.layout.iter().flat_map(|l| l.rows.iter().enumerate()) {
            let Some(line) = &row.table else {
                current = None;
                continue;
            };
            let same = current.is_some_and(|(edges, _)| std::rc::Rc::ptr_eq(edges, &line.edges));
            if !same {
                let right = line.edges.last().map_or(0, |e| e + 1);
                out.push((right, 0, 0, line.edges.len() - 1, index));
                current = None;
            }
            let entry = out.last_mut().expect("pushed above");
            entry.1 += 1;
            if current.is_none_or(|(_, start)| start != row.start) {
                entry.2 += 1;
            }
            current = Some((&line.edges, row.start));
        }
        out
    }
    /// Horizontal scroll applied to `row`. With wrapping on, only tables wider than
    /// the reading column scroll; text and tables that fit stay in place.
    fn shift(&self, row: &Row) -> i32 {
        let overflows = row
            .table
            .as_ref()
            .and_then(|t| t.edges.last())
            .is_some_and(|&edge| edge + 1 > self.body_width());
        if self.settings.wrap && !overflows {
            0
        } else {
            self.horizontal
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
    unsafe fn save_preferences(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            self.status = trf("设置未能保存：{}", &[&e]);
        }
        self.update_checks();
    }
    unsafe fn persist(&mut self) {
        self.save_preferences();
        let a = self.current();
        self.relayout(a);
    }
    unsafe fn persist_theme(&mut self) {
        // Colors do not change layout. Keep wrapped-row, selection and horizontal position.
        self.save_preferences();
        self.invalidate();
        InvalidateRect(self.edit, null(), 1);
    }
    /// Apply a saved Settings draft. Only fields owned by the dialog are copied,
    /// so commands handled while it was open are not overwritten.
    unsafe fn apply_preferences(&mut self, draft: Settings) {
        let reflow = draft.font_family != self.settings.font_family
            || draft.font_size != self.settings.font_size
            || draft.images != self.settings.images;
        self.settings.font_family = draft.font_family;
        self.settings.font_size = draft.font_size;
        if self.settings.images && !draft.images {
            // Turned off: free the decoded images at once.
            self.pictures.clear();
        }
        self.settings.images = draft.images;
        self.settings.theme = draft.theme;
        self.settings.opacity = draft.opacity;
        self.settings.editor = draft.editor;
        self.settings.update_check = draft.update_check;
        let language = draft.language != self.settings.language;
        self.settings.language = draft.language;
        self.preview = None;
        self.system_dark = system_uses_dark_theme();
        self.apply_opacity();
        if language {
            self.apply_language();
        }
        // The automatic reading face follows the interface language.
        let face = self.settings.reading_font().to_owned();
        let refont = self.reading_fonts.family() != face;
        if refont {
            self.reading_fonts = Fonts::with_family(self.fonts.dpi, &face);
        }
        if reflow || language || refont {
            self.persist();
        } else {
            // Skin, opacity and editor never change layout or the wrapped-row position.
            self.save_preferences();
            self.invalidate();
        }
        InvalidateRect(self.edit, null(), 1);
    }
    /// Switch interface strings, the UI font and the menus to the saved language.
    unsafe fn apply_language(&mut self) {
        i18n::set(Lang::resolve(self.settings.language));
        // The search box keeps using the old font until it is given the new one.
        let dpi = self.fonts.dpi;
        let old = std::mem::replace(&mut self.fonts, Fonts::new(dpi));
        let font = self.fonts.get(FontKey {
            size: 16,
            bold: false,
            italic: false,
            mono: false,
            strike: false,
            underline: false,
        });
        SendMessageW(self.edit, WM_SETFONT, font as usize, 1);
        drop(old);
        DestroyMenu(self.menu);
        (self.menu, self.settings_menu) = menus();
        if self.document_name.is_empty() {
            SetWindowTextW(self.hwnd, w(tr("FeatherMD 羽读")).as_ptr());
        }
        self.status = tr("就绪").into();
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
        let filter = w(tr(
            "Markdown / Text\0*.md;*.markdown;*.mdown;*.mkd;*.txt\0所有文件\0*.*\0",
        ));
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
                Ok(()) => self.status = tr("已导出内置许可证").into(),
                Err(e) => self.error(&e.to_string()),
            }
        }
    }
    unsafe fn position_edit(&self, _mode: usize) {
        let left = (self.sidebar_width() + self.s(24)).max(self.s(48));
        MoveWindow(
            self.edit,
            left,
            self.s(HEADER_HEIGHT + 12),
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
        self.status = tr("就绪").into();
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
                    self.status = tr("请输入 0～100 之间的百分比").into();
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
            self.status = tr("正在后台搜索…（Esc 取消，区分大小写）").into();
            self.invalidate();
        }
    }
    /// Source caret nearest to client `x` on layout row `index` (source mode), and the
    /// byte under the pointer, for word selection.
    unsafe fn caret_at(&mut self, index: usize, x: i32) -> Option<(Caret, usize)> {
        let origin = {
            let row = self.layout.as_ref()?.rows.get(index)?;
            self.left() - self.shift(row)
        };
        let (Some(layout), Some(page)) = (&self.layout, &self.page) else {
            return None;
        };
        let row = layout.rows.get(index)?;
        let line = line_of(&page.lines, row.start)?;
        let run = row.runs.first()?;
        let mut dx = vec![0; run.text.len()];
        if !run.text.is_empty() {
            let dc = GetDC(self.hwnd);
            let old = SelectObject(dc, self.reading_fonts.get(run.font));
            let (mut fit, mut size) = (0, SIZE::default());
            GetTextExtentExPointW(
                dc,
                run.text.as_ptr(),
                run.text.len() as i32,
                i32::MAX / 4,
                &mut fit,
                dx.as_mut_ptr(),
                &mut size,
            );
            SelectObject(dc, old);
            ReleaseDC(self.hwnd, dc);
        }
        let local = x - origin - run.x;
        let pen = |k: usize| if k == 0 { 0 } else { dx[k - 1] };
        let (first, last) = (row.unit, row.unit + run.text.len());
        let (mut best, mut under, mut unit) = (None::<(i32, usize)>, None, 0);
        let ends = std::iter::once((line.text.len(), 0));
        for (byte, width) in line
            .text
            .char_indices()
            .map(|(i, c)| (i, text::units_of(c)))
            .chain(ends)
        {
            if unit > last {
                break;
            }
            if unit >= first {
                let at = pen(unit - first);
                let distance = (at - local).abs();
                if best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, byte));
                }
                if width > 0
                    && unit + width <= last
                    && local >= at
                    && local < pen(unit + width - first)
                {
                    under = Some(byte);
                }
            }
            unit += width;
        }
        let (_, byte) = best?;
        Some((
            Caret {
                line: line.start,
                index: byte,
            },
            under.unwrap_or(byte),
        ))
    }
    /// Caret at the start or end of a source-mode layout row.
    fn row_caret(&self, row: &Row, end: bool) -> Option<Caret> {
        let line = line_of(&self.page.as_ref()?.lines, row.start)?;
        let units = row.unit
            + if end {
                row.runs.iter().map(|r| r.text.len()).sum()
            } else {
                0
            };
        Some(Caret {
            line: line.start,
            index: text::index_at_unit(&line.text, units),
        })
    }
    /// Layout row painted at client `y`, clamped to the rows on screen.
    fn row_near(&self, y: i32) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let end = self.viewport().visible_end(self.top);
        let mut top = self.content_top() + self.s(12);
        for index in self.top..end {
            let height = layout.rows[index].height;
            if y < top + height {
                return Some(index);
            }
            top += height;
        }
        end.checked_sub(1).filter(|&last| last >= self.top)
    }
    /// The preview selection in reading order; None when nothing is selected.
    fn preview_range(&self) -> Option<(Spot, Spot)> {
        let (a, b) = self.selection?;
        (a != b).then(|| (a.min(b), a.max(b)))
    }
    /// Preview position at client `x` on layout row `index`. A table row is one
    /// piece: its left half is before it, its right half after it.
    unsafe fn spot_at(&mut self, index: usize, x: i32) -> Option<Spot> {
        let origin = {
            let row = self.layout.as_ref()?.rows.get(index)?;
            self.left() - self.shift(row)
        };
        let row = self.layout.as_ref()?.rows.get(index)?;
        if let Some(table) = &row.table {
            let middle = table.edges.last().copied().unwrap_or(0) / 2;
            let unit = usize::from(x - origin >= middle);
            return Some(Spot {
                block: row.start,
                unit,
            });
        }
        let dc = GetDC(self.hwnd);
        let fonts = &mut self.reading_fonts;
        let unit = select::nearest(row, x - origin, |run| pens(dc, fonts, run));
        ReleaseDC(self.hwnd, dc);
        Some(Spot {
            block: row.start,
            unit,
        })
    }
    /// Moves the active end of a source or preview selection to the pointer.
    unsafe fn drag_to(&mut self, x: i32, y: i32) {
        let Some(index) = self.row_near(y) else {
            return;
        };
        if !self.settings.raw {
            if let (Some(spot), Some((anchor, _))) = (self.spot_at(index, x), self.selection) {
                self.selection = Some((anchor, spot));
                self.invalidate();
            }
            return;
        }
        if let (Some((caret, _)), Some((anchor, _))) =
            (self.caret_at(index, x), self.text_selection)
        {
            self.text_selection = Some((anchor, caret));
            self.invalidate();
        }
    }
    /// Scrolls while a source selection is dragged above or below the text.
    unsafe fn drag_scroll(&mut self) {
        let Some((x, y)) = self.drag_point.filter(|_| self.selecting) else {
            return;
        };
        let (top, bottom) = (self.content_top(), self.content_bottom());
        let distance = if y < top {
            y - top
        } else if y >= bottom {
            y - bottom + 1
        } else {
            return;
        };
        // Farther from the text scrolls faster, up to ten rows per tick.
        let rows = (1 + distance.abs() / self.s(20)).min(10);
        self.scroll(rows * distance.signum());
        self.drag_to(x, y);
    }
    /// What Copy puts on the clipboard, and the status line describing it.
    fn copy_text(&self) -> Option<(String, &'static str)> {
        let layout = self.layout.as_ref()?;
        let last_visible = (self.top + self.visible_rows().saturating_sub(1))
            .min(layout.rows.len().saturating_sub(1));
        if self.settings.raw {
            let lines = &self.page.as_ref()?.lines;
            if let Some((a, b)) = self.text_selection.filter(|(a, b)| a != b) {
                let (text, clipped) = text::selected(lines, a, b);
                let status = if clipped {
                    tr("已复制选中文本中已载入的部分")
                } else {
                    tr("已复制选中文本")
                };
                return Some((text, status));
            }
            // Nothing selected: the exact source of the rows on screen.
            let from = self.row_caret(layout.rows.get(self.top)?, false)?;
            let to = self.row_caret(layout.rows.get(last_visible)?, true)?;
            return Some((text::selected(lines, from, to).0, tr("已复制可见内容")));
        }
        if let Some((from, to)) = self.preview_range() {
            // A selection kept across a page change may reach past the loaded page.
            let lines = &self.page.as_ref()?.lines;
            let clipped = lines.first().is_some_and(|l| from.block < l.start)
                || lines.last().is_some_and(|l| to.block > l.start);
            let status = if clipped {
                tr("已复制选中文本中已载入的部分")
            } else {
                tr("已复制选中文本")
            };
            return Some((select::text(&layout.rows, from, to), status));
        }
        // Nothing selected: what is on screen, as shown.
        let (first, last) = (layout.rows.get(self.top)?, layout.rows.get(last_visible)?);
        let from = Spot {
            block: first.start,
            unit: first.unit,
        };
        let to = Spot {
            block: last.start,
            unit: select::row_end(last).max(1),
        };
        Some((select::text(&layout.rows, from, to), tr("已复制可见内容")))
    }
    unsafe fn copy(&mut self) {
        let Some((text, status)) = self.copy_text() else {
            return;
        };
        if self.set_clipboard(&text) {
            self.status = status.into();
        }
        self.invalidate();
    }
    /// What the copy button of box `index` copies: code as its exact source (no
    /// fences; indented code loses one indent), quotes as shown, tables as
    /// tab-separated rows.
    fn box_text(&self, index: usize) -> Option<String> {
        let b = self.boxes.get(index)?;
        let (a, z) = b.content?;
        let rows = &self.layout.as_ref()?.rows;
        let (first, last) = (rows.get(a)?, rows.get(z)?);
        if matches!(
            b.kind,
            markdown::BoxKind::Fenced | markdown::BoxKind::Indented
        ) {
            return Some(text::source(
                &self.page.as_ref()?.lines,
                first.start,
                last.start,
                b.kind == markdown::BoxKind::Indented,
            ));
        }
        // Wrapped rows of one line join without a break; rows of a table row repeat
        // its copy text and are written once.
        let mut out = String::new();
        let mut line = None;
        for row in &rows[a..=z] {
            let next = line != Some(row.start);
            if next && line.is_some() {
                out.push_str("\r\n");
            }
            line = Some(row.start);
            match &row.table {
                Some(table) if next => out.push_str(&table.copy),
                Some(_) => {}
                None => {
                    for run in &row.runs {
                        push_run(&mut out, run);
                    }
                }
            }
        }
        Some(out)
    }
    unsafe fn copy_box(&mut self, index: usize) {
        let (Some(text), Some(b)) = (self.box_text(index), self.boxes.get(index)) else {
            return;
        };
        let (kind, partial, row) = (b.kind, b.partial, b.rows.0);
        if !self.set_clipboard(&text) {
            self.invalidate();
            return;
        }
        self.status = match (kind, partial) {
            (markdown::BoxKind::Table, false) => tr("已复制表格"),
            (markdown::BoxKind::Table, true) => tr("已复制表格中已载入的部分"),
            (markdown::BoxKind::Quote, false) => tr("已复制引用"),
            (markdown::BoxKind::Quote, true) => tr("已复制引用中已载入的部分"),
            (_, false) => tr("已复制代码块"),
            (_, true) => tr("已复制代码块中已载入的部分"),
        }
        .into();
        let start = self
            .layout
            .as_ref()
            .and_then(|l| l.rows.get(row))
            .map(|r| r.start);
        self.copied = start.map(|s| (s, Instant::now()));
        self.invalidate();
    }
    /// Put `text` on the clipboard. With FEATHERMD_TEST_SNAPSHOT set it is written to
    /// copy.txt beside the snapshot instead, so tests never touch the user's clipboard.
    unsafe fn set_clipboard(&mut self, text: &str) -> bool {
        if let Some(path) = std::env::var_os("FEATHERMD_TEST_SNAPSHOT") {
            return std::fs::write(PathBuf::from(path).with_file_name("copy.txt"), text).is_ok();
        }
        let data = w(text);
        if OpenClipboard(self.hwnd) == 0 {
            self.status = tr("剪贴板暂不可用，请重试").into();
            return false;
        }
        let mut done = false;
        let memory = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
        if !memory.is_null() {
            let dest = GlobalLock(memory);
            if !dest.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), dest.cast(), data.len());
                GlobalUnlock(memory);
                done = EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null();
            }
            if !done {
                GlobalFree(memory);
            }
        }
        CloseClipboard();
        done
    }
    /// The copy button of box `index` shows its check mark.
    fn just_copied(&self, index: usize) -> bool {
        let start = self
            .boxes
            .get(index)
            .and_then(|b| self.layout.as_ref()?.rows.get(b.rows.0))
            .map(|r| r.start);
        self.copied
            .is_some_and(|(s, at)| Some(s) == start && at.elapsed() < COPIED_FOR)
    }
    fn visible_rows(&self) -> usize {
        self.viewport()
            .visible_end(self.top)
            .saturating_sub(self.top)
    }
    unsafe fn register_open_with(&mut self) -> bool {
        if distribution::store() {
            self.status = tr("文件关联由 Windows 随商店包管理，无需手动注册").into();
            return true;
        }
        match std::env::current_exe().and_then(|path| crate::association::register(&path)) {
            Ok(()) => {
                self.status = tr("已加入 Markdown 文件的右键打开方式").into();
                true
            }
            Err(error) => {
                self.error(&trf("无法加入打开方式：{}", &[&error]));
                false
            }
        }
    }
    unsafe fn choose_default_app(&mut self) {
        if !self.register_open_with() {
            return;
        }
        // Opening Settings is not evidence that the default actually changed.
        // Windows owns the final per-user association choice.
        for uri in [
            &distribution::default_apps_uri(distribution::identity()),
            "ms-settings:defaultapps",
        ] {
            let result = ShellExecuteW(
                self.hwnd,
                w("open").as_ptr(),
                w(uri).as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            ) as isize;
            if result > 32 {
                self.status = tr("请在 Windows 默认应用中将 .md 设为 FeatherMD").into();
                return;
            }
        }
        self.error(tr("已加入打开方式，但无法打开 Windows 默认应用设置。\n请右键 .md 文件 → 打开方式 → 选择其他应用 → FeatherMD → 始终使用。"));
    }
    fn edit_label(&self) -> String {
        match &self.settings.editor {
            Editor::System => tr("编辑（系统默认）\tCtrl+E").into(),
            Editor::Notepad => tr("用记事本编辑\tCtrl+E").into(),
            editor => trf("用 {} 编辑\tCtrl+E", &[&crate::editor::name(editor)]),
        }
    }
    /// Hand the open document to the chosen editor. The reader itself stays read-only.
    unsafe fn edit_document(&mut self) {
        let Some(path) = self.document.as_ref().map(|d| d.path.clone()) else {
            self.status = tr("请先打开文档").into();
            return;
        };
        let editor = self.settings.editor.clone();
        match crate::editor::launch(&editor, self.hwnd, &path) {
            Ok(launched) => {
                let name = match (&editor, launched) {
                    (_, crate::editor::Launched::NotepadFallback) => {
                        tr("记事本（系统未登记编辑方式）").to_string()
                    }
                    (Editor::System, _) => tr("系统默认编辑器").to_string(),
                    (editor, _) => crate::editor::name(editor),
                };
                self.status = trf("已用 {} 打开 · 保存后按 F5 重新载入", &[&name]);
            }
            Err(crate::editor::LaunchError::MissingProgram(program)) => {
                self.status = tr("找不到默认编辑应用").into();
                if MessageBoxW(
                    self.hwnd,
                    w(&trf(
                        "找不到默认编辑应用：\n{}\n\n现在重新选择编辑应用吗？",
                        &[&program],
                    ))
                    .as_ptr(),
                    w(tr("编辑")).as_ptr(),
                    MB_YESNO | MB_ICONWARNING,
                ) == IDYES
                {
                    PostMessageW(self.hwnd, OPEN_PREFERENCES, 2, 0);
                }
            }
            Err(crate::editor::LaunchError::Failed(error)) => {
                self.status = tr("无法启动编辑应用").into();
                self.error(&trf("无法打开编辑应用。\n\n{}", &[&error]));
            }
        }
        self.invalidate();
    }
    /// The context menu is built fresh so its labels describe the current state.
    unsafe fn context_menu(&self) -> HMENU {
        let menu = CreatePopupMenu();
        let document = self.document.is_some();
        let state = |enabled: bool| MF_STRING | if enabled { 0 } else { MF_GRAYED };
        AppendMenuW(
            menu,
            state(document),
            EDIT_DOCUMENT,
            w(&self.edit_label()).as_ptr(),
        );
        add(menu, EDITOR_SETTINGS, tr("选择编辑应用…"));
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(
            menu,
            state(self.layout.is_some()),
            COPY,
            w(
                if (self.settings.raw && self.text_selection.is_some_and(|(a, b)| a != b))
                    || (!self.settings.raw && self.preview_range().is_some())
                {
                    tr("复制选中文本\tCtrl+C")
                } else {
                    tr("复制可见内容\tCtrl+C")
                },
            )
            .as_ptr(),
        );
        AppendMenuW(
            menu,
            state(self.layout.is_some()),
            SELECT_ALL,
            w(tr("全选当前分页\tCtrl+A")).as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, state(document), FIND, w(tr("查找…\tCtrl+F")).as_ptr());
        AppendMenuW(
            menu,
            state(document),
            RELOAD,
            w(tr("重新载入\tF5")).as_ptr(),
        );
        if document {
            SetMenuDefaultItem(menu, EDIT_DOCUMENT as u32, 0);
        }
        menu
    }
    /// Screen position for WM_CONTEXTMENU, or None when the point is outside the text.
    unsafe fn context_point(&self, lp: LPARAM) -> Option<POINT> {
        if lp as i32 == -1 {
            // Shift+F10 / Menu key: anchor to the selection or the first visible line.
            self.document.as_ref()?;
            // The row holding the selection's start, when it is on screen.
            let row = self
                .preview_range()
                .zip(self.layout.as_ref())
                .and_then(|((from, _), layout)| {
                    layout.rows.iter().position(|r| {
                        r.start == from.block && from.unit < select::row_end(r).max(r.unit + 1)
                    })
                })
                .filter(|row| *row >= self.top)
                .unwrap_or(self.top);
            let y = self
                .hits
                .iter()
                .find(|h| h.action == Hit::Row(row))
                .map_or(self.content_top() + self.s(24), |h| h.rect.top + self.s(8));
            let mut point = POINT {
                x: self.left() + self.s(24),
                y: y.min(self.content_bottom() - self.s(8)),
            };
            ClientToScreen(self.hwnd, &mut point);
            return Some(point);
        }
        let mut point = POINT {
            x: lp as u16 as i16 as i32,
            y: (lp >> 16) as u16 as i16 as i32,
        };
        let screen = point;
        ScreenToClient(self.hwnd, &mut point);
        self.reading_area(point.x, point.y).then_some(screen)
    }
    #[allow(
        clippy::collapsible_match,
        reason = "Keep user-confirmation side effects inside the selected command"
    )]
    unsafe fn command(&mut self, id: usize) {
        self.tooltip_visible = false;
        self.hover_since = None;
        if id != THEME_PICKER && id != THEME_PANEL {
            self.theme_menu = None;
        }
        match id {
            THEME_PICKER => {
                SetFocus(self.hwnd);
                self.theme_menu = if self.theme_menu.is_some() {
                    None
                } else {
                    Some(chrome::picker_index(self.settings.theme))
                };
            }
            THEME_PANEL => {}
            THEME_MORE | PREFERENCES => {
                PostMessageW(self.hwnd, OPEN_PREFERENCES, 0, 0);
            }
            EDITOR_SETTINGS => {
                PostMessageW(self.hwnd, OPEN_PREFERENCES, 2, 0);
            }
            LANGUAGE => {
                PostMessageW(self.hwnd, OPEN_PREFERENCES, 4, 0);
            }
            CHECK_UPDATES => {
                // Opens the update page and checks right away.
                PostMessageW(self.hwnd, OPEN_PREFERENCES, 3, 1);
            }
            EDIT_DOCUMENT => self.edit_document(),
            SELECT_ALL => {
                if self.settings.raw {
                    if let Some(page) = &self.page
                        && let (Some(first), Some(last)) = (page.lines.first(), page.lines.last())
                    {
                        self.text_selection = Some((
                            Caret {
                                line: first.start,
                                index: 0,
                            },
                            Caret {
                                line: last.start,
                                index: last.text.len(),
                            },
                        ));
                        self.status = tr("已全选当前分页").into();
                    }
                } else if let Some(l) = &self.layout
                    && let (Some(first), Some(last)) = (l.rows.first(), l.rows.last())
                {
                    self.selection = Some((
                        Spot {
                            block: first.start,
                            unit: 0,
                        },
                        Spot {
                            block: last.start,
                            unit: select::row_end(last).max(1),
                        },
                    ));
                    self.status = tr("已全选当前分页").into();
                }
            }

            OUTLINE => self.toggle_outline(),
            MINIMIZE => {
                PostMessageW(self.hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
            }
            MAXIMIZE => {
                PostMessageW(
                    self.hwnd,
                    WM_SYSCOMMAND,
                    if IsZoomed(self.hwnd) != 0 {
                        SC_RESTORE
                    } else {
                        SC_MAXIMIZE
                    } as usize,
                    0,
                );
            }
            PREVIEW | SOURCE => {
                if self.settings.raw != (id == SOURCE) {
                    self.settings.raw = id == SOURCE;
                    self.selection = None;
                    self.text_selection = None;
                    self.persist();
                }
            }
            FILE_MENU | HELP_MENU => {
                let menu = GetSubMenu(self.menu, if id == FILE_MENU { 0 } else { 2 });
                let mut point = POINT {
                    x: self.s(20),
                    y: self.s(94),
                };
                ClientToScreen(self.hwnd, &mut point);
                let selected =
                    TrackPopupMenu(menu, TPM_RETURNCMD, point.x, point.y, 0, self.hwnd, null());
                if selected > 0 {
                    self.command(selected as usize);
                }
            }
            LICENSES => self.export_licenses(),
            OPEN => self.dialog(),
            FIND | GOTO => self.show_edit(id),
            NEXT => self.search(),
            COPY => self.copy(),
            EXIT => {
                DestroyWindow(self.hwnd);
            }
            RELOAD => {
                if let Some(doc) = &self.document {
                    let p = doc.path.clone();
                    let anchor = (self.current(), self.wrapped_row(self.top));
                    self.open(p);
                    self.reload_anchor = Some(anchor);
                }
            }
            THEME => {
                // Legacy cycle through the basic skins; gallery skins return to light.
                self.settings.theme = match self.settings.theme {
                    Theme::Light => Theme::Dark,
                    Theme::Dark => Theme::Sepia,
                    _ => Theme::Light,
                };
                self.persist_theme();
            }
            THEME_LIGHT | THEME_DARK | THEME_SYSTEM | THEME_SEPIA => {
                self.settings.theme = match id {
                    THEME_LIGHT => Theme::Light,
                    THEME_DARK => Theme::Dark,
                    THEME_SEPIA => Theme::Sepia,
                    _ => Theme::System,
                };
                self.system_dark = system_uses_dark_theme();
                self.persist_theme();
            }
            ZOOM_IN => {
                self.settings.font_size = (self.settings.font_size + 2).min(36);
                self.persist();
            }
            ZOOM_OUT => {
                self.settings.font_size = (self.settings.font_size - 2).max(12);
                self.persist();
            }
            WRAP => {
                self.settings.wrap = !self.settings.wrap;
                self.persist();
            }
            RAW => {
                self.settings.raw = !self.settings.raw;
                self.selection = None;
                self.text_selection = None;
                self.persist();
            }
            WIDE => {
                self.settings.wide = !self.settings.wide;
                self.persist();
            }
            RESET => {
                // Restores reading settings; update bookkeeping is not a preference.
                self.settings = Settings {
                    update_checked: self.settings.update_checked,
                    update_latest: std::mem::take(&mut self.settings.update_latest),
                    ..Settings::default()
                };
                self.reading_fonts =
                    Fonts::with_family(self.fonts.dpi, self.settings.reading_font());
                self.persist();
                self.start_outline();
            }
            SETTINGS => {
                // The update entry names a newer release when one is known.
                let label = match self.update_available() {
                    Some(tag) => trf("发现新版本 {}…", &[&tag]),
                    None => tr("检查更新…").to_string(),
                };
                ModifyMenuW(
                    self.settings_menu,
                    CHECK_UPDATES as u32,
                    MF_BYCOMMAND | MF_STRING,
                    CHECK_UPDATES,
                    w(&label).as_ptr(),
                );
                let mut point = POINT {
                    x: self.width - self.s(145),
                    y: self.s(HEADER_HEIGHT - 2),
                };
                ClientToScreen(self.hwnd, &mut point);
                let id = TrackPopupMenu(
                    self.settings_menu,
                    TPM_RIGHTALIGN | TPM_RETURNCMD,
                    point.x,
                    point.y,
                    0,
                    self.hwnd,
                    null(),
                );
                if id > 0 {
                    self.command(id as usize);
                }
            }
            REGISTER => {
                self.register_open_with();
            }
            DEFAULT_APP => self.choose_default_app(),
            UNREGISTER if !distribution::store() => {
                if MessageBoxW(
                    self.hwnd,
                    w(tr(
                        "移除 FeatherMD 的打开方式注册？不会删除文件或其他应用的关联。",
                    ))
                    .as_ptr(),
                    w(tr("移除注册")).as_ptr(),
                    MB_OKCANCEL | MB_ICONQUESTION,
                ) == IDOK
                {
                    match crate::association::unregister() {
                        Ok(()) => self.status = tr("已移除打开方式注册").into(),
                        Err(e) => self.error(&e.to_string()),
                    }
                }
            }
            TYPOGRAPHY => {
                PostMessageW(self.hwnd, OPEN_PREFERENCES, 1, 0);
            }
            ABOUT => {
                // Enter the modal loop only after the current App borrow has been released.
                PostMessageW(self.hwnd, WM_APP + 43, 0, 0);
            }
            _ => {}
        }
        self.invalidate();
    }
    unsafe fn keyboard(&mut self, key: usize, ctrl: bool) -> bool {
        if ctrl && key == 0x54 {
            self.command(THEME_PICKER);
            return true;
        }
        if let Some(index) = self.theme_menu {
            if !ctrl {
                let rows = chrome::PICKER.len();
                match key {
                    0x1b => self.theme_menu = None,
                    0x26 => self.theme_menu = Some((index + rows - 1) % rows),
                    0x28 | 0x09 => {
                        self.theme_menu = Some(
                            (index
                                + if key == 0x09 && test_host::key_pressed(VK_SHIFT as i32) {
                                    rows - 1
                                } else {
                                    1
                                })
                                % rows,
                        )
                    }
                    0x0d | 0x20 => self.command(chrome::PICKER[index.min(rows - 1)].0),
                    _ => {}
                }
                self.invalidate();
                return true;
            }
            self.theme_menu = None;
        }

        if ctrl {
            match key {
                0x4f => self.command(OPEN),
                0x46 => self.command(FIND),
                0x47 => self.command(GOTO),
                0x43 => self.command(COPY),
                0x52 => self.command(RAW),
                0x42 => self.command(OUTLINE),
                0x45 => self.command(EDIT_DOCUMENT),
                0xbc => self.command(PREFERENCES),
                0x41 => self.command(SELECT_ALL),
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
            0x1b if self.edit_mode.is_none()
                && (self.selection.is_some() || self.text_selection.is_some()) =>
            {
                self.selection = None;
                self.text_selection = None;
                self.invalidate();
            }
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
                underline: false,
            }),
        );
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        let mut r = rect;
        let text: Vec<u16> = text.encode_utf16().collect();
        snapshot::clip(self.hwnd, dc, &text, rect, flags);
        DrawTextW(dc, text.as_ptr(), text.len() as i32, &mut r, flags);
        SelectObject(dc, old);
    }
    unsafe fn paint(&mut self, supplied: Option<HDC>) {
        let mut ps: PAINTSTRUCT = zeroed();
        let target = supplied.unwrap_or_else(|| BeginPaint(self.hwnd, &mut ps));
        let dc = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, self.width.max(1), self.height.max(1));
        let old_bitmap = SelectObject(dc, bitmap);
        let p = self.palette();
        let all = RECT {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        };
        Self::fill(dc, &all, p.bg);
        self.hits.clear();
        snapshot::clear_clips(self.hwnd);
        SetBkMode(dc, TRANSPARENT as i32);
        self.paint_header(dc, p);
        self.paint_outline(dc, p);
        if self.document.is_none() {
            self.paint_welcome(dc, p);
        } else if self.page.as_ref().is_some_and(|p| p.size == 0) {
            self.label(
                dc,
                tr("这是一份空白文档。"),
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
            let y = self.content_top() + self.s(12);
            let bottom = self.content_bottom();
            self.paint_page(dc, p);
            let saved = SaveDC(dc);
            IntersectClipRect(dc, left, y, left + body_width, bottom);
            self.paint_rows(dc, p, y, bottom);
            RestoreDC(dc, saved);
            self.paint_copy_button(dc, p);
        }
        self.paint_footer(dc, p);
        self.paint_scrollbars(dc, p);
        self.paint_header_overlay(dc, p);
        BitBlt(target, 0, 0, self.width, self.height, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        if supplied.is_none() {
            EndPaint(self.hwnd, &ps);
        }
    }
    unsafe fn test_snapshot(&mut self) -> bool {
        snapshot::capture(self.hwnd, |dc| {
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
        })
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
    if msg == OPEN_PREFERENCES {
        if IsWindowEnabled(hwnd) != 0 {
            let settings = (*ptr).try_borrow().ok().map(|app| app.settings.clone());
            if let Some(settings) = settings {
                // No App borrow may cross the modal loop; drafts arrive as
                // PREVIEW_APPEARANCE, finished update checks as UPDATE_RESULT.
                let saved = preferences::show(hwnd, &settings, wp.min(4), lp == 1);
                if let Ok(mut app) = (*ptr).try_borrow_mut() {
                    match saved {
                        Some(draft) => app.apply_preferences(draft),
                        None => {
                            app.preview = None;
                            app.apply_opacity();
                            app.invalidate();
                            InvalidateRect(app.edit, null(), 1);
                        }
                    }
                }
            }
        }
        return 0;
    }
    if msg == UPDATE_RESULT {
        if lp != 0
            && let Ok(mut app) = (*ptr).try_borrow_mut()
        {
            app.record_update(&*(lp as *const crate::updater::Outcome), true);
        }
        return 0;
    }
    if msg == PREVIEW_APPEARANCE {
        if let Ok(mut app) = (*ptr).try_borrow_mut() {
            app.preview = Theme::ALL
                .get(wp)
                .map(|theme| (*theme, Settings::clamp_opacity(lp as i64)));
            app.apply_opacity();
            app.invalidate();
            InvalidateRect(app.edit, null(), 1);
        }
        return 0;
    }
    if msg == WM_CONTEXTMENU && hwnd == wp as HWND {
        // Build while borrowed, then release App before TrackPopupMenu's modal loop.
        let menu = (*ptr).try_borrow_mut().ok().and_then(|mut app| {
            let point = app.context_point(lp)?;
            app.theme_menu = None;
            app.tooltip_visible = false;
            app.hover_since = None;
            app.pressed = None;
            Some((app.context_menu(), point))
        });
        let Some((menu, point)) = menu else {
            return DefWindowProcW(hwnd, msg, wp, lp);
        };
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        );
        DestroyMenu(menu);
        if selected > 0
            && let Ok(mut app) = (*ptr).try_borrow_mut()
        {
            app.command(selected as usize);
        }
        return 0;
    }
    if msg == WM_APP + 43 {
        if IsWindowEnabled(hwnd) != 0 {
            let theme = (*ptr).try_borrow().ok().map(|app| app.settings.theme);
            if let Some(theme) = theme {
                about::show(hwnd, theme);
            }
        }
        return 0;
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
            38 => app.settings.raw as isize,
            // Saved skin index in Theme::ALL order (0 system, 1 light, 2 dark, 3 sepia, ...).
            39 => app.settings.theme.index() as isize,
            40 => app.active_theme().is_dark() as isize,
            41 => app.system_dark as isize,
            42 => app.theme_menu.map_or(-1, |i| i as isize),
            43 => app.tooltip_visible as isize,
            44 => app.hover.map_or(-1, |id| id as isize),
            45 => app.theme_menu_rect().top as isize,
            46 => app.outline_active().map_or(-1, |i| i as isize),
            // On-screen skin including an unsaved Settings draft; -1 when none.
            47 => app.preview.map_or(-1, |(theme, _)| theme.index() as isize),
            // Applied window alpha (255 when not layered).
            48 => {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
                let mut alpha = 255u8;
                if style & WS_EX_LAYERED != 0 {
                    let mut flags = 0;
                    GetLayeredWindowAttributes(hwnd, null_mut(), &mut alpha, &mut flags);
                }
                alpha as isize
            }
            49 => app.s(HEADER_HEIGHT) as isize,
            // Whether a right-click at client point LPARAM would open the text menu.
            50 => {
                app.reading_area(lp as u16 as i16 as i32, (lp >> 16) as u16 as i16 as i32) as isize
            }
            // Automatic update checks started, a known newer release, a check in flight.
            51 => app.update_checks as isize,
            52 => app.update_available().is_some() as isize,
            53 => app.update_check.is_some() as isize,
            97 => distribution::store() as isize,
            98 => matches!(distribution::identity(), distribution::Identity::Package(_)) as isize,
            // Opt-in package smoke diagnostics; never redirect settings through a test variable.
            99 => match std::env::var_os("FEATHERMD_TEST_SNAPSHOT") {
                Some(path) => {
                    let document = app
                        .document
                        .as_ref()
                        .map(|d| d.path.display().to_string())
                        .unwrap_or_default();
                    let report = format!("{}\n{}\n", app.settings_path.display(), document);
                    std::fs::write(
                        PathBuf::from(path).with_file_name("distribution.txt"),
                        report,
                    )
                    .map_or(-1, |_| 1)
                }
                None => -1,
            },
            // Tables: right edge of the widest table, the reading width, visual table
            // lines, distinct table rows and the first table's column count.
            54 => app.layout.as_ref().map_or(0, |l| l.table_width as isize),
            55 => app.body_width() as isize,
            // Per table LPARAM of this page: right edge, visual lines, rows, columns,
            // first layout row; 65 is the number of tables.
            56..=58 | 64 => app.tables().get(lp as usize).map_or(-1, |t| match wp {
                56 => t.0 as isize,
                57 => t.1 as isize,
                58 => t.3 as isize,
                _ => t.4 as isize,
            }),
            65 => app.tables().len() as isize,
            66 => app.tables().get(lp as usize).map_or(-1, |t| t.2 as isize),
            // Layout row LPARAM on screen: client left / right of its text and middle y.
            59..=61 => {
                let index = lp as usize;
                app.hits
                    .iter()
                    .find(|h| h.action == Hit::Row(index))
                    .zip(app.layout.as_ref().and_then(|l| l.rows.get(index)))
                    .map_or(-1, |(hit, row)| {
                        let left = app.left() - app.shift(row);
                        let value = match wp {
                            59 => left + row.runs.first().map_or(0, |r| r.x),
                            60 => left + row.runs.last().map_or(0, |r| r.x + r.width),
                            _ => (hit.rect.top + hit.rect.bottom) / 2,
                        };
                        value as isize
                    })
            }
            // Test seam: write what Copy would put on the clipboard next to the
            // snapshot file and return its UTF-8 length (-1 without a test path).
            62 => match (std::env::var_os("FEATHERMD_TEST_SNAPSHOT"), app.copy_text()) {
                (Some(path), Some((text, _))) => {
                    let path = PathBuf::from(path).with_file_name("copy.txt");
                    std::fs::write(&path, text.as_bytes()).map_or(-1, |_| text.len() as isize)
                }
                _ => -1,
            },
            // Source selection present and non-empty.
            63 => {
                (app.text_selection.is_some_and(|(a, b)| a != b) || app.preview_range().is_some())
                    as isize
            }
            // Preview: client x of the boundary before unit HIWORD(LPARAM) of layout
            // row LOWORD(LPARAM).
            91 => {
                let (index, unit) = ((lp & 0xffff) as usize, (lp >> 16) as usize);
                // A plain reference, so the layout and the font cache borrow apart.
                let app = &mut *app;
                match app.layout.as_ref().and_then(|l| l.rows.get(index)) {
                    Some(row) => {
                        let origin = app.left() - app.shift(row);
                        let dc = GetDC(hwnd);
                        let fonts = &mut app.reading_fonts;
                        let x = select::x_of(row, unit, |run| pens(dc, fonts, run));
                        ReleaseDC(hwnd, dc);
                        (origin + x) as isize
                    }
                    None => -1,
                }
            }
            // Copy boxes: count; per box LPARAM its kind (0 fenced, 1 indented,
            // 2 quote, 3 table), first and last layout row.
            67 => app.boxes.len() as isize,
            68..=70 => app.boxes.get(lp as usize).map_or(-1, |b| match wp {
                68 => b.kind as isize,
                69 => b.rows.0 as isize,
                _ => b.rows.1 as isize,
            }),
            // The copy button painted last: left, top, right, bottom, its box and
            // whether it shows the check mark; -1 when no button is shown.
            71..=76 => app
                .hits
                .iter()
                .find_map(|h| match h.action {
                    Hit::CopyBlock(index) => Some((h.rect, index)),
                    _ => None,
                })
                .map_or(-1, |(r, index)| match wp {
                    71 => r.left as isize,
                    72 => r.top as isize,
                    73 => r.right as isize,
                    74 => r.bottom as isize,
                    75 => index as isize,
                    _ => app.just_copied(index) as isize,
                }),
            // Interface language index (zh-CN, zh-TW, en, ja); labels of the last
            // paint that did not fit (listed in labels.txt; -1 outside tests); the
            // menu labels written to menus.txt, one per line, and their count.
            77 => i18n::current().index() as isize,
            78 => snapshot::report(hwnd),
            79 => match std::env::var_os("FEATHERMD_TEST_SNAPSHOT") {
                Some(path) => {
                    let mut labels = Vec::new();
                    menu_labels(app.menu, &mut labels);
                    let path = PathBuf::from(path).with_file_name("menus.txt");
                    std::fs::write(path, labels.join("\r\n")).map_or(-1, |_| labels.len() as isize)
                }
                None => -1,
            },
            // Left edge of the reading column (its width is query 55).
            80 => app.left() as isize,
            // Images: decoded image runs in the layout, images of this page still
            // being read, decoded bytes held, images drawn by the last paint and
            // the client rectangle of drawn image LPARAM.
            81 => app.layout.as_ref().map_or(0, |l| {
                l.rows
                    .iter()
                    .flat_map(|r| &r.runs)
                    .filter(|r| r.picture.is_some())
                    .count() as isize
            }),
            82 => app.pictures.pending() as isize,
            83 => app.pictures.bytes() as isize,
            90 => app.pictures.bitmaps() as isize,
            // Visible manual image actions and their rectangles (diagnostics).
            92 => app
                .hits
                .iter()
                .filter(|h| matches!(h.action, Hit::LoadImage(_)))
                .count() as isize,
            93..=96 => app
                .hits
                .iter()
                .filter(|h| matches!(h.action, Hit::LoadImage(_)))
                .nth(lp as usize)
                .map_or(-1, |h| match wp {
                    93 => h.rect.left,
                    94 => h.rect.top,
                    95 => h.rect.right,
                    _ => h.rect.bottom,
                } as isize),
            84 => app.painted_pictures.len() as isize,
            85..=88 => app
                .painted_pictures
                .get(lp as usize)
                .map_or(-1, |r| match wp {
                    85 => r.left,
                    86 => r.top,
                    87 => r.right,
                    _ => r.bottom,
                } as isize),
            // What a click at client point LPARAM hits: 1 link, 2 text row, 3 a
            // command, 4 another target, 0 nothing.
            89 => match app.hit(lp as u16 as i16 as i32, (lp >> 16) as u16 as i16 as i32) {
                Some(Hit::Link(_)) => 1,
                Some(Hit::Row(_)) => 2,
                Some(Hit::Command(_)) => 3,
                Some(_) => 4,
                None => 0,
            },
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
        WM_KILLFOCUS | WM_NCLBUTTONDOWN | WM_ENTERSIZEMOVE => {
            app.theme_menu = None;
            app.tooltip_visible = false;
            app.hover_since = None;
            app.invalidate();
            drop(app);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_ERASEBKGND => 1,
        WM_PRINTCLIENT | WM_PRINT => {
            app.paint(Some(wp as HDC));
            0
        }
        WM_PAINT => {
            app.paint(None);
            0
        }
        WM_TIMER if wp == UPDATE_TIMER => {
            KillTimer(hwnd, UPDATE_TIMER);
            // The preference may have been turned off since launch.
            if app.settings.update_check {
                app.start_update_check();
            }
            0
        }
        WM_TIMER => {
            app.receive();
            app.drag_scroll();
            // The copy button's check mark returns to "Copy".
            if app.copied.is_some_and(|(_, at)| at.elapsed() >= COPIED_FOR) {
                app.copied = None;
                app.invalidate();
            }
            if !app.tooltip_visible
                && app.header_tip().is_some()
                && app
                    .hover_since
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(450))
            {
                app.tooltip_visible = true;
                app.invalidate();
            }
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            app.system_dark = system_uses_dark_theme();
            app.invalidate();
            InvalidateRect(app.edit, null(), 1);
            0
        }
        WM_SIZE => {
            if wp != SIZE_MINIMIZED as usize {
                app.width = (lp as u32 & 0xffff) as i32;
                app.height = ((lp as u32 >> 16) & 0xffff) as i32;
                app.drag = None;
                app.hover = None;
                app.pressed = None;
                app.theme_menu = None;
                app.tooltip_visible = false;
                app.hover_since = None;
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
                x: app.s(MIN_SIZE.0),
                y: app.s(MIN_SIZE.1),
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
            if y < app.s(HEADER_HEIGHT) && !matches!(app.hit(x, y), Some(Hit::Command(_))) {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_DPICHANGED => {
            let anchor = app.current();
            app.fonts = Fonts::new((wp & 0xffff) as u32);
            app.reading_fonts = Fonts::with_family(app.fonts.dpi, app.settings.reading_font());
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
                underline: false,
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
            if app.keyboard(wp, test_host::key_pressed(VK_CONTROL as i32)) {
                0
            } else {
                drop(app);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
        }
        WM_MOUSEWHEEL if app.theme_menu.is_some() => 0,
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
            if point.x >= 0 && point.x < app.sidebar_width() && point.y >= app.s(HEADER_HEIGHT) {
                app.scroll_outline(-steps * 3);
                return 0;
            }
            if test_host::key_pressed(VK_CONTROL as i32) {
                app.command(if steps > 0 { ZOOM_IN } else { ZOOM_OUT });
            } else if test_host::key_pressed(VK_SHIFT as i32) {
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
            app.tooltip_visible = false;
            app.hover_since = None;
            if app.theme_menu.is_some() {
                let r = app.theme_menu_rect();
                if y >= app.s(HEADER_HEIGHT)
                    && !(x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                {
                    app.theme_menu = None;
                    app.invalidate();
                    return 0;
                }
                if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                    app.pressed = app.hit(x, y);
                    SetCapture(hwnd);
                    app.invalidate();
                    return 0;
                }
            }
            if app.begin_scrollbar(x, y) {
                return 0;
            }
            app.pressed = app.hit(x, y);
            if matches!(
                app.pressed,
                Some(
                    Hit::Command(_)
                        | Hit::Link(_)
                        | Hit::LoadImage(_)
                        | Hit::Heading(_)
                        | Hit::CopyBlock(_)
                )
            ) {
                SetCapture(hwnd);
                app.invalidate();
            }
            if let Some(Hit::Row(row)) = app.hit(x, y) {
                if app.settings.raw {
                    // Source mode selects characters: double-click a word, Shift extends.
                    if let Some((caret, under)) = app.caret_at(row, x) {
                        if msg == WM_LBUTTONDBLCLK {
                            let word = app
                                .page
                                .as_ref()
                                .and_then(|p| line_of(&p.lines, caret.line))
                                .map(|l| text::word_at(&l.text, under));
                            if let Some((a, b)) = word {
                                app.text_selection = Some((
                                    Caret {
                                        line: caret.line,
                                        index: a,
                                    },
                                    Caret {
                                        line: caret.line,
                                        index: b,
                                    },
                                ));
                            }
                        } else {
                            let anchor = match app.text_selection {
                                Some((anchor, _)) if wp & MK_SHIFT != 0 => anchor,
                                _ => caret,
                            };
                            app.text_selection = Some((anchor, caret));
                            app.selecting = true;
                            app.drag_point = Some((x, y));
                            SetCapture(hwnd);
                        }
                    }
                } else if let Some(spot) = app.spot_at(row, x) {
                    // The preview selects characters too, as shown on screen.
                    if msg == WM_LBUTTONDBLCLK {
                        let table = app
                            .layout
                            .as_ref()
                            .and_then(|l| l.rows.get(row))
                            .is_some_and(|r| r.table.is_some());
                        app.selection = Some(if table {
                            (Spot { unit: 0, ..spot }, Spot { unit: 1, ..spot })
                        } else {
                            let rows = app.layout.as_ref().map_or(&[][..], |l| &l.rows[..]);
                            select::word(rows, spot.block, spot.unit)
                        });
                    } else {
                        let anchor = match app.selection {
                            Some((anchor, _)) if wp & MK_SHIFT != 0 => anchor,
                            _ => spot,
                        };
                        app.selection = Some((anchor, spot));
                        app.selecting = true;
                        app.drag_point = Some((x, y));
                        SetCapture(hwnd);
                    }
                }
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
            if app.hover != hover {
                app.hover_since = hover.map(|_| Instant::now());
                app.tooltip_visible = false;
                if app.theme_menu.is_some()
                    && let Some(index) =
                        chrome::PICKER.iter().position(|(id, _)| Some(*id) == hover)
                {
                    app.theme_menu = Some(index);
                }
            }
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
            // Hidden diagnostic windows receive synthetic moves, not desktop hover.
            if IsWindowVisible(hwnd) != 0 {
                TrackMouseEvent(&mut tracking);
            }
            if app.drag.is_some() {
                app.drag_scrollbar(x, y);
            } else if app.selecting {
                app.drag_point = Some((x, y));
                app.drag_to(x, y);
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
                app.drag_point = None;
                ReleaseCapture();
            } else {
                ReleaseCapture();
                let hit = app.hit(x, y);
                if pressed != hit {
                    app.invalidate();
                    return 0;
                }
                match hit {
                    Some(Hit::CopyBlock(index)) => app.copy_box(index),
                    Some(Hit::Heading(index)) => app.select_heading(index),
                    Some(Hit::Command(0)) => app.hide_edit(),
                    Some(Hit::Command(NEXT)) if app.edit_mode.is_some() => app.submit(),
                    Some(Hit::Command(id)) => app.command(id),
                    Some(Hit::LoadImage(url)) => {
                        let ticket = app.io_ticket;
                        drop(app);
                        // Release the App borrow so timers and paint can run in
                        // the confirmation's modal loop. Only this URL is allowed.
                        let accepted = MessageBoxW(hwnd,
                            w(&trf("下载并显示这一张网络图片？\n\n{}\n\n图片网站会收到你的 IP 地址；仅缓存在内存中，不保存文件。", &[&url])).as_ptr(),
                            w(tr("加载网络图片")).as_ptr(), MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2) == IDOK;
                        if accepted
                            && let Ok(mut app) = (*ptr).try_borrow_mut()
                            && app.io_ticket == ticket
                            && !app.settings.raw
                        {
                            app.pictures.request_web(&url);
                            app.pictures_changed = true;
                            app.invalidate();
                        }
                        return 0;
                    }
                    Some(Hit::Link(url)) => {
                        if markdown::safe_web_link(&url) {
                            if MessageBoxW(
                                hwnd,
                                w(&trf("使用默认浏览器打开此链接？\n\n{}", &[&url])).as_ptr(),
                                w(tr("打开外部链接")).as_ptr(),
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
                            app.status =
                                tr("仅允许 http / https 链接；本地链接和脚本不会执行").into();
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
            app.drag_point = None;
            app.drag = None;
            app.pressed = None;
            if msg == WM_CANCELMODE {
                ReleaseCapture();
            }
            app.invalidate();
            0
        }
        WM_MOUSELEAVE => {
            app.tooltip_visible = false;
            app.hover_since = None;
            app.hover = None;
            app.mouse = None;
            app.invalidate();
            0
        }
        WM_SETCURSOR if lp as u16 as u32 == HTCLIENT => {
            let clickable = app.mouse.is_some_and(|(x, y)| {
                matches!(
                    app.hit(x, y),
                    Some(
                        Hit::Command(_)
                            | Hit::Link(_)
                            | Hit::LoadImage(_)
                            | Hit::Heading(_)
                            | Hit::CopyBlock(_)
                    )
                )
            });
            // Source text is selectable by character.
            let text = app.settings.raw
                && app
                    .mouse
                    .is_some_and(|(x, y)| app.reading_area(x, y) && !clickable);
            SetCursor(LoadCursorW(
                null_mut(),
                if clickable {
                    IDC_HAND
                } else if text {
                    IDC_IBEAM
                } else {
                    IDC_ARROW
                },
            ));
            1
        }
        WM_CTLCOLOREDIT => {
            let p = app.palette();
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
/// Diagnostics: every item label of `menu` and its submenus, depth first.
unsafe fn menu_labels(menu: HMENU, out: &mut Vec<String>) {
    for position in 0..GetMenuItemCount(menu).max(0) as u32 {
        let mut text = [0u16; 256];
        let len = GetMenuStringW(menu, position, text.as_mut_ptr(), 256, MF_BYPOSITION);
        if len > 0 {
            out.push(String::from_utf16_lossy(&text[..len as usize]));
        }
        let sub = GetSubMenu(menu, position as i32);
        if !sub.is_null() {
            menu_labels(sub, out);
        }
    }
}
unsafe fn menus() -> (HMENU, HMENU) {
    let menu = CreateMenu();
    let file = CreatePopupMenu();
    for (id, text) in [
        (OPEN, tr("打开…\tCtrl+O")),
        (FIND, tr("查找…\tCtrl+F")),
        (GOTO, tr("跳转到百分比…\tCtrl+G")),
        (RELOAD, tr("重新载入\tF5")),
        (COPY, tr("复制选中文本 / 可见内容\tCtrl+C")),
        (EXIT, tr("退出")),
    ] {
        add(file, id, text);
    }
    AppendMenuW(menu, MF_POPUP, file as usize, w(tr("文件(&F)")).as_ptr());
    let settings = CreatePopupMenu();
    add(settings, PREFERENCES, tr("设置…\tCtrl+,"));
    // Bilingual, so a reader who cannot read the current language still finds it.
    add(settings, LANGUAGE, tr("界面语言 / Language…"));
    add(settings, EDIT_DOCUMENT, tr("用默认编辑应用打开\tCtrl+E"));
    AppendMenuW(settings, MF_SEPARATOR, 0, null());
    for (id, text) in [
        (GOTO, tr("跳转到百分比…\tCtrl+G")),
        (OUTLINE, tr("显示目录\tCtrl+B")),
        (TYPOGRAPHY, tr("字体与默认字号…")),
        (ZOOM_IN, tr("放大字号\tCtrl++")),
        (ZOOM_OUT, tr("缩小字号\tCtrl+-")),
        (WRAP, tr("自动换行")),
        (WIDE, tr("宽屏阅读")),
        (RESET, tr("恢复默认阅读设置")),
        (RELOAD, tr("重新载入\tF5")),
        (COPY, tr("复制选中文本 / 可见内容\tCtrl+C")),
        (CHECK_UPDATES, tr("检查更新…")),
        (ABOUT, tr("关于 FeatherMD…\tF1")),
        (LICENSES, tr("导出内置许可证…")),
    ] {
        add(settings, id, text);
    }
    AppendMenuW(settings, MF_SEPARATOR, 0, null());
    let associations = CreatePopupMenu();
    if !distribution::store() {
        add(associations, REGISTER, tr("加入 Markdown 右键打开方式"));
    }
    add(
        associations,
        DEFAULT_APP,
        tr("设为 .md 默认应用（系统确认）…"),
    );
    if !distribution::store() {
        AppendMenuW(associations, MF_SEPARATOR, 0, null());
        add(associations, UNREGISTER, tr("移除打开方式注册…"));
    }
    AppendMenuW(
        settings,
        MF_POPUP,
        associations as usize,
        w(tr("文件关联")).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_POPUP,
        settings as usize,
        w(tr("设置(&S)")).as_ptr(),
    );
    let help = CreatePopupMenu();
    add(help, ABOUT, tr("关于 FeatherMD…\tF1"));
    add(help, LICENSES, tr("导出内置许可证…"));
    AppendMenuW(menu, MF_POPUP, help as usize, w(tr("帮助(&H)")).as_ptr());
    (menu, settings)
}
pub fn run(path: Option<PathBuf>) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _test_host = test_host::Host::start()?;
        let _graphics = smooth::Session::start()?;
        // ShellExecuteEx may use shell extensions that require an STA thread.
        let com = CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
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
        let app = Box::new(RefCell::new(App::new(
            Settings::path().map_err(|e| e.to_string())?,
        )));
        let (menu, settings_menu) = menus();
        let hwnd = CreateWindowExW(
            WS_EX_ACCEPTFILES,
            class.as_ptr(),
            w(tr("FeatherMD 羽读")).as_ptr(),
            (WS_OVERLAPPEDWINDOW & !WS_CAPTION) | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            DEFAULT_SIZE.0,
            DEFAULT_SIZE.1,
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
        // Per-monitor aware windows are created in physical pixels, so scale the
        // default size to the monitor Windows chose before the window is shown.
        let mut frame = RECT::default();
        GetWindowRect(hwnd, &mut frame);
        let mut monitor = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..zeroed()
        };
        let placed = GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        ) != 0;
        if placed {
            let origin = POINT {
                x: frame.left,
                y: frame.top,
            };
            frame = initial_frame(GetDpiForWindow(hwnd), origin, monitor.rcWork);
        }
        SetWindowPos(
            hwnd,
            null_mut(),
            frame.left,
            frame.top,
            frame.right - frame.left,
            frame.bottom - frame.top,
            SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_FRAMECHANGED
                | if placed { 0 } else { SWP_NOMOVE | SWP_NOSIZE },
        );
        {
            let mut a = app.borrow_mut();
            a.hwnd = hwnd;
            a.menu = menu;
            a.settings_menu = settings_menu;
            a.fonts = Fonts::new(GetDpiForWindow(hwnd));
            a.reading_fonts = Fonts::with_family(a.fonts.dpi, a.settings.reading_font());
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
                underline: false,
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
            a.apply_opacity();
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
        // Opt-in startup check: after first paint, at most once a day.
        {
            let a = app.borrow();
            if !distribution::store()
                && feathermd::update::due(
                    a.settings.update_check,
                    a.settings.update_checked,
                    feathermd::update::now(),
                )
            {
                SetTimer(hwnd, UPDATE_TIMER, UPDATE_DELAY_MS, None);
            }
        }
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
                if test_host::key_pressed(VK_CONTROL as i32)
                    && matches!(msg.wParam, 0x4f | 0x46 | 0x47 | 0x54 | 0x45 | 0xbc)
                {
                    a.keyboard(msg.wParam, true);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        DestroyMenu(app.borrow().menu);
        if com >= 0 {
            CoUninitialize();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// WCAG relative-luminance contrast of two COLORREF values.
    fn contrast(a: u32, b: u32) -> f64 {
        let luminance = |c: u32| {
            let channel = |v: u32| {
                let v = f64::from(v & 255) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c) + 0.7152 * channel(c >> 8) + 0.0722 * channel(c >> 16)
        };
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }
    #[test]
    fn every_skin_keeps_readable_contrast() {
        for theme in Theme::ALL {
            let p = Palette::new(theme.resolve(false));
            let name = skin_name(theme);
            assert!(contrast(p.fg, p.page) >= 9.8, "{name} body text");
            assert!(contrast(p.fg, p.panel) >= 7.0, "{name} chrome text");
            assert!(contrast(p.fg, p.code) >= 7.0, "{name} code rows");
            assert!(contrast(p.fg, p.selection) >= 4.5, "{name} selected text");
            assert!(contrast(p.muted, p.page) >= 4.5, "{name} secondary text");
            assert!(contrast(p.muted, p.panel) >= 4.3, "{name} footer text");
            assert!(contrast(p.accent, p.page) >= 4.5, "{name} links");
            // The primary button draws page-coloured text on the accent.
            assert!(contrast(p.page, p.accent) >= 4.5, "{name} primary button");
            assert!(!skin_name(theme).is_empty());
        }
    }
    #[test]
    fn source_selection_stands_out_on_code_rows_and_keeps_text_readable() {
        for theme in Theme::ALL {
            let p = Palette::new(theme.resolve(false));
            let name = skin_name(theme);
            let mark = p.text_highlight();
            // Measured 1.43-1.89:1 on code rows and >= 6.0:1 for text in 2026-10.
            assert!(
                contrast(mark, p.code) >= 1.4,
                "{name} selection on code rows"
            );
            assert!(contrast(p.fg, mark) >= 4.5, "{name} selected source text");
        }
    }
    #[test]
    fn default_window_follows_display_scaling_inside_the_work_area() {
        let rect = |left, top, right, bottom| RECT {
            left,
            top,
            right,
            bottom,
        };
        let at = |x, y| POINT { x, y };
        let edges = |r: RECT| (r.left, r.top, r.right, r.bottom);
        let size = |r: RECT| (r.right - r.left, r.bottom - r.top);
        let screen = rect(0, 0, 2560, 1552);
        // The same logical size at every scale; a failed DPI query (0) acts as 100%.
        assert_eq!(size(initial_frame(96, at(26, 26), screen)), (1080, 820));
        assert_eq!(size(initial_frame(120, at(26, 26), screen)), (1350, 1025));
        assert_eq!(size(initial_frame(144, at(26, 26), screen)), (1620, 1230));
        assert_eq!(size(initial_frame(0, at(26, 26), screen)), (1080, 820));
        // A cascade position that would overflow moves back inside the work area.
        assert_eq!(
            edges(initial_frame(144, at(1200, 600), screen)),
            (940, 322, 2560, 1552)
        );
        // Small work areas cap the size, but never below the minimum size.
        assert_eq!(
            edges(initial_frame(96, at(0, 0), rect(0, 0, 1024, 728))),
            (0, 0, 1024, 728)
        );
        assert_eq!(
            edges(initial_frame(192, at(0, 0), rect(0, 0, 1920, 1032))),
            (0, 0, 1920, 1032)
        );
        assert_eq!(
            size(initial_frame(144, at(0, 0), rect(0, 0, 800, 560))),
            (960, 720)
        );
        // Monitors left of the primary one have negative coordinates.
        assert_eq!(
            edges(initial_frame(144, at(-2500, 10), rect(-1920, 0, 0, 1040))),
            (-1920, 0, -300, 1040)
        );
    }
}
