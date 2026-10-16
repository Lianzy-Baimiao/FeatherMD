//! Themed native About dialog. Standard child controls retain keyboard/UIA support.
use super::{chrome::chrome_round, *};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_NOFOCUSRECT, ODS_SELECTED};

const GITHUB: &str = "https://github.com/Lianzy-Baimiao/FeatherMD";
const SUPPORT: &str = "https://ifdian.net/a/lianzy";
const GITHUB_BUTTON: usize = 1001;
const SUPPORT_BUTTON: usize = 1002;
const DETAILS: usize = 1003;
const SS_LEFTNOWORDWRAP: u32 = 0x000c;
/// SS_CENTER | SS_CENTERIMAGE: one line centred both ways.
const SS_CENTER_MIDDLE: u32 = 0x0201;
const WIDTH: i32 = 520;
const HEIGHT: i32 = 480;
/// Extra height of the expanded shortcuts and notes panel.
const DETAILS_HEIGHT: i32 = 260;

fn destination(button: usize) -> Option<&'static str> {
    match button {
        GITHUB_BUTTON => Some(GITHUB),
        SUPPORT_BUTTON => Some(SUPPORT),
        _ => None,
    }
}

pub(super) unsafe fn open_link(owner: HWND, url: &str) {
    if MessageBoxW(
        owner,
        w(&trf("使用默认浏览器打开此链接？\n\n{}", &[&url])).as_ptr(),
        w(tr("打开外部链接")).as_ptr(),
        MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
    ) != IDOK
    {
        return;
    }
    if ShellExecuteW(
        owner,
        w("open").as_ptr(),
        w(url).as_ptr(),
        null(),
        null(),
        SW_SHOWNORMAL,
    ) as isize
        <= 32
    {
        MessageBoxW(
            owner,
            w(&trf("无法打开默认浏览器，请手动访问：\n\n{}", &[&url])).as_ptr(),
            w(tr("打开链接失败")).as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Id, text, logical rect, font size, bold, muted.
type Label = (usize, &'static str, [i32; 4], i32, bool, bool);
struct Control {
    hwnd: HWND,
    id: usize,
    rect: [i32; 4],
    size: i32,
    bold: bool,
    muted: bool,
    details: bool,
    footer: bool,
}
struct About {
    hwnd: HWND,
    owner: HWND,
    theme: Theme,
    system_dark: bool,
    fonts: Fonts,
    expanded: bool,
    controls: Vec<Control>,
}
impl About {
    fn palette(&self) -> Palette {
        Palette::new(self.theme.resolve(self.system_dark))
    }
    fn s(&self, n: i32) -> i32 {
        self.fonts.scale(n)
    }
    fn rect(&self, x: i32, y: i32, width: i32, height: i32) -> RECT {
        RECT {
            left: self.s(x),
            top: self.s(y),
            right: self.s(x + width),
            bottom: self.s(y + height),
        }
    }
    fn key(size: i32, bold: bool) -> FontKey {
        FontKey {
            size,
            bold,
            italic: false,
            mono: false,
            strike: false,
            underline: false,
        }
    }
    unsafe fn create(&mut self) -> bool {
        // Native STATIC/BUTTON controls expose names, roles, focus and activation to UIA.
        let mut labels: Vec<Label> = vec![
            (2000, tr("关于"), [28, 19, 160, 22], 12, false, true),
            (2001, "FeatherMD", [108, 73, 260, 40], 29, true, false),
            (
                2002,
                tr("羽读 / Markdown 阅读器"),
                [110, 117, 270, 24],
                13,
                false,
                true,
            ),
            (
                2003,
                concat!("v", env!("CARGO_PKG_VERSION")),
                [397, 86, 74, 24],
                12,
                false,
                true,
            ),
            (
                2004,
                tr("轻一点，读更多。"),
                [32, 164, 448, 32],
                22,
                true,
                false,
            ),
            (
                2005,
                tr("原生 Windows   ·   轻量便携   ·   默认不联网"),
                [32, 204, 456, 24],
                13,
                false,
                true,
            ),
            (
                2006,
                tr("由 lianzy 制作  ·  MIT 开源许可"),
                [32, 344, 350, 22],
                12,
                false,
                true,
            ),
            (
                2007,
                tr("项目永久免费，赞赏完全自愿。"),
                [32, 437, 320, 20],
                11,
                false,
                true,
            ),
        ];
        // Expanded details: two columns of key and action, then reading notes.
        let columns = [
            [
                ("Ctrl+O", tr("打开文档")),
                ("Ctrl+F", tr("查找")),
                ("F3", tr("查找下一个")),
                ("Ctrl+G", tr("跳转到百分比")),
                ("F5", tr("重新载入")),
                ("Ctrl+C", tr("复制")),
                ("Ctrl+A", tr("全选当前分页")),
            ],
            [
                ("Ctrl+R", tr("预览 / 源码")),
                ("Ctrl+B", tr("显示目录")),
                ("Ctrl+T", tr("外观")),
                ("Ctrl+,", tr("设置")),
                ("Ctrl+E", tr("用编辑应用打开")),
                (tr("Ctrl+滚轮"), tr("缩放字号")),
                ("Esc", tr("取消选择")),
            ],
        ];
        for (column, rows) in columns.into_iter().enumerate() {
            let [key_x, key_width, action_x, action_width] = if column == 0 {
                [48, 60, 112, 136]
            } else {
                [260, 96, 358, 118]
            };
            for (row, (key, action)) in rows.into_iter().enumerate() {
                let y = 414 + row as i32 * 22;
                let id = 2010 + column * 7 + row;
                labels.push((id, key, [key_x, y, key_width, 22], 12, true, false));
                labels.push((
                    id + 20,
                    action,
                    [action_x, y, action_width, 22],
                    12,
                    false,
                    false,
                ));
            }
        }
        for (i, note) in [
            tr("代码块、引用和表格：鼠标移到右上角可一键复制；表格随窗口宽度自动排版。"),
            tr("预览与源码都按字符选择，双击选中词语。"),
            tr("本机图片可在设置 → 阅读中开启；网络图片需逐张点击并确认后加载。"),
            tr("预览按有界分页显示；跨页语法可能不完整，可在源码模式核对。"),
        ]
        .into_iter()
        .enumerate()
        {
            labels.push((
                2050 + i,
                note,
                [48, 576 + i as i32 * 20, 428, 20],
                11,
                false,
                true,
            ));
        }
        for (id, text, rect, size, bold, muted) in labels {
            let hwnd = CreateWindowExW(
                0,
                w("STATIC").as_ptr(),
                w(text).as_ptr(),
                WS_CHILD
                    | WS_VISIBLE
                    // The version sits centred in its pill.
                    | if id == 2003 {
                        SS_CENTER_MIDDLE
                    } else {
                        SS_LEFTNOWORDWRAP
                    },
                0,
                0,
                0,
                0,
                self.hwnd,
                id as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            if hwnd.is_null() {
                return false;
            }
            self.controls.push(Control {
                hwnd,
                id,
                rect,
                size,
                bold,
                muted,
                details: id >= 2010,
                footer: id == 2007,
            });
        }
        for (id, text, rect) in [
            (
                GITHUB_BUTTON,
                tr("GitHub · 项目主页 · 源码、反馈与更新"),
                [32, 252, 222, 76],
            ),
            (
                SUPPORT_BUTTON,
                tr("爱发电 · 支持作者 · 自愿赞赏"),
                [266, 252, 222, 76],
            ),
            (DETAILS, tr("快捷键与阅读说明"), [24, 374, 280, 28]),
            (IDCANCEL as usize, tr("返回阅读"), [360, 426, 128, 34]),
            (IDCLOSE as usize, tr("关闭关于窗口"), [464, 12, 36, 32]),
        ] {
            let hwnd = CreateWindowExW(
                0,
                w("BUTTON").as_ptr(),
                w(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                0,
                0,
                0,
                0,
                self.hwnd,
                id as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            if hwnd.is_null() {
                return false;
            }
            SetWindowSubclass(hwnd, Some(button_proc), 1, 0);
            self.controls.push(Control {
                hwnd,
                id,
                rect,
                size: 13,
                bold: false,
                muted: false,
                details: false,
                footer: id == IDCANCEL as usize,
            });
        }
        true
    }
    unsafe fn layout(&mut self, center: bool) {
        let logical_height = HEIGHT + if self.expanded { DETAILS_HEIGHT } else { 0 };
        let mut monitor: MONITORINFO = zeroed();
        monitor.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(
            MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let area = monitor.rcWork;
        // Fit the complete modal dialog, including Close, even on a small work area.
        let dpi = GetDpiForWindow(self.hwnd)
            .max(96)
            .min(((area.right - area.left - 24).max(1) * 96 / WIDTH) as u32)
            .min(((area.bottom - area.top - 24).max(1) * 96 / logical_height) as u32)
            .max(48);
        let old_fonts = if dpi != self.fonts.dpi {
            Some(std::mem::replace(&mut self.fonts, Fonts::new(dpi)))
        } else {
            None
        };
        let mut anchor: RECT = zeroed();
        GetWindowRect(if center { self.owner } else { self.hwnd }, &mut anchor);
        let width = self.s(WIDTH);
        let height = self.s(logical_height);
        let x = if center {
            (anchor.left + anchor.right - width) / 2
        } else {
            anchor.left
        };
        let y = if center {
            (anchor.top + anchor.bottom - height) / 2
        } else {
            anchor.top
        };
        SetWindowPos(
            self.hwnd,
            null_mut(),
            x.clamp(area.left, (area.right - width).max(area.left)),
            y.clamp(area.top, (area.bottom - height).max(area.top)),
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        for control in &self.controls {
            let [x, y, width, height] = control.rect;
            let y = y + if control.footer && self.expanded {
                DETAILS_HEIGHT
            } else {
                0
            };
            SetWindowPos(
                control.hwnd,
                null_mut(),
                self.s(x),
                self.s(y),
                self.s(width),
                self.s(height),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let font = self.fonts.get(Self::key(control.size, control.bold));
            SendMessageW(control.hwnd, WM_SETFONT, font as usize, 0);
            ShowWindow(
                control.hwnd,
                if control.details && !self.expanded {
                    SW_HIDE
                } else {
                    SW_SHOWNA
                },
            );
        }
        drop(old_fonts); // Child controls no longer reference the previous DPI's fonts.
        let p = self.palette();
        DwmSetWindowAttribute(
            self.hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&p.edge as *const u32).cast(),
            size_of::<u32>() as u32,
        );
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
        );
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn text(
        &mut self,
        dc: HDC,
        text: &str,
        rect: RECT,
        size: i32,
        color: u32,
        bold: bool,
        align: u32,
    ) {
        let old = SelectObject(dc, self.fonts.get(Self::key(size, bold)));
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        let mut rect = rect;
        let text = w(text);
        let flags = align | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX;
        snapshot::clip(self.hwnd, dc, &text, rect, flags);
        DrawTextW(dc, text.as_ptr(), (text.len() - 1) as i32, &mut rect, flags);
        SelectObject(dc, old);
    }
    unsafe fn background(&self, dc: HDC) {
        let p = self.palette();
        let mut client = RECT::default();
        GetClientRect(self.hwnd, &mut client);
        SetDCBrushColor(dc, p.bg);
        FillRect(dc, &client, GetStockObject(DC_BRUSH) as HBRUSH);
        SetDCBrushColor(dc, p.edge);
        FrameRect(dc, &client, GetStockObject(DC_BRUSH) as HBRUSH);
        chrome_round(dc, self.rect(32, 78, 56, 56), p.selection, self.s(14));
        chrome_round(dc, self.rect(389, 82, 90, 32), p.panel, self.s(10));
        if self.expanded {
            chrome_round(dc, self.rect(32, 404, 456, 262), p.panel, self.s(8));
        }
        smooth::icon(
            dc,
            self.rect(44, 90, 32, 32),
            smooth::FEATHER,
            p.accent,
            self.fonts.dpi as f32 / 96.0,
        );
        let line = self.rect(
            32,
            418 + if self.expanded { DETAILS_HEIGHT } else { 0 },
            456,
            1,
        );
        SetDCBrushColor(dc, p.edge);
        FillRect(dc, &line, GetStockObject(DC_BRUSH) as HBRUSH);
    }
    unsafe fn button(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let r = item.rcItem;
        let id = item.CtlID as usize;
        let p = self.palette();
        let pressed = item.itemState & ODS_SELECTED != 0;
        let focus = item.itemState & ODS_FOCUS != 0 && item.itemState & ODS_NOFOCUSRECT == 0;
        let mut hover = 0;
        GetWindowSubclass(item.hwndItem, Some(button_proc), 1, &mut hover);
        SetDCBrushColor(dc, p.bg);
        FillRect(dc, &r, GetStockObject(DC_BRUSH) as HBRUSH);
        let card = destination(id).is_some();
        if card || id == IDCANCEL as usize || pressed || hover != 0 {
            let color = if pressed || id == IDCANCEL as usize {
                p.selection
            } else if hover != 0 {
                p.code
            } else {
                p.panel
            };
            chrome_round(
                dc,
                r,
                if focus {
                    p.accent
                } else if card {
                    p.edge
                } else {
                    color
                },
                self.s(10),
            );
            let mut inner = r;
            InflateRect(&mut inner, -self.s(1).max(1), -self.s(1).max(1));
            chrome_round(dc, inner, color, self.s(9));
        }
        if card {
            let (title, note, accent) = if id == GITHUB_BUTTON {
                ("GitHub", tr("源码 · 反馈 · 版本更新"), p.accent)
            } else {
                (
                    tr("爱发电"),
                    tr("请作者喝杯咖啡"),
                    if self.theme.resolve(self.system_dark).is_dark() {
                        rgb(190, 169, 235)
                    } else {
                        rgb(124, 93, 175)
                    },
                )
            };
            let left = DT_LEFT;
            self.text(
                dc,
                title,
                self.rect(18, 12, 170, 27),
                17,
                accent,
                true,
                left,
            );
            self.text(
                dc,
                note,
                self.rect(18, 43, 192, 22),
                12,
                p.muted,
                false,
                left,
            );
            smooth::icon(
                dc,
                RECT {
                    left: r.right - self.s(32),
                    top: self.s(12),
                    right: r.right - self.s(8),
                    bottom: self.s(36),
                },
                smooth::EXTERNAL,
                accent,
                self.fonts.dpi as f32 / 96.0,
            );
        } else if id == IDCLOSE as usize {
            smooth::icon(dc, r, EXIT, p.muted, self.fonts.dpi as f32 / 96.0);
        } else if id == DETAILS {
            let label = if self.expanded {
                format!("−   {}", tr("收起快捷键与说明"))
            } else {
                format!("+   {}", tr("快捷键与阅读说明"))
            };
            self.text(
                dc,
                &label,
                self.rect(8, 0, 270, 28),
                12,
                p.muted,
                false,
                DT_LEFT,
            );
        } else {
            self.text(dc, tr("返回阅读"), r, 13, p.accent, true, DT_CENTER);
        }
        if focus && !card {
            let mut ring = r;
            InflateRect(&mut ring, -self.s(4), -self.s(4));
            DrawFocusRect(dc, &ring);
        }
    }
}

// Keep standard BUTTON semantics; only hover styling and the link cursor are custom.
unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    id: usize,
    hover: usize,
) -> isize {
    match msg {
        // BS_OWNERDRAW is not a native default-pushbutton style. Request Enter
        // explicitly so the dialog manager doesn't activate Return to reader instead.
        WM_GETDLGCODE if lp != 0 && (*(lp as *const MSG)).wParam == VK_RETURN as usize => {
            return DefSubclassProc(hwnd, msg, wp, lp) | DLGC_WANTMESSAGE as isize;
        }
        WM_KEYDOWN if wp == VK_RETURN as usize => {
            if lp & (1 << 30) == 0 {
                SendMessageW(hwnd, BM_CLICK, 0, 0);
            }
            return 0;
        }
        WM_MOUSEMOVE if hover == 0 => {
            SetWindowSubclass(hwnd, Some(button_proc), id, 1);
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_MOUSELEAVE => {
            SetWindowSubclass(hwnd, Some(button_proc), id, 0);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_SETCURSOR if destination(GetDlgCtrlID(hwnd) as usize).is_some() => {
            SetCursor(LoadCursorW(null_mut(), IDC_HAND));
            return 1;
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(button_proc), id);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    if msg == WM_INITDIALOG {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
        let cell = &*(lp as *const RefCell<About>);
        let mut about = cell.borrow_mut();
        about.hwnd = hwnd;
        SetWindowTextW(hwnd, w(tr("关于 FeatherMD")).as_ptr());
        if !about.create() {
            drop(about);
            EndDialog(hwnd, -1);
            return 1;
        }
        let round = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&round as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        about.layout(true);
        drop(about);
        SendMessageW(hwnd, DM_SETDEFID, IDCANCEL as usize, 0);
        SetFocus(GetDlgItem(hwnd, IDCANCEL));
        SendMessageW(
            hwnd,
            WM_CHANGEUISTATE,
            ((UISF_HIDEFOCUS as usize) << 16) | UIS_SET as usize,
            0,
        );
        return 0; // Focus was explicitly placed on the safe default action.
    }
    if msg == WM_NCCALCSIZE || msg == WM_NCPAINT {
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, 0);
        return 1;
    }
    if msg == WM_NCACTIVATE {
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, 1);
        return 1;
    }
    if msg == WM_CLOSE {
        EndDialog(hwnd, IDCANCEL as isize);
        return 1;
    }
    if msg == WM_COMMAND {
        let id = wp & 0xffff;
        if id == IDCANCEL as usize || id == IDCLOSE as usize {
            EndDialog(hwnd, id as isize);
            return 1;
        }
        if let Some(url) = destination(id) {
            // No About or App borrow may cross this nested native confirmation loop.
            open_link(hwnd, url);
            return 1;
        }
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<About>;
    if ptr.is_null() {
        return 0;
    }
    if msg == WM_APP + 42 && wp == 5 {
        // Diagnostics: text of the last capture that does not fit (labels.txt).
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, snapshot::report(hwnd));
        return 1;
    }
    if msg == WM_APP + 42 && wp == 4 {
        snapshot::clear_clips(hwnd);
        let captured = snapshot::capture(hwnd, |dc| {
            let about = (*ptr).borrow();
            about.background(dc);
            let children: Vec<HWND> = about
                .controls
                .iter()
                .filter(|c| !c.details || about.expanded)
                .map(|c| c.hwnd)
                .collect();
            drop(about); // Child WM_PRINT sends WM_CTLCOLOR/WM_DRAWITEM back to us.
            for child in children {
                let mut rect = RECT::default();
                GetWindowRect(child, &mut rect);
                let mut origin = POINT {
                    x: rect.left,
                    y: rect.top,
                };
                ScreenToClient(hwnd, &mut origin);
                let saved = SaveDC(dc);
                SetViewportOrgEx(dc, origin.x, origin.y, null_mut());
                SendMessageW(
                    child,
                    WM_PRINT,
                    dc as usize,
                    (PRF_CLIENT | PRF_ERASEBKGND) as isize,
                );
                RestoreDC(dc, saved);
            }
        });
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, captured as isize);
        return 1;
    }
    let Ok(mut about) = (*ptr).try_borrow_mut() else {
        return 0;
    };
    match msg {
        WM_COMMAND if wp & 0xffff == DETAILS => {
            about.expanded = !about.expanded;
            SetWindowTextW(
                GetDlgItem(hwnd, DETAILS as i32),
                w(if about.expanded {
                    tr("收起快捷键与说明")
                } else {
                    tr("快捷键与阅读说明")
                })
                .as_ptr(),
            );
            about.layout(false);
            1
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            about.background(dc);
            EndPaint(hwnd, &ps);
            1
        }
        WM_PRINTCLIENT => {
            about.background(wp as HDC);
            1
        }
        WM_ERASEBKGND => {
            SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, 1);
            1
        }
        WM_DRAWITEM => {
            about.button(&*(lp as *const DRAWITEMSTRUCT));
            1
        }
        WM_CTLCOLORSTATIC => {
            let p = about.palette();
            let id = GetDlgCtrlID(lp as HWND) as usize;
            let muted = about
                .controls
                .iter()
                .find(|c| c.id == id)
                .is_some_and(|c| c.muted);
            let bg = if id == 2003 || id >= 2010 {
                p.panel
            } else {
                p.bg
            };
            SetTextColor(wp as HDC, if muted { p.muted } else { p.fg });
            SetBkColor(wp as HDC, bg);
            SetDCBrushColor(wp as HDC, bg);
            GetStockObject(DC_BRUSH) as isize
        }
        WM_DPICHANGED | WM_SETTINGCHANGE | WM_THEMECHANGED => {
            if msg == WM_DPICHANGED && lp != 0 {
                let suggested = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            about.system_dark = system_uses_dark_theme();
            about.layout(false);
            1
        }
        WM_NCHITTEST => {
            let mut point = POINT {
                x: lp as i16 as i32,
                y: (lp >> 16) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            if point.y < about.s(52) {
                SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, HTCAPTION as isize);
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

// Dialog templates require DWORD alignment and three trailing empty UTF-16 fields.
#[repr(C, align(4))]
struct EmptyDialog {
    template: DLGTEMPLATE,
    menu: u16,
    class: u16,
    title: u16,
}
pub(super) unsafe fn show(owner: HWND, theme: Theme) {
    let state = RefCell::new(About {
        hwnd: null_mut(),
        owner,
        theme,
        system_dark: system_uses_dark_theme(),
        fonts: Fonts::new(GetDpiForWindow(owner).max(96)),
        expanded: false,
        controls: Vec::new(),
    });
    let template = EmptyDialog {
        template: DLGTEMPLATE {
            style: WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
            dwExtendedStyle: WS_EX_DLGMODALFRAME,
            cdit: 0,
            x: 0,
            y: 0,
            cx: 320,
            cy: 280,
        },
        menu: 0,
        class: 0,
        title: 0,
    };
    if DialogBoxIndirectParamW(
        GetModuleHandleW(null()),
        &template.template,
        owner,
        Some(dialog_proc),
        &state as *const RefCell<About> as isize,
    ) == -1
    {
        let text = trf(
            "FeatherMD 羽读 {}\n\nGitHub：{}\n爱发电：{}\n\n关于窗口无法创建，请手动访问以上链接。",
            &[&env!("CARGO_PKG_VERSION"), &GITHUB, &SUPPORT],
        );
        MessageBoxW(
            owner,
            w(&text).as_ptr(),
            w(tr("关于 FeatherMD")).as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_about_actions_have_destinations() {
        assert_eq!(destination(GITHUB_BUTTON), Some(GITHUB));
        assert_eq!(destination(SUPPORT_BUTTON), Some(SUPPORT));
        for id in [
            0,
            IDOK as usize,
            IDCANCEL as usize,
            IDCLOSE as usize,
            DETAILS,
            usize::MAX,
        ] {
            assert_eq!(destination(id), None);
        }
        // Every README translation links to the same project and support pages.
        for readme in [
            include_str!("../../README.md"),
            include_str!("../../README.zh-TW.md"),
            include_str!("../../README.en.md"),
            include_str!("../../README.ja.md"),
        ] {
            for url in [GITHUB, SUPPORT] {
                assert!(url.starts_with("https://"));
                assert!(readme.contains(url));
            }
        }
        assert_eq!(std::mem::offset_of!(EmptyDialog, menu), 18);
    }
}
