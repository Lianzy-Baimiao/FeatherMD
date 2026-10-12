//! Settings: appearance (skins, window opacity), reading font and external editor.
//! Draft state never escapes until Save; skin/opacity drafts preview live on the
//! reader through PREVIEW_APPEARANCE and are reverted by the owner on Cancel.
use super::*;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::UI::Controls::{
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, CDRF_SKIPDEFAULT,
    DRAWITEMSTRUCT, EM_SETLIMITTEXT, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    MEASUREITEMSTRUCT, NM_CUSTOMDRAW, NMCUSTOMDRAW, NMHDR, ODS_FOCUS, ODS_NOACCEL, ODS_NOFOCUSRECT,
    ODS_SELECTED, TBCD_CHANNEL, TBCD_THUMB, TBM_GETTHUMBRECT, TBM_SETLINESIZE, TBM_SETPAGESIZE,
    TBM_SETPOS, TBM_SETRANGE, TBS_NOTICKS,
};
const TBM_GETPOS: u32 = WM_USER;
const SS_RIGHT: u32 = 0x0002;
const SS_NOPREFIX: u32 = 0x0080;
const SS_PATHELLIPSIS: u32 = 0x8000;

// Reading page ids are unchanged from the former font dialog.
const FAMILY: usize = 1101;
const SIZE: usize = 1102;
const DEFAULTS: usize = 1103;
const ERROR: usize = 1104;
const NAV: [usize; 3] = [1110, 1111, 1112];
const CARD: usize = 1120;
const OPACITY: usize = 1140;
const OPACITY_VALUE: usize = 1141;
const EDITOR_LIST: usize = 1150;
const BROWSE: usize = 1151;
const EDITOR_PATH: usize = 1152;
const WIDTH: i32 = 720;
const HEIGHT: i32 = 552;
const NAV_WIDTH: i32 = 184;
/// Content column origin.
const X0: i32 = 212;
const PAGES: [(&str, usize); 3] = [
    ("外观(&A)", smooth::PALETTE),
    ("阅读(&R)", smooth::BOOK),
    ("编辑(&E)", smooth::PENCIL),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Text,
    Muted,
    Accent,
    Nav,
    NavMuted,
    Info,
    InfoMuted,
    Field,
}
/// Font size, weight and colour role of a control.
#[derive(Clone, Copy)]
struct Look(i32, bool, Tone);
const TITLE: Look = Look(22, true, Tone::Text);
const BODY: Look = Look(14, false, Tone::Text);
const NOTE: Look = Look(13, false, Tone::Muted);

struct Control {
    hwnd: HWND,
    /// None: visible on every page.
    page: Option<usize>,
    rect: [i32; 4],
    look: Look,
}
struct Prefs {
    hwnd: HWND,
    owner: HWND,
    page: usize,
    draft: Settings,
    ui: Fonts,
    preview: Fonts,
    families: Vec<String>,
    editors: Vec<(String, Editor)>,
    valid: bool,
    controls: Vec<Control>,
    result: Option<Settings>,
}

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

fn opacity_position(opacity: u8) -> isize {
    isize::from(Settings::clamp_opacity(i64::from(opacity)) / 5)
}

