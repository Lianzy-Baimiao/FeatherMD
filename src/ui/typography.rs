//! Native font preferences. Draft state never escapes until the user chooses Save.
use super::*;
use windows_sys::Win32::UI::Controls::{
    DRAWITEMSTRUCT, EM_SETLIMITTEXT, MEASUREITEMSTRUCT, ODS_FOCUS, ODS_SELECTED,
};
const FAMILY: usize = 1101;
const SIZE: usize = 1102;
const DEFAULTS: usize = 1103;
const ERROR: usize = 1104;
const WIDTH: i32 = 536;
const HEIGHT: i32 = 470;

unsafe extern "system" fn enum_font(
    font: *const LOGFONTW,
    _: *const TEXTMETRICW,
    _: u32,
    data: isize,
) -> i32 {
    let face = &(*font).lfFaceName;
    let name =
        String::from_utf16_lossy(&face[..face.iter().position(|v| *v == 0).unwrap_or(face.len())]);
    if Settings::validate_font_family(&name).is_some() {
        (*(data as *mut std::collections::BTreeSet<String>)).insert(name);
    }
    1
}
struct Typography {
    hwnd: HWND,
    owner: HWND,
    theme: Theme,
    ui: Fonts,
    preview: Fonts,
    families: Vec<String>,
    family: String,
    size: i32,
    valid: bool,
    controls: Vec<(HWND, [i32; 4])>,
    result: Option<(String, i32)>,
}
impl Typography {
    fn palette(&self) -> Palette {
        Palette::new(self.theme.resolve(system_uses_dark_theme()))
    }
    fn rect(&self, x: i32, y: i32, width: i32, height: i32) -> RECT {
        RECT {
            left: self.ui.scale(x),
            top: self.ui.scale(y),
            right: self.ui.scale(x + width),
            bottom: self.ui.scale(y + height),
        }
    }
    unsafe fn control(
        &mut self,
        class: &str,
        text: &str,
        id: usize,
        style: u32,
        rect: [i32; 4],
    ) -> HWND {
        let hwnd = CreateWindowExW(
            0,
            w(class).as_ptr(),
            w(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            self.hwnd,
            id as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        if class == "BUTTON" {
            SetWindowSubclass(hwnd, Some(button_proc), 1, 0);
        }
        self.controls.push((hwnd, rect));
        hwnd
    }
    unsafe fn init(&mut self) {
        SetWindowTextW(self.hwnd, w("字体与默认字号").as_ptr());
        let dc = GetDC(self.hwnd);
        let mut names = std::collections::BTreeSet::new();
        let mut lf: LOGFONTW = zeroed();
        lf.lfCharSet = DEFAULT_CHARSET;
        EnumFontFamiliesExW(dc, &lf, Some(enum_font), &mut names as *mut _ as isize, 0);
        ReleaseDC(self.hwnd, dc);
        // Keep a previously saved choice visible even if a font was uninstalled.
        names.insert(self.family.clone());
        names.insert(Settings::default().font_family);
        self.families = names.into_iter().collect();
        self.control("STATIC", "阅读字体", 1200, 0, [28, 22, 460, 30]);
        self.control(
            "STATIC",
            "让每一页，都更合你的阅读习惯。",
            1201,
            0,
            [28, 59, 480, 24],
        );
        self.control("STATIC", "字体(&F)", 1202, 0, [28, 105, 320, 22]);
        let combo = self.control(
            "COMBOBOX",
            "",
            FAMILY,
            WS_TABSTOP
                | WS_VSCROLL
                | CBS_DROPDOWNLIST as u32
                | CBS_OWNERDRAWFIXED as u32
                | CBS_HASSTRINGS as u32,
            [28, 134, 340, 260],
        );
        SetWindowSubclass(combo, Some(combo_proc), 1, 0);
        for family in &self.families {
            SendMessageW(combo, CB_ADDSTRING, 0, w(family).as_ptr() as isize);
        }
        self.control("STATIC", "默认字号(&S)", 1203, 0, [388, 105, 120, 22]);
        let edit = self.control(
            "EDIT",
            "",
            SIZE,
            WS_TABSTOP | WS_BORDER | ES_NUMBER as u32 | ES_CENTER as u32 | ES_AUTOHSCROLL as u32,
            [388, 134, 76, 30],
        );
        SendMessageW(edit, EM_SETLIMITTEXT, 3, 0);
        self.control("STATIC", "px", 1204, 0, [472, 139, 30, 24]);
        self.control("STATIC", "", ERROR, 0, [28, 177, 480, 22]);
        self.control(
            "STATIC",
            "仅正文与标题使用此字体，代码仍保持等宽。",
            1206,
            0,
            [28, 352, 480, 22],
        );
        self.control(
            "STATIC",
            "字号范围 12–36 px · 保存后自动记住",
            1207,
            0,
            [28, 378, 480, 22],
        );
        self.control(
            "BUTTON",
            "恢复默认",
            DEFAULTS,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            [28, 420, 106, 32],
        );
        self.control(
            "BUTTON",
            "取消",
            IDCANCEL as usize,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            [318, 420, 86, 32],
        );
        self.control(
            "BUTTON",
            "保存",
            IDOK as usize,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            [416, 420, 92, 32],
        );
        self.set_draft();
        self.layout(true);
        self.update();
        SendMessageW(self.hwnd, DM_SETDEFID, IDOK as usize, 0);
    }
    unsafe fn set_draft(&self) {
        let selected = self
            .families
            .iter()
            .position(|f| f == &self.family)
            .unwrap_or(0);
        SendDlgItemMessageW(self.hwnd, FAMILY as i32, CB_SETCURSEL, selected, 0);
        SetDlgItemTextW(self.hwnd, SIZE as i32, w(&self.size.to_string()).as_ptr());
    }
    unsafe fn layout(&mut self, center: bool) {
        let old_ui =
            std::mem::replace(&mut self.ui, Fonts::new(GetDpiForWindow(self.hwnd).max(96)));
        self.preview = Fonts::with_family(self.ui.dpi, &self.family);
        let dark = (self.theme.resolve(system_uses_dark_theme()) == Theme::Dark) as i32;
        DwmSetWindowAttribute(
            self.hwnd,
            20,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        );
        let mut bounds = self.rect(0, 0, WIDTH, HEIGHT);
        AdjustWindowRectExForDpi(
            &mut bounds,
            WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
            0,
            WS_EX_DLGMODALFRAME,
            self.ui.dpi,
        );
        let mut owner = RECT::default();
        GetWindowRect(self.owner, &mut owner);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        SetWindowPos(
            self.hwnd,
            null_mut(),
            owner.left + (owner.right - owner.left - width) / 2,
            owner.top + (owner.bottom - owner.top - height) / 2,
            width,
            height,
            SWP_NOZORDER | if center { 0 } else { SWP_NOMOVE },
        );
        for &(hwnd, [x, y, w, h]) in &self.controls {
            let id = GetDlgCtrlID(hwnd);
            let font = self.ui.get(FontKey {
                size: if id == 1200 { 24 } else { 14 },
                bold: id == 1200,
                italic: false,
                mono: false,
                strike: false,
            });
            SendMessageW(hwnd, WM_SETFONT, font as usize, 1);
            SetWindowPos(
                hwnd,
                null_mut(),
                self.ui.scale(x),
                self.ui.scale(y),
                self.ui.scale(w),
                self.ui.scale(h),
                SWP_NOZORDER,
            );
        }
        SendDlgItemMessageW(
            self.hwnd,
            FAMILY as i32,
            CB_SETITEMHEIGHT,
            0,
            self.ui.scale(26) as isize,
        );
        SendDlgItemMessageW(
            self.hwnd,
            FAMILY as i32,
            CB_SETITEMHEIGHT,
            usize::MAX,
            self.ui.scale(26) as isize,
        );
        // Child controls no longer refer to any handles in the retired cache.
        drop(old_ui);
        InvalidateRect(self.hwnd, null(), 1);
    }
    unsafe fn update(&mut self) {
        let index = SendDlgItemMessageW(self.hwnd, FAMILY as i32, CB_GETCURSEL, 0, 0) as usize;
        let mut text = [0u16; 16];
        GetDlgItemTextW(self.hwnd, SIZE as i32, text.as_mut_ptr(), text.len() as i32);
        let text = String::from_utf16_lossy(
            &text[..text.iter().position(|c| *c == 0).unwrap_or(text.len())],
        );
        let size = text.parse::<i32>().ok().filter(|v| (12..=36).contains(v));
        self.valid = size.is_some() && index < self.families.len();
        SetDlgItemTextW(
            self.hwnd,
            ERROR as i32,
            w(if self.valid {
                "实时预览"
            } else {
                "请输入 12–36 之间的字号"
            })
            .as_ptr(),
        );
        EnableWindow(GetDlgItem(self.hwnd, IDOK), self.valid as i32);
        if let Some(size) = size
            && let Some(family) = self.families.get(index)
        {
            self.size = size;
            if &self.family != family {
                self.family = family.clone();
                self.preview = Fonts::with_family(self.ui.dpi, family);
            }
        }
        InvalidateRect(self.hwnd, null(), 0);
    }
    unsafe fn background(&mut self, dc: HDC) {
        let saved = SaveDC(dc);
        let p = self.palette();
        SetDCBrushColor(dc, p.bg);
        let mut r = RECT::default();
        GetClientRect(self.hwnd, &mut r);
        FillRect(dc, &r, GetStockObject(DC_BRUSH));
        chrome::chrome_round(dc, self.rect(28, 206, 480, 128), p.panel, self.ui.scale(14));
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, p.fg);
        for (text, y, size, bold) in [
            ("字里行间，慢慢读。", 221, self.size, true),
            ("FeatherMD · A little room to read.", 276, self.size, false),
        ] {
            let font = self.preview.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
            });
            SelectObject(dc, font);
            let mut rect = self.rect(48, y, 440, 48);
            let text = w(text);
            DrawTextW(
                dc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        RestoreDC(dc, saved);
    }
    unsafe fn combo(&mut self, hwnd: HWND, dc: HDC) {
        let saved = SaveDC(dc);
        let p = self.palette();
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        SetDCBrushColor(dc, p.bg);
        FillRect(dc, &r, GetStockObject(DC_BRUSH));
        chrome::chrome_round(
            dc,
            r,
            if GetFocus() == hwnd { p.accent } else { p.edge },
            self.ui.scale(7),
        );
        InflateRect(&mut r, -1, -1);
        chrome::chrome_round(dc, r, p.panel, self.ui.scale(6));
        let arrow = RECT {
            left: r.right - self.ui.scale(32),
            top: r.top,
            right: r.right - self.ui.scale(8),
            bottom: r.bottom,
        };
        smooth::icon(dc, arrow, THEME_PICKER, p.muted, self.ui.dpi as f32 / 96.0);
        let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0) as usize;
        if let Some(family) = self.families.get(index) {
            SelectObject(
                dc,
                self.ui.get(FontKey {
                    size: 14,
                    bold: false,
                    italic: false,
                    mono: false,
                    strike: false,
                }),
            );
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, p.fg);
            r.left += self.ui.scale(10);
            r.right -= self.ui.scale(36);
            DrawTextW(
                dc,
                w(family).as_ptr(),
                -1,
                &mut r,
                DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        RestoreDC(dc, saved);
    }
    unsafe fn font_item(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        SetDCBrushColor(
            dc,
            if item.itemState & ODS_SELECTED != 0 {
                p.selection
            } else {
                p.panel
            },
        );
        FillRect(dc, &item.rcItem, GetStockObject(DC_BRUSH));
        if let Some(family) = self.families.get(item.itemID as usize) {
            SelectObject(
                dc,
                self.ui.get(FontKey {
                    size: 14,
                    bold: false,
                    italic: false,
                    mono: false,
                    strike: false,
                }),
            );
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, p.fg);
            let mut r = item.rcItem;
            r.left += self.ui.scale(10);
            DrawTextW(
                dc,
                w(family).as_ptr(),
                -1,
                &mut r,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        RestoreDC(dc, saved);
    }
    unsafe fn button(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        let primary = item.CtlID == IDOK as u32;
        SetDCBrushColor(dc, p.bg);
        FillRect(dc, &item.rcItem, GetStockObject(DC_BRUSH));
        chrome::chrome_round(
            dc,
            item.rcItem,
            if item.itemState & ODS_SELECTED != 0 {
                p.selection
            } else if primary && self.valid {
                p.accent
            } else {
                p.panel
            },
            self.ui.scale(8),
        );
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(
            dc,
            if primary && self.valid {
                if self.theme.resolve(system_uses_dark_theme()) == Theme::Dark {
                    p.bg
                } else {
                    rgb(255, 255, 255)
                }
            } else {
                p.fg
            },
        );
        SelectObject(
            dc,
            self.ui.get(FontKey {
                size: 14,
                bold: primary,
                italic: false,
                mono: false,
                strike: false,
            }),
        );
        let mut text = [0u16; 64];
        GetWindowTextW(item.hwndItem, text.as_mut_ptr(), 64);
        let mut r = item.rcItem;
        DrawTextW(
            dc,
            text.as_ptr(),
            -1,
            &mut r,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        if item.itemState & ODS_FOCUS != 0 {
            InflateRect(&mut r, -self.ui.scale(4), -self.ui.scale(4));
            DrawFocusRect(dc, &r);
        }
        RestoreDC(dc, saved);
    }
}
unsafe extern "system" fn combo_proc(
    hwnd: HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    id: usize,
    _: usize,
) -> isize {
    if msg == WM_PAINT || msg == WM_PRINTCLIENT {
        let ptr = GetWindowLongPtrW(GetParent(hwnd), GWLP_USERDATA) as *const RefCell<Typography>;
        if !ptr.is_null()
            && let Ok(mut state) = (*ptr).try_borrow_mut()
        {
            if msg == WM_PAINT {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                state.combo(hwnd, dc);
                EndPaint(hwnd, &ps);
            } else {
                state.combo(hwnd, wp as HDC);
            }
            return 0;
        }
    }
    if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) {
        InvalidateRect(hwnd, null(), 0);
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(combo_proc), id);
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    id: usize,
    _: usize,
) -> isize {
    if msg == WM_GETDLGCODE && wp == VK_RETURN as usize {
        return DefSubclassProc(hwnd, msg, wp, lp) | DLGC_WANTMESSAGE as isize;
    }
    if msg == WM_KEYDOWN && wp == VK_RETURN as usize {
        if lp & (1 << 30) == 0 {
            SendMessageW(hwnd, BM_CLICK, 0, 0);
        }
        return 0;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(button_proc), id);
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
unsafe extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    if msg == WM_INITDIALOG {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
        let mut state = (*(lp as *const RefCell<Typography>)).borrow_mut();
        state.hwnd = hwnd;
        state.init();
        return 1;
    }
    if msg == WM_CLOSE || (msg == WM_COMMAND && wp & 0xffff == IDCANCEL as usize) {
        EndDialog(hwnd, IDCANCEL as isize);
        return 1;
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<Typography>;
    if ptr.is_null() {
        return 0;
    }
    if msg == WM_APP + 42 && wp == 4 {
        let captured = snapshot::capture(hwnd, |dc| {
            let mut state = (*ptr).borrow_mut();
            state.background(dc);
            let children: Vec<HWND> = state.controls.iter().map(|c| c.0).collect();
            drop(state);
            for child in children {
                let mut r = RECT::default();
                GetWindowRect(child, &mut r);
                let mut p = POINT {
                    x: r.left,
                    y: r.top,
                };
                ScreenToClient(hwnd, &mut p);
                let saved = SaveDC(dc);
                SetViewportOrgEx(dc, p.x, p.y, null_mut());
                SendMessageW(
                    child,
                    if GetDlgCtrlID(child) == FAMILY as i32 {
                        WM_PRINTCLIENT
                    } else {
                        WM_PRINT
                    },
                    dc as usize,
                    (PRF_CLIENT | PRF_ERASEBKGND) as isize,
                );
                RestoreDC(dc, saved);
            }
        });
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, captured as isize);
        return 1;
    }
    let Ok(mut state) = (*ptr).try_borrow_mut() else {
        return 0;
    };
    match msg {
        WM_COMMAND => {
            let id = wp & 0xffff;
            if id == IDOK as usize {
                state.update();
                if state.valid {
                    state.result = Some((state.family.clone(), state.size));
                    drop(state);
                    EndDialog(hwnd, IDOK as isize);
                }
            } else if id == DEFAULTS {
                let defaults = Settings::default();
                state.family = defaults.font_family;
                state.size = defaults.font_size;
                state.preview = Fonts::with_family(state.ui.dpi, &state.family);
                state.set_draft();
                state.update();
            } else if (id == FAMILY && wp >> 16 == CBN_SELCHANGE as usize)
                || (id == SIZE && wp >> 16 == EN_CHANGE as usize)
            {
                state.update();
            }
            1
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            state.background(dc);
            EndPaint(hwnd, &ps);
            1
        }
        WM_PRINTCLIENT => {
            state.background(wp as HDC);
            1
        }
        WM_ERASEBKGND => 1,
        WM_MEASUREITEM => {
            let item = &mut *(lp as *mut MEASUREITEMSTRUCT);
            item.itemHeight = state.ui.scale(26) as u32;
            1
        }
        WM_DRAWITEM => {
            let item = &*(lp as *const DRAWITEMSTRUCT);
            if item.CtlID == FAMILY as u32 {
                state.font_item(item);
            } else {
                state.button(item);
            }
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let p = state.palette();
            let id = GetDlgCtrlID(lp as HWND);
            let bg = if id == FAMILY as i32 || id == SIZE as i32 {
                p.panel
            } else {
                p.bg
            };
            SetTextColor(
                wp as HDC,
                if matches!(id, 1201 | 1204 | 1206 | 1207) || id == ERROR as i32 {
                    p.muted
                } else {
                    p.fg
                },
            );
            SetBkColor(wp as HDC, bg);
            SetDCBrushColor(wp as HDC, bg);
            GetStockObject(DC_BRUSH) as isize
        }
        WM_DPICHANGED | WM_THEMECHANGED | WM_SETTINGCHANGE => {
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
            state.layout(false);
            1
        }
        _ => 0,
    }
}
#[repr(C, align(4))]
struct EmptyDialog {
    template: DLGTEMPLATE,
    menu: u16,
    class: u16,
    title: u16,
}
pub(super) unsafe fn choose(owner: HWND, settings: &Settings) -> Option<(String, i32)> {
    let dpi = GetDpiForWindow(owner).max(96);
    let state = RefCell::new(Typography {
        hwnd: null_mut(),
        owner,
        theme: settings.theme,
        ui: Fonts::new(dpi),
        preview: Fonts::with_family(dpi, &settings.font_family),
        families: Vec::new(),
        family: settings.font_family.clone(),
        size: settings.font_size,
        valid: true,
        controls: Vec::new(),
        result: None,
    });
    let template = EmptyDialog {
        template: DLGTEMPLATE {
            style: WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
            dwExtendedStyle: WS_EX_DLGMODALFRAME,
            cdit: 0,
            x: 0,
            y: 0,
            cx: 340,
            cy: 320,
        },
        menu: 0,
        class: 0,
        title: 0,
    };
    let result = DialogBoxIndirectParamW(
        GetModuleHandleW(null()),
        &template.template,
        owner,
        Some(dialog_proc),
        &state as *const RefCell<Typography> as isize,
    );
    if result == -1 {
        MessageBoxW(
            owner,
            w("无法创建字体设置窗口。").as_ptr(),
            w("FeatherMD").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
    state.into_inner().result
}