impl Prefs {
    fn palette(&self) -> Palette {
        Palette::new(self.draft.theme.resolve(system_uses_dark_theme()))
    }
    fn s(&self, value: i32) -> i32 {
        self.ui.scale(value)
    }
    fn rect(&self, x: i32, y: i32, width: i32, height: i32) -> RECT {
        RECT {
            left: self.s(x),
            top: self.s(y),
            right: self.s(x + width),
            bottom: self.s(y + height),
        }
    }
    unsafe fn add(
        &mut self,
        class: &str,
        text: &str,
        id: usize,
        style: u32,
        (page, rect): (Option<usize>, [i32; 4]),
        look: Look,
    ) -> HWND {
        let hwnd = CreateWindowExW(
            0,
            w(class).as_ptr(),
            w(text).as_ptr(),
            WS_CHILD | style,
            0,
            0,
            0,
            0,
            self.hwnd,
            id as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        self.controls.push(Control {
            hwnd,
            page,
            rect,
            look,
        });
        hwnd
    }
    unsafe fn label(
        &mut self,
        text: &str,
        id: usize,
        place: (Option<usize>, [i32; 4]),
        look: Look,
    ) {
        // Labels with "&" carry the mnemonic for the control created after them.
        let prefix = if text.contains('&') { 0 } else { SS_NOPREFIX };
        self.add("STATIC", text, id, prefix, place, look);
    }
    unsafe fn button(&mut self, text: &str, id: usize, place: (Option<usize>, [i32; 4])) -> HWND {
        let hwnd = self.add(
            "BUTTON",
            text,
            id,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            place,
            BODY,
        );
        SetWindowSubclass(hwnd, Some(button_proc), 1, 0);
        hwnd
    }
    unsafe fn init(&mut self) {
        SetWindowTextW(self.hwnd, w("设置").as_ptr());
        let dc = GetDC(self.hwnd);
        let mut names = std::collections::BTreeSet::new();
        let mut lf: LOGFONTW = zeroed();
        lf.lfCharSet = DEFAULT_CHARSET;
        EnumFontFamiliesExW(dc, &lf, Some(enum_font), &mut names as *mut _ as isize, 0);
        ReleaseDC(self.hwnd, dc);
        // Keep a previously saved choice visible even if a font was uninstalled.
        names.insert(self.draft.font_family.clone());
        names.insert(Settings::default().font_family);
        self.families = names.into_iter().collect();
        // Fixed metadata probes only; see editor::detect.
        self.editors = vec![
            ("系统默认".into(), Editor::System),
            ("记事本".into(), Editor::Notepad),
        ];
        for (name, path) in crate::editor::detect() {
            self.editors.push((name, Editor::Program(path)));
        }
        if !self.editors.iter().any(|(_, e)| *e == self.draft.editor) {
            let editor = self.draft.editor.clone();
            self.editors.push((crate::editor::name(&editor), editor));
        }

        // Navigation column.
        self.label(
            "设置",
            1200,
            (None, [24, 22, 140, 30]),
            Look(18, true, Tone::Nav),
        );
        for (i, (name, _)) in PAGES.into_iter().enumerate() {
            self.button(name, NAV[i], (None, [14, 72 + i as i32 * 42, 156, 36]));
        }
        self.label(
            "FeatherMD · 本地保存",
            1201,
            (None, [24, HEIGHT - 40, 150, 20]),
            Look(11, false, Tone::NavMuted),
        );

        // Appearance.
        let page = Some(0);
        self.label("皮肤", 1210, (page, [X0, 24, 300, 30]), TITLE);
        self.label(
            "书本、古籍与常用配色，选择后立即预览。",
            1211,
            (page, [X0, 58, 470, 22]),
            NOTE,
        );
        for (i, theme) in Theme::ALL.into_iter().enumerate() {
            let (column, row) = (i as i32 % 4, i as i32 / 4);
            let card = self.add(
                "BUTTON",
                skin_name(theme),
                CARD + i,
                BS_OWNERDRAW as u32
                    | if theme == self.draft.theme {
                        WS_TABSTOP
                    } else {
                        0
                    },
                (page, [X0 + column * 121, 92 + row * 96, 109, 86]),
                BODY,
            );
            SetWindowSubclass(card, Some(button_proc), 1, 0);
        }
        self.label("窗口不透明度", 1212, (page, [X0, 392, 200, 22]), BODY);
        self.add(
            "STATIC",
            "",
            OPACITY_VALUE,
            SS_NOPREFIX | SS_RIGHT,
            (page, [X0 + 300, 392, 176, 22]),
            Look(14, true, Tone::Accent),
        );
        let slider = self.add(
            "msctls_trackbar32",
            "窗口不透明度",
            OPACITY,
            WS_TABSTOP | TBS_NOTICKS,
            (page, [X0 - 6, 418, 488, 32]),
            BODY,
        );
        let (low, high) = (opacity_position(Settings::MIN_OPACITY), 20);
        SendMessageW(slider, TBM_SETRANGE, 1, low | (high << 16));
        SendMessageW(slider, TBM_SETLINESIZE, 0, 1);
        SendMessageW(slider, TBM_SETPAGESIZE, 0, 2);
        self.label(
            "30%–100%，数值越低越透明；只作用于阅读窗口，设置窗口保持不透明。",
            1213,
            (page, [X0, 452, 480, 20]),
            Look(12, false, Tone::Muted),
        );

        // Reading (ids kept from the former font dialog).
        let page = Some(1);
        self.label("阅读字体", 1220, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            "让每一页，都更合你的阅读习惯。",
            1221,
            (page, [X0, 58, 470, 22]),
            NOTE,
        );
        self.label("字体(&F)", 1202, (page, [X0, 98, 320, 22]), BODY);
        let combo = self.add(
            "COMBOBOX",
            "",
            FAMILY,
            WS_TABSTOP
                | WS_VSCROLL
                | CBS_DROPDOWNLIST as u32
                | CBS_OWNERDRAWFIXED as u32
                | CBS_HASSTRINGS as u32,
            (page, [X0, 126, 340, 260]),
            Look(14, false, Tone::Field),
        );
        SetWindowSubclass(combo, Some(combo_proc), 1, 0);
        for family in &self.families {
            SendMessageW(combo, CB_ADDSTRING, 0, w(family).as_ptr() as isize);
        }
        self.label("默认字号(&S)", 1203, (page, [X0 + 360, 98, 120, 22]), BODY);
        let edit = self.add(
            "EDIT",
            "",
            SIZE,
            WS_TABSTOP | WS_BORDER | ES_NUMBER as u32 | ES_CENTER as u32 | ES_AUTOHSCROLL as u32,
            (page, [X0 + 360, 126, 76, 30]),
            Look(14, false, Tone::Field),
        );
        SendMessageW(edit, EM_SETLIMITTEXT, 3, 0);
        self.label("px", 1204, (page, [X0 + 444, 131, 30, 24]), NOTE);
        self.label("", ERROR, (page, [X0, 168, 480, 22]), NOTE);
        self.label(
            "仅正文与标题使用此字体，代码仍保持等宽。",
            1206,
            (page, [X0, 352, 480, 22]),
            NOTE,
        );
        self.label(
            "字号范围 12–36 px · 保存后自动记住",
            1207,
            (page, [X0, 378, 480, 22]),
            NOTE,
        );

        // External editor.
        let page = Some(2);
        self.label("默认编辑应用", 1230, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            "FeatherMD 只读；编辑时把当前文档交给你选择的应用。",
            1231,
            (page, [X0, 58, 480, 22]),
            NOTE,
        );
        self.label("编辑应用(&D)", 1232, (page, [X0, 98, 320, 22]), BODY);
        let editors = self.add(
            "COMBOBOX",
            "",
            EDITOR_LIST,
            WS_TABSTOP
                | WS_VSCROLL
                | CBS_DROPDOWNLIST as u32
                | CBS_OWNERDRAWFIXED as u32
                | CBS_HASSTRINGS as u32,
            (page, [X0, 126, 360, 260]),
            Look(14, false, Tone::Field),
        );
        SetWindowSubclass(editors, Some(combo_proc), 1, 0);
        for (name, _) in &self.editors {
            SendMessageW(editors, CB_ADDSTRING, 0, w(name).as_ptr() as isize);
        }
        self.button("浏览(&B)…", BROWSE, (page, [X0 + 372, 126, 108, 30]));
        self.add(
            "STATIC",
            "",
            EDITOR_PATH,
            SS_NOPREFIX | SS_PATHELLIPSIS,
            (page, [X0, 164, 480, 40]),
            Look(12, false, Tone::Muted),
        );
        self.label(
            "使用方式",
            1233,
            (page, [X0 + 20, 238, 440, 24]),
            Look(14, true, Tone::Info),
        );
        for (i, text) in [
            "正文右键 → 编辑，或按 Ctrl+E，用所选应用打开当前文档。",
            "保存后回到 FeatherMD 按 F5 重新载入，阅读位置保持不变。",
            "不会修改 Windows 的默认打开方式或文件关联。",
        ]
        .into_iter()
        .enumerate()
        {
            self.label(
                text,
                1234 + i,
                (page, [X0 + 20, 270 + i as i32 * 28, 440, 22]),
                Look(13, false, Tone::InfoMuted),
            );
        }

        // Shared actions.
        self.button("恢复默认", DEFAULTS, (None, [X0, 500, 106, 32]));
        self.button(
            "取消",
            IDCANCEL as usize,
            (None, [WIDTH - 220, 500, 86, 32]),
        );
        self.button("保存", IDOK as usize, (None, [WIDTH - 122, 500, 94, 32]));
        self.set_draft();
        self.layout(true);
        self.update();
        self.show_editor();
        SendMessageW(self.hwnd, DM_SETDEFID, IDOK as usize, 0);
    }
    unsafe fn layout(&mut self, center: bool) {
        let old_ui =
            std::mem::replace(&mut self.ui, Fonts::new(GetDpiForWindow(self.hwnd).max(96)));
        self.preview = Fonts::with_family(self.ui.dpi, &self.draft.font_family);
        self.caption();
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
        for control in &self.controls {
            let [x, y, w, h] = control.rect;
            let Look(size, bold, _) = control.look;
            let font = self.ui.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
            });
            SendMessageW(control.hwnd, WM_SETFONT, font as usize, 1);
            SetWindowPos(
                control.hwnd,
                null_mut(),
                self.ui.scale(x),
                self.ui.scale(y),
                self.ui.scale(w),
                self.ui.scale(h),
                SWP_NOZORDER,
            );
            ShowWindow(
                control.hwnd,
                if control.page.is_none_or(|page| page == self.page) {
                    SW_SHOWNA
                } else {
                    SW_HIDE
                },
            );
        }
        for id in [FAMILY, EDITOR_LIST] {
            for item in [0, usize::MAX] {
                SendDlgItemMessageW(
                    self.hwnd,
                    id as i32,
                    CB_SETITEMHEIGHT,
                    item,
                    self.ui.scale(26) as isize,
                );
            }
        }
        // Child controls no longer refer to any handles in the retired cache.
        drop(old_ui);
        InvalidateRect(self.hwnd, null(), 1);
    }
    /// The native caption follows the draft skin (Windows 11 honours caption colours).
    unsafe fn caption(&self) {
        let p = self.palette();
        let dark = self.draft.theme.resolve(system_uses_dark_theme()).is_dark() as i32;
        DwmSetWindowAttribute(
            self.hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        );
        for (attribute, color) in [
            (DWMWA_CAPTION_COLOR, p.panel),
            (DWMWA_TEXT_COLOR, p.fg),
            (DWMWA_BORDER_COLOR, p.edge),
        ] {
            DwmSetWindowAttribute(
                self.hwnd,
                attribute as u32,
                (&color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
        }
    }
    unsafe fn show_page(&mut self, page: usize) {
        if page == self.page {
            return;
        }
        self.page = page;
        self.layout(false);
        for id in NAV {
            InvalidateRect(GetDlgItem(self.hwnd, id as i32), null(), 0);
        }
        SetFocus(self.first_focus());
    }
    unsafe fn first_focus(&self) -> HWND {
        GetDlgItem(
            self.hwnd,
            match self.page {
                0 => (CARD + self.draft.theme.index()) as i32,
                1 => FAMILY as i32,
                _ => EDITOR_LIST as i32,
            },
        )
    }
    unsafe fn preview_owner(&self) {
        SendMessageW(
            self.owner,
            PREVIEW_APPEARANCE,
            self.draft.theme.index(),
            self.draft.opacity as isize,
        );
    }
    unsafe fn select_skin(&mut self, theme: Theme) {
        self.draft.theme = theme;
        // Radio-group semantics: only the selected card is a Tab stop.
        for (i, candidate) in Theme::ALL.into_iter().enumerate() {
            let card = GetDlgItem(self.hwnd, (CARD + i) as i32);
            let style = GetWindowLongPtrW(card, GWL_STYLE) as u32;
            let style = if candidate == theme {
                style | WS_TABSTOP
            } else {
                style & !WS_TABSTOP
            };
            SetWindowLongPtrW(card, GWL_STYLE, style as isize);
        }
        self.caption();
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN,
        );
        self.preview_owner();
    }
    unsafe fn set_opacity(&mut self, opacity: u8) {
        self.draft.opacity = Settings::clamp_opacity(i64::from(opacity));
        SendDlgItemMessageW(
            self.hwnd,
            OPACITY as i32,
            TBM_SETPOS,
            1,
            opacity_position(self.draft.opacity),
        );
        self.show_opacity();
        self.preview_owner();
    }
    unsafe fn show_opacity(&self) {
        SetDlgItemTextW(
            self.hwnd,
            OPACITY_VALUE as i32,
            w(&if self.draft.opacity >= 100 {
                "100% 不透明".to_string()
            } else {
                format!("{}%", self.draft.opacity)
            })
            .as_ptr(),
        );
        InvalidateRect(GetDlgItem(self.hwnd, OPACITY as i32), null(), 1);
    }
    unsafe fn pick_editor(&mut self) {
        let index = SendDlgItemMessageW(self.hwnd, EDITOR_LIST as i32, CB_GETCURSEL, 0, 0) as usize;
        if let Some((_, editor)) = self.editors.get(index) {
            self.draft.editor = editor.clone();
        }
        self.show_editor();
    }
    unsafe fn show_editor(&self) {
        let text = match &self.draft.editor {
            Editor::System => {
                "使用 Windows 为 Markdown 登记的“编辑”方式；没有登记时改用记事本。".to_string()
            }
            Editor::Notepad => crate::editor::notepad().display().to_string(),
            Editor::Program(path) => path.clone(),
        };
        SetDlgItemTextW(self.hwnd, EDITOR_PATH as i32, w(&text).as_ptr());
    }
    unsafe fn set_draft(&mut self) {
        let family = self
            .families
            .iter()
            .position(|f| f == &self.draft.font_family)
            .unwrap_or(0);
        SendDlgItemMessageW(self.hwnd, FAMILY as i32, CB_SETCURSEL, family, 0);
        SetDlgItemTextW(
            self.hwnd,
            SIZE as i32,
            w(&self.draft.font_size.to_string()).as_ptr(),
        );
        let editor = self
            .editors
            .iter()
            .position(|(_, e)| *e == self.draft.editor)
            .unwrap_or(0);
        SendDlgItemMessageW(self.hwnd, EDITOR_LIST as i32, CB_SETCURSEL, editor, 0);
        SendDlgItemMessageW(
            self.hwnd,
            OPACITY as i32,
            TBM_SETPOS,
            1,
            opacity_position(self.draft.opacity),
        );
        self.show_opacity();
    }
    /// "Restore defaults" resets only the visible page's draft.
    unsafe fn reset_page(&mut self) {
        let defaults = Settings::default();
        match self.page {
            0 => {
                self.draft.opacity = defaults.opacity;
                self.select_skin(defaults.theme);
                self.set_opacity(defaults.opacity);
            }
            1 => {
                self.draft.font_family = defaults.font_family;
                self.draft.font_size = defaults.font_size;
                self.preview = Fonts::with_family(self.ui.dpi, &self.draft.font_family);
                self.set_draft();
                self.update();
            }
            _ => {
                self.draft.editor = defaults.editor;
                self.set_draft();
                self.show_editor();
            }
        }
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
            self.draft.font_size = size;
            if &self.draft.font_family != family {
                self.draft.font_family = family.clone();
                self.preview = Fonts::with_family(self.ui.dpi, family);
            }
        }
        InvalidateRect(self.hwnd, null(), 0);
    }
    unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
        SetDCBrushColor(dc, color);
        FillRect(dc, rect, GetStockObject(DC_BRUSH));
    }
    unsafe fn text(&mut self, dc: HDC, text: &str, rect: RECT, look: Look, color: u32, flags: u32) {
        let Look(size, bold, _) = look;
        SelectObject(
            dc,
            self.ui.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
            }),
        );
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        let mut rect = rect;
        DrawTextW(dc, w(text).as_ptr(), -1, &mut rect, flags);
    }
    unsafe fn background(&mut self, dc: HDC) {
        let saved = SaveDC(dc);
        let p = self.palette();
        let mut client = RECT::default();
        GetClientRect(self.hwnd, &mut client);
        Self::fill(dc, &client, p.page);
        let nav = RECT {
            right: self.s(NAV_WIDTH),
            ..client
        };
        Self::fill(dc, &nav, p.panel);
        Self::fill(
            dc,
            &RECT {
                left: nav.right - 1,
                ..nav
            },
            p.edge,
        );
        Self::fill(dc, &self.rect(X0, 484, WIDTH - X0 - 28, 1), p.edge);
        match self.page {
            1 => {
                chrome::chrome_round(dc, self.rect(X0, 198, 480, 136), p.panel, self.s(14));
                for (text, y, bold) in [
                    ("字里行间，慢慢读。", 214, true),
                    ("FeatherMD · A little room to read.", 270, false),
                ] {
                    let font = self.preview.get(FontKey {
                        size: self.draft.font_size,
                        bold,
                        italic: false,
                        mono: false,
                        strike: false,
                    });
                    SelectObject(dc, font);
                    SetBkMode(dc, TRANSPARENT as i32);
                    SetTextColor(dc, p.fg);
                    let mut rect = self.rect(X0 + 20, y, 440, 48);
                    DrawTextW(
                        dc,
                        w(text).as_ptr(),
                        -1,
                        &mut rect,
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                }
            }
            2 => chrome::chrome_round(dc, self.rect(X0, 222, 480, 140), p.code, self.s(12)),
            _ => {}
        }
        RestoreDC(dc, saved);
    }
    unsafe fn nav(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        let index = NAV
            .iter()
            .position(|id| *id == item.CtlID as usize)
            .unwrap_or(0);
        let active = index == self.page;
        let r = item.rcItem;
        Self::fill(dc, &r, p.panel);
        if active || item.itemState & ODS_SELECTED != 0 || hover(item.hwndItem) {
            chrome::chrome_round(dc, r, if active { p.selection } else { p.code }, self.s(8));
        }
        if active {
            chrome::chrome_round(
                dc,
                RECT {
                    left: r.left + self.s(2),
                    top: r.top + self.s(10),
                    right: r.left + self.s(5),
                    bottom: r.bottom - self.s(10),
                },
                p.accent,
                self.s(2),
            );
        }
        let color = if active { p.accent } else { p.fg };
        smooth::icon(
            dc,
            RECT {
                left: r.left + self.s(12),
                top: r.top,
                right: r.left + self.s(36),
                bottom: r.bottom,
            },
            PAGES[index].1,
            color,
            self.ui.dpi as f32 / 96.0 * 0.92,
        );
        let flags = DT_SINGLELINE
            | DT_VCENTER
            | if item.itemState & ODS_NOACCEL != 0 {
                DT_HIDEPREFIX
            } else {
                0
            };
        self.text(
            dc,
            PAGES[index].0,
            RECT {
                left: r.left + self.s(44),
                ..r
            },
            Look(14, active, Tone::Nav),
            color,
            flags,
        );
        focus_ring(dc, item, self.s(3));
        RestoreDC(dc, saved);
    }
    unsafe fn action(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        let primary = item.CtlID == IDOK as u32 && self.valid;
        Self::fill(dc, &item.rcItem, p.page);
        chrome::chrome_round(
            dc,
            item.rcItem,
            if item.itemState & ODS_SELECTED != 0 {
                p.selection
            } else if primary {
                p.accent
            } else if hover(item.hwndItem) {
                p.code
            } else {
                p.panel
            },
            self.s(8),
        );
        let mut text = [0u16; 64];
        GetWindowTextW(item.hwndItem, text.as_mut_ptr(), 64);
        let label =
            String::from_utf16_lossy(&text[..text.iter().position(|c| *c == 0).unwrap_or(0)]);
        self.text(
            dc,
            &label,
            item.rcItem,
            Look(14, primary, Tone::Text),
            if primary { p.page } else { p.fg },
            DT_CENTER
                | DT_VCENTER
                | DT_SINGLELINE
                | if item.itemState & ODS_NOACCEL != 0 {
                    DT_HIDEPREFIX
                } else {
                    0
                },
        );
        focus_ring(dc, item, self.s(4));
        RestoreDC(dc, saved);
    }
    /// Skin card: a miniature reader (header strip, page, text lines, link) and its name.
    unsafe fn card(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        let index = item.CtlID as usize - CARD;
        let theme = Theme::ALL[index.min(Theme::ALL.len() - 1)];
        let selected = theme == self.draft.theme;
        let r = item.rcItem;
        Self::fill(dc, &r, p.page);
        let swatch = RECT {
            left: r.left + self.s(3),
            top: r.top + self.s(3),
            right: r.right - self.s(3),
            bottom: r.bottom - self.s(26),
        };
        if selected || hover(item.hwndItem) {
            chrome::chrome_round(
                dc,
                RECT {
                    left: swatch.left - self.s(3),
                    top: swatch.top - self.s(3),
                    right: swatch.right + self.s(3),
                    bottom: swatch.bottom + self.s(3),
                },
                if selected { p.accent } else { p.edge },
                self.s(11),
            );
            chrome::chrome_round(
                dc,
                RECT {
                    left: swatch.left - self.s(1),
                    top: swatch.top - self.s(1),
                    right: swatch.right + self.s(1),
                    bottom: swatch.bottom + self.s(1),
                },
                p.page,
                self.s(9),
            );
        }
        // "Follow system" shows its light and dark halves side by side.
        let middle = (swatch.left + swatch.right) / 2;
        let halves = if theme == Theme::System {
            vec![
                (Theme::Light, swatch.left, middle),
                (Theme::Dark, middle, swatch.right),
            ]
        } else {
            vec![(theme, swatch.left, swatch.right)]
        };
        for (skin, left, right) in halves {
            let saved = SaveDC(dc);
            IntersectClipRect(dc, left, swatch.top, right, swatch.bottom);
            self.swatch(dc, swatch, Palette::new(skin));
            RestoreDC(dc, saved);
        }
        if selected {
            let badge = RECT {
                left: swatch.right - self.s(20),
                top: swatch.top + self.s(4),
                right: swatch.right - self.s(4),
                bottom: swatch.top + self.s(20),
            };
            chrome::chrome_round(dc, badge, p.accent, self.s(8));
            smooth::icon(
                dc,
                badge,
                smooth::CHECK,
                p.page,
                self.ui.dpi as f32 / 96.0 * 0.8,
            );
        }
        self.text(
            dc,
            skin_name(theme),
            RECT {
                top: swatch.bottom + self.s(3),
                ..r
            },
            Look(13, selected, Tone::Text),
            if selected { p.accent } else { p.fg },
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        focus_ring(dc, item, self.s(1));
        RestoreDC(dc, saved);
    }
    unsafe fn swatch(&self, dc: HDC, r: RECT, skin: Palette) {
        let radius = self.s(8);
        chrome::chrome_round(dc, r, skin.edge, radius);
        let inner = RECT {
            left: r.left + 1,
            top: r.top + 1,
            right: r.right - 1,
            bottom: r.bottom - 1,
        };
        chrome::chrome_round(dc, inner, skin.bg, radius - 1);
        let header = RECT {
            bottom: inner.top + self.s(11),
            ..inner
        };
        let saved = SaveDC(dc);
        IntersectClipRect(dc, header.left, header.top, header.right, header.bottom);
        chrome::chrome_round(dc, inner, skin.panel, radius - 1);
        RestoreDC(dc, saved);
        Self::fill(
            dc,
            &RECT {
                top: header.bottom,
                bottom: header.bottom + 1,
                ..header
            },
            skin.edge,
        );
        let sheet = RECT {
            left: inner.left + self.s(10),
            top: header.bottom + self.s(5),
            right: inner.right - self.s(10),
            bottom: inner.bottom - self.s(4),
        };
        if skin.page != skin.bg {
            chrome::chrome_round(dc, sheet, skin.page, self.s(3));
        }
        if let Some(rule) = skin.ornament {
            for x in [sheet.left + self.s(4), sheet.right - self.s(5)] {
                Self::fill(
                    dc,
                    &RECT {
                        left: x,
                        top: sheet.top + self.s(3),
                        right: x + 1,
                        bottom: sheet.bottom - self.s(3),
                    },
                    rule,
                );
            }
        }
        let x = sheet.left + self.s(10);
        for (y, width, height, color) in [
            (6, 46, 4, skin.fg),
            (15, 64, 3, skin.muted),
            (22, 54, 3, skin.muted),
            (29, 30, 3, skin.accent),
        ] {
            let top = sheet.top + self.s(y);
            if top + self.s(height) > sheet.bottom {
                break;
            }
            chrome::chrome_round(
                dc,
                RECT {
                    left: x,
                    top,
                    right: (x + self.s(width)).min(sheet.right - self.s(8)),
                    bottom: top + self.s(height),
                },
                color,
                self.s(2),
            );
        }
    }
    /// Closed face of an owner-drawn drop-down list (font family or editor).
    unsafe fn combo(&mut self, hwnd: HWND, dc: HDC) {
        let saved = SaveDC(dc);
        let p = self.palette();
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        Self::fill(dc, &r, p.page);
        chrome::chrome_round(
            dc,
            r,
            if GetFocus() == hwnd { p.accent } else { p.edge },
            self.s(7),
        );
        InflateRect(&mut r, -1, -1);
        chrome::chrome_round(dc, r, p.panel, self.s(6));
        let arrow = RECT {
            left: r.right - self.s(32),
            top: r.top,
            right: r.right - self.s(8),
            bottom: r.bottom,
        };
        smooth::icon(dc, arrow, THEME_PICKER, p.muted, self.ui.dpi as f32 / 96.0);
        let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0) as usize;
        if let Some((name, _)) = self.item(GetDlgCtrlID(hwnd) as usize, index) {
            r.left += self.s(10);
            r.right -= self.s(36);
            self.text(
                dc,
                &name,
                r,
                Look(14, false, Tone::Field),
                p.fg,
                DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        RestoreDC(dc, saved);
    }
    /// Name and optional muted detail of a drop-down entry.
    fn item(&self, id: usize, index: usize) -> Option<(String, Option<String>)> {
        if id == FAMILY {
            return self.families.get(index).map(|f| (f.clone(), None));
        }
        self.editors.get(index).map(|(name, editor)| {
            (
                name.clone(),
                match editor {
                    Editor::Program(path) => Some(path.clone()),
                    _ => None,
                },
            )
        })
    }
    unsafe fn list_item(&mut self, item: &DRAWITEMSTRUCT) {
        let dc = item.hDC;
        let saved = SaveDC(dc);
        let p = self.palette();
        Self::fill(
            dc,
            &item.rcItem,
            if item.itemState & ODS_SELECTED != 0 {
                p.selection
            } else {
                p.panel
            },
        );
        if let Some((name, detail)) = self.item(item.CtlID as usize, item.itemID as usize) {
            let mut r = item.rcItem;
            r.left += self.s(10);
            r.right -= self.s(10);
            let split = r.left + self.s(130);
            self.text(
                dc,
                &name,
                RECT {
                    right: if detail.is_some() { split } else { r.right },
                    ..r
                },
                Look(14, false, Tone::Field),
                p.fg,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
            if let Some(detail) = detail {
                self.text(
                    dc,
                    &detail,
                    RECT {
                        left: split + self.s(8),
                        ..r
                    },
                    Look(12, false, Tone::Muted),
                    p.muted,
                    DT_SINGLELINE | DT_VCENTER | DT_PATH_ELLIPSIS | DT_NOPREFIX | DT_RIGHT,
                );
            }
        }
        RestoreDC(dc, saved);
    }
    /// Opacity slider: rounded channel with the filled part in the accent colour.
    unsafe fn slider(&mut self, draw: &NMCUSTOMDRAW) -> isize {
        if draw.dwDrawStage == CDDS_PREPAINT {
            return CDRF_NOTIFYITEMDRAW as isize;
        }
        if draw.dwDrawStage != CDDS_ITEMPREPAINT {
            return CDRF_DODEFAULT as isize;
        }
        let p = self.palette();
        let mut thumb = RECT::default();
        SendMessageW(
            draw.hdr.hwndFrom,
            TBM_GETTHUMBRECT,
            0,
            &mut thumb as *mut RECT as isize,
        );
        let center = (thumb.left + thumb.right) / 2;
        match draw.dwItemSpec as u32 {
            TBCD_CHANNEL => {
                let middle = (draw.rc.top + draw.rc.bottom) / 2;
                let track = RECT {
                    left: draw.rc.left,
                    top: middle - self.s(2),
                    right: draw.rc.right,
                    bottom: middle + self.s(2),
                };
                chrome::chrome_round(draw.hdc, track, p.edge, self.s(2));
                chrome::chrome_round(
                    draw.hdc,
                    RECT {
                        right: center.max(track.left + self.s(4)),
                        ..track
                    },
                    p.accent,
                    self.s(2),
                );
                CDRF_SKIPDEFAULT as isize
            }
            TBCD_THUMB => {
                let middle = (thumb.top + thumb.bottom) / 2;
                let radius = self.s(8);
                let disc = |r: i32| RECT {
                    left: center - r,
                    top: middle - r,
                    right: center + r,
                    bottom: middle + r,
                };
                chrome::chrome_round(draw.hdc, disc(radius), p.edge, radius);
                chrome::chrome_round(draw.hdc, disc(radius - 1), p.page, radius - 1);
                chrome::chrome_round(draw.hdc, disc(self.s(5)), p.accent, self.s(5));
                CDRF_SKIPDEFAULT as isize
            }
            _ => CDRF_DODEFAULT as isize,
        }
    }
}

/// Owner-drawn controls show the focus cue only after keyboard use.
unsafe fn focus_ring(dc: HDC, item: &DRAWITEMSTRUCT, inset: i32) {
    if item.itemState & ODS_FOCUS != 0 && item.itemState & ODS_NOFOCUSRECT == 0 {
        let mut ring = item.rcItem;
        InflateRect(&mut ring, -inset, -inset);
        DrawFocusRect(dc, &ring);
    }
}
unsafe fn hover(hwnd: HWND) -> bool {
    let mut hover = 0;
    GetWindowSubclass(hwnd, Some(button_proc), 1, &mut hover);
    hover != 0
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
        let ptr = GetWindowLongPtrW(GetParent(hwnd), GWLP_USERDATA) as *const RefCell<Prefs>;
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
/// Owner-drawn buttons: Enter clicks the focused button (except skin cards, where
/// Enter saves like a radio group), arrows move between cards, hover is tracked.
unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    id: usize,
    hover: usize,
) -> isize {
    let control = GetDlgCtrlID(hwnd) as usize;
    let card = (CARD..CARD + Theme::ALL.len()).contains(&control);
    match msg {
        WM_GETDLGCODE if card => {
            return DefSubclassProc(hwnd, msg, wp, lp) | DLGC_WANTARROWS as isize;
        }
        WM_GETDLGCODE if wp == VK_RETURN as usize => {
            return DefSubclassProc(hwnd, msg, wp, lp) | DLGC_WANTMESSAGE as isize;
        }
        WM_KEYDOWN if !card && wp == VK_RETURN as usize => {
            if lp & (1 << 30) == 0 {
                SendMessageW(hwnd, BM_CLICK, 0, 0);
            }
            return 0;
        }
        WM_KEYDOWN if card && (0x25..=0x28).contains(&wp) => {
            let index = (control - CARD) as i32;
            let count = Theme::ALL.len() as i32;
            let next = match wp {
                0x25 => index - 1,
                0x27 => index + 1,
                0x26 => index - 4,
                _ => index + 4,
            };
            if (0..count).contains(&next) {
                let target = GetDlgItem(GetParent(hwnd), CARD as i32 + next);
                SetFocus(target);
                SendMessageW(target, BM_CLICK, 0, 0);
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
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(button_proc), id);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
/// The file picker runs its own modal loop, so no Prefs borrow may cross it.
unsafe fn browse(ptr: *const RefCell<Prefs>) {
    let Ok(hwnd) = (*ptr).try_borrow().map(|s| s.hwnd) else {
        return;
    };
    let mut file = vec![0u16; 32768];
    let filter = w("应用程序 (*.exe)\0*.exe\0");
    let title = w("选择编辑应用");
    let initial: Option<Vec<u16>> =
        std::env::var_os("ProgramFiles").map(|dir| dir.encode_wide().chain(Some(0)).collect());
    let mut dialog: OPENFILENAMEW = zeroed();
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = hwnd;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = file.as_mut_ptr();
    dialog.nMaxFile = file.len() as u32;
    dialog.lpstrTitle = title.as_ptr();
    if let Some(dir) = &initial {
        dialog.lpstrInitialDir = dir.as_ptr();
    }
    dialog.Flags =
        OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_HIDEREADONLY;
    if GetOpenFileNameW(&mut dialog) == 0 {
        return;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    let own = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned));
    let chosen = String::from_utf16(&file[..len]).ok().and_then(|path| {
        Editor::validate_program(&path)
            .filter(|p| {
                own.as_deref()
                    .is_none_or(|own| !own.eq_ignore_ascii_case(p))
            })
            .map(str::to_owned)
    });
    let Some(path) = chosen else {
        MessageBoxW(
            hwnd,
            w("请选择本机磁盘上的 .exe 编辑应用（不能选择 FeatherMD 本身）。").as_ptr(),
            w("选择编辑应用").as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
        return;
    };
    let Ok(mut state) = (*ptr).try_borrow_mut() else {
        return;
    };
    let editor = Editor::Program(path);
    let index = match state.editors.iter().position(|(_, e)| *e == editor) {
        Some(index) => index,
        None => {
            let name = crate::editor::name(&editor);
            SendDlgItemMessageW(
                hwnd,
                EDITOR_LIST as i32,
                CB_ADDSTRING,
                0,
                w(&name).as_ptr() as isize,
            );
            state.editors.push((name, editor.clone()));
            state.editors.len() - 1
        }
    };
    SendDlgItemMessageW(hwnd, EDITOR_LIST as i32, CB_SETCURSEL, index, 0);
    state.draft.editor = editor;
    state.show_editor();
}

unsafe extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    if msg == WM_INITDIALOG {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
        let mut state = (*(lp as *const RefCell<Prefs>)).borrow_mut();
        state.hwnd = hwnd;
        state.init();
        let focus = state.first_focus();
        drop(state);
        SetFocus(focus);
        return 0; // Focus was placed on the requested page's first control.
    }
    if msg == WM_CLOSE || (msg == WM_COMMAND && wp & 0xffff == IDCANCEL as usize) {
        EndDialog(hwnd, IDCANCEL as isize);
        return 1;
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<Prefs>;
    if ptr.is_null() {
        return 0;
    }
    if msg == WM_APP + 42 && wp == 4 {
        let captured = snapshot::capture(hwnd, |dc| {
            let mut state = (*ptr).borrow_mut();
            state.background(dc);
            let page = state.page;
            let children: Vec<HWND> = state
                .controls
                .iter()
                .filter(|c| c.page.is_none_or(|p| p == page))
                .map(|c| c.hwnd)
                .collect();
            // Children paint through WM_CTLCOLOR/WM_DRAWITEM/WM_NOTIFY back into us.
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
                let id = GetDlgCtrlID(child) as usize;
                SendMessageW(
                    child,
                    if id == FAMILY || id == EDITOR_LIST {
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
    if msg == WM_COMMAND && wp & 0xffff == BROWSE {
        browse(ptr);
        return 1;
    }
    let Ok(mut state) = (*ptr).try_borrow_mut() else {
        return 0;
    };
    match msg {
        WM_COMMAND => {
            let id = wp & 0xffff;
            let code = wp >> 16;
            if id == IDOK as usize {
                state.update();
                if state.valid {
                    state.pick_editor();
                    state.result = Some(state.draft.clone());
                    drop(state);
                    EndDialog(hwnd, IDOK as isize);
                } else {
                    // The only invalid field lives on the reading page.
                    state.show_page(1);
                }
            } else if id == DEFAULTS {
                state.reset_page();
            } else if let Some(page) = NAV.iter().position(|n| *n == id) {
                state.show_page(page);
            } else if (CARD..CARD + Theme::ALL.len()).contains(&id) {
                state.select_skin(Theme::ALL[id - CARD]);
            } else if (id == FAMILY && code == CBN_SELCHANGE as usize)
                || (id == SIZE && code == EN_CHANGE as usize)
            {
                state.update();
            } else if id == EDITOR_LIST && code == CBN_SELCHANGE as usize {
                state.pick_editor();
            }
            1
        }
        WM_HSCROLL if lp as HWND == GetDlgItem(hwnd, OPACITY as i32) => {
            let position = SendMessageW(lp as HWND, TBM_GETPOS, 0, 0);
            state.set_opacity((position * 5).clamp(0, 100) as u8);
            1
        }
        WM_NOTIFY => {
            let header = &*(lp as *const NMHDR);
            if header.idFrom == OPACITY && header.code == NM_CUSTOMDRAW {
                let result = state.slider(&*(lp as *const NMCUSTOMDRAW));
                SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, result);
                return 1;
            }
            0
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
            item.itemHeight = state.s(26) as u32;
            1
        }
        WM_DRAWITEM => {
            let item = &*(lp as *const DRAWITEMSTRUCT);
            let id = item.CtlID as usize;
            if id == FAMILY || id == EDITOR_LIST {
                state.list_item(item);
            } else if (CARD..CARD + Theme::ALL.len()).contains(&id) {
                state.card(item);
            } else if NAV.contains(&id) {
                state.nav(item);
            } else {
                state.action(item);
            }
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let p = state.palette();
            let tone = state
                .controls
                .iter()
                .find(|c| c.hwnd == lp as HWND)
                .map_or(Tone::Text, |c| c.look.2);
            let (fg, bg) = match tone {
                Tone::Text => (p.fg, p.page),
                Tone::Muted => (p.muted, p.page),
                Tone::Accent => (p.accent, p.page),
                Tone::Nav => (p.fg, p.panel),
                Tone::NavMuted => (p.muted, p.panel),
                Tone::Info => (p.fg, p.code),
                Tone::InfoMuted => (p.muted, p.code),
                Tone::Field => (p.fg, p.panel),
            };
            SetTextColor(wp as HDC, fg);
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

// Dialog templates require DWORD alignment and three trailing empty UTF-16 fields.
#[repr(C, align(4))]
struct EmptyDialog {
    template: DLGTEMPLATE,
    menu: u16,
    class: u16,
    title: u16,
}
/// Modal Settings opened on `page` (0 appearance, 1 reading, 2 editor).
/// Returns the saved draft, or None when cancelled.
pub(super) unsafe fn show(owner: HWND, settings: &Settings, page: usize) -> Option<Settings> {
    let classes = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_BAR_CLASSES,
    };
    InitCommonControlsEx(&classes);
    let dpi = GetDpiForWindow(owner).max(96);
    let state = RefCell::new(Prefs {
        hwnd: null_mut(),
        owner,
        page: page.min(PAGES.len() - 1),
        draft: settings.clone(),
        ui: Fonts::new(dpi),
        preview: Fonts::with_family(dpi, &settings.font_family),
        families: Vec::new(),
        editors: Vec::new(),
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
            cx: 400,
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
        &state as *const RefCell<Prefs> as isize,
    );
    if result == -1 {
        MessageBoxW(
            owner,
            w("无法创建设置窗口。").as_ptr(),
            w("FeatherMD").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
    state.into_inner().result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opacity_slider_positions_cover_the_setting_range() {
        assert_eq!(opacity_position(Settings::MIN_OPACITY), 6);
        assert_eq!(opacity_position(100), 20);
        for opacity in (Settings::MIN_OPACITY..=100).step_by(5) {
            assert_eq!(opacity_position(opacity) * 5, isize::from(opacity));
        }
        assert_eq!(std::mem::offset_of!(EmptyDialog, menu), 18);
        assert_eq!(PAGES.len(), NAV.len());
    }
}
