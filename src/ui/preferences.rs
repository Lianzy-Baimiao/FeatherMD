//! Settings: appearance (skins, window opacity), reading font, external editor,
//! update checks and interface language.
//! Draft state never escapes until Save; skin/opacity drafts preview live on the
//! reader through PREVIEW_APPEARANCE and are reverted by the owner on Cancel.
use super::*;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::UI::Controls::{
    BST_CHECKED, CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDIS_DISABLED, CDIS_FOCUS, CDIS_HOT,
    CDIS_SELECTED, CDIS_SHOWKEYBOARDCUES, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, CDRF_SKIPDEFAULT,
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
const SHOW_IMAGES: usize = 1105;
const NAV: [usize; 5] = [1110, 1111, 1112, 1113, 1114];
/// Skin cards 1120–1139, one per `Theme::GALLERY` entry, five to a row.
const CARD: usize = 1120;
const GALLERY_COLUMNS: i32 = 5;
const OPACITY: usize = 1140;
const OPACITY_VALUE: usize = 1141;
const EDITOR_LIST: usize = 1150;
const BROWSE: usize = 1151;
const EDITOR_PATH: usize = 1152;
const AUTO_CHECK: usize = 1160;
const CHECK_NOW: usize = 1161;
const UPDATE_OPEN: usize = 1162;
const UPDATE_STATUS: usize = 1163;
const UPDATE_DETAIL: usize = 1164;
const UPDATE_LAST: usize = 1165;
const LANGUAGE_LIST: usize = 1170;
/// Language list order; `None` follows the Windows display language.
const LANGUAGES: [Option<Lang>; 5] = [
    None,
    Some(Lang::ZhCn),
    Some(Lang::ZhTw),
    Some(Lang::En),
    Some(Lang::Ja),
];
/// Polls a manual update check; the check itself runs off the UI thread.
const UPDATE_POLL: usize = 7;
const WIDTH: i32 = 720;
const HEIGHT: i32 = 552;
const NAV_WIDTH: i32 = 184;
/// Content column origin.
const X0: i32 = 212;
const PAGES: [(&str, usize); 5] = [
    (key!("外观(&A)"), smooth::PALETTE),
    (key!("阅读(&R)"), smooth::BOOK),
    (key!("编辑(&E)"), smooth::PENCIL),
    (key!("更新(&U)"), smooth::UPDATE),
    (key!("语言(&L)"), smooth::GLOBE),
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
    /// Manual update check in flight, and the release a finished check found.
    checking: Option<crate::updater::Pending>,
    found: Option<feathermd::update::Release>,
    /// Colour of the update status line (accent for a newer release).
    update_tone: Tone,
    /// Start a manual check once the dialog exists (More → 检查更新).
    check_now: bool,
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
        SetWindowTextW(self.hwnd, w(tr("设置")).as_ptr());
        let dc = GetDC(self.hwnd);
        let mut names = std::collections::BTreeSet::new();
        let mut lf: LOGFONTW = zeroed();
        lf.lfCharSet = DEFAULT_CHARSET;
        EnumFontFamiliesExW(dc, &lf, Some(enum_font), &mut names as *mut _ as isize, 0);
        ReleaseDC(self.hwnd, dc);
        // Keep a previously saved choice visible even if a font was uninstalled.
        names.insert(self.draft.reading_font().to_owned());
        names.insert(i18n::current().reading_font().to_owned());
        self.families = names.into_iter().collect();
        // Fixed metadata probes only; see editor::detect.
        self.editors = vec![
            (tr("系统默认").into(), Editor::System),
            (tr("记事本").into(), Editor::Notepad),
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
            tr("设置"),
            1200,
            (None, [24, 22, 140, 30]),
            Look(18, true, Tone::Nav),
        );
        for (i, (name, _)) in PAGES.into_iter().enumerate() {
            self.button(tr(name), NAV[i], (None, [14, 72 + i as i32 * 42, 156, 36]));
        }
        self.label(
            tr("FeatherMD · 本地保存"),
            1201,
            (None, [24, HEIGHT - 40, 150, 20]),
            Look(11, false, Tone::NavMuted),
        );

        // Appearance.
        let page = Some(0);
        self.label(tr("皮肤"), 1210, (page, [X0, 24, 300, 30]), TITLE);
        self.label(
            tr("纸张、书本、夜读与常用配色，选择后立即预览。"),
            1211,
            (page, [X0, 58, 470, 22]),
            NOTE,
        );
        // Card ids follow the gallery order: light skins, then dark ones.
        for (i, theme) in Theme::GALLERY.into_iter().enumerate() {
            let (column, row) = (i as i32 % GALLERY_COLUMNS, i as i32 / GALLERY_COLUMNS);
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
                (page, [X0 + column * 97, 88 + row * 76, 88, 70]),
                BODY,
            );
            SetWindowSubclass(card, Some(button_proc), 1, 0);
        }
        self.label(tr("窗口不透明度"), 1212, (page, [X0, 394, 200, 22]), BODY);
        self.add(
            "STATIC",
            "",
            OPACITY_VALUE,
            SS_NOPREFIX | SS_RIGHT,
            (page, [X0 + 300, 394, 176, 22]),
            Look(14, true, Tone::Accent),
        );
        let slider = self.add(
            "msctls_trackbar32",
            tr("窗口不透明度"),
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
            tr("30%–100%，数值越低越透明；只作用于阅读窗口，设置窗口保持不透明。"),
            1213,
            (page, [X0, 452, 480, 20]),
            Look(12, false, Tone::Muted),
        );

        // Reading (ids kept from the former font dialog).
        let page = Some(1);
        self.label(tr("阅读字体"), 1220, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            tr("让每一页，都更合你的阅读习惯。"),
            1221,
            (page, [X0, 58, 470, 22]),
            NOTE,
        );
        self.label(tr("字体(&F)"), 1202, (page, [X0, 98, 320, 22]), BODY);
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
        self.label(
            tr("默认字号(&S)"),
            1203,
            (page, [X0 + 360, 98, 120, 22]),
            BODY,
        );
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
            tr("仅正文与标题使用此字体，代码仍保持等宽。"),
            1206,
            (page, [X0, 310, 480, 22]),
            NOTE,
        );
        self.label(
            tr("字号范围 12–36 px · 保存后自动记住"),
            1207,
            (page, [X0, 334, 480, 22]),
            NOTE,
        );
        let images = self.add(
            "BUTTON",
            tr("显示图片(&I)"),
            SHOW_IMAGES,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            (page, [X0, 376, 360, 26]),
            BODY,
        );
        SendMessageW(
            images,
            BM_SETCHECK,
            if self.draft.images {
                BST_CHECKED as usize
            } else {
                0
            },
            0,
        );
        self.label(
            tr("开启后自动显示本机图片；网络图片需点击占位文字并确认后加载。"),
            1208,
            (page, [X0 + 28, 404, 452, 40]),
            Look(12, false, Tone::Muted),
        );

        // External editor.
        let page = Some(2);
        self.label(tr("默认编辑应用"), 1230, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            tr("FeatherMD 只读；编辑时把当前文档交给你选择的应用。"),
            1231,
            (page, [X0, 58, 480, 22]),
            NOTE,
        );
        self.label(tr("编辑应用(&D)"), 1232, (page, [X0, 98, 320, 22]), BODY);
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
        self.button(tr("浏览(&B)…"), BROWSE, (page, [X0 + 372, 126, 108, 30]));
        self.add(
            "STATIC",
            "",
            EDITOR_PATH,
            SS_NOPREFIX | SS_PATHELLIPSIS,
            (page, [X0, 164, 480, 40]),
            Look(12, false, Tone::Muted),
        );
        self.label(
            tr("使用方式"),
            1233,
            (page, [X0 + 20, 238, 440, 24]),
            Look(14, true, Tone::Info),
        );
        for (i, text) in [
            tr("正文右键 → 编辑，或按 Ctrl+E，用所选应用打开当前文档。"),
            tr("保存后回到 FeatherMD 按 F5 重新载入，阅读位置保持不变。"),
            tr("不会修改 Windows 的默认打开方式或文件关联。"),
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

        // Updates.
        let page = Some(3);
        self.label(tr("检查更新"), 1240, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            tr("更新源：GitHub Releases（Lianzy-Baimiao/FeatherMD）。"),
            1241,
            (page, [X0, 58, 480, 22]),
            NOTE,
        );
        self.label(tr("当前版本"), 1242, (page, [X0, 98, 120, 22]), BODY);
        self.label(
            concat!("v", env!("CARGO_PKG_VERSION")),
            1243,
            (page, [X0 + 120, 98, 200, 22]),
            Look(14, true, Tone::Accent),
        );
        let auto = self.add(
            "BUTTON",
            tr("打开时自动检查更新(&S)"),
            AUTO_CHECK,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            (page, [X0, 134, 360, 26]),
            BODY,
        );
        SendMessageW(
            auto,
            BM_SETCHECK,
            if self.draft.update_check {
                BST_CHECKED as usize
            } else {
                0
            },
            0,
        );
        self.label(
            tr("默认关闭。开启后启动几秒在后台检查，每天最多一次，失败时不打扰。"),
            1244,
            (page, [X0 + 28, 162, 452, 20]),
            Look(12, false, Tone::Muted),
        );
        self.button(tr("立即检查(&C)"), CHECK_NOW, (page, [X0, 198, 120, 32]));
        let open = self.button(
            tr("前往下载页…"),
            UPDATE_OPEN,
            (page, [X0 + 132, 198, 130, 32]),
        );
        ShowWindow(open, SW_HIDE);
        self.label(
            "",
            UPDATE_STATUS,
            (page, [X0, 242, 480, 24]),
            Look(14, true, Tone::Text),
        );
        self.label("", UPDATE_DETAIL, (page, [X0, 268, 480, 22]), NOTE);
        self.label(
            "",
            UPDATE_LAST,
            (page, [X0, 292, 480, 20]),
            Look(12, false, Tone::Muted),
        );
        self.label(
            tr("隐私与安全"),
            1245,
            (page, [X0 + 20, 338, 440, 24]),
            Look(14, true, Tone::Info),
        );
        for (i, text) in [
            tr("只向 api.github.com 请求公开的最新版本信息，不上传文档或设置。"),
            tr("不会自动下载或安装；前往下载页前会再次确认。"),
            tr("关闭自动检查且不点“立即检查”时，FeatherMD 不会联网。"),
        ]
        .into_iter()
        .enumerate()
        {
            self.label(
                text,
                1246 + i,
                (page, [X0 + 20, 368 + i as i32 * 26, 440, 22]),
                Look(13, false, Tone::InfoMuted),
            );
        }

        // Interface language.
        let page = Some(4);
        self.label(tr("界面语言"), 1250, (page, [X0, 24, 400, 30]), TITLE);
        self.label(
            tr("FeatherMD 的菜单、按钮和提示使用的语言。"),
            1251,
            (page, [X0, 58, 480, 22]),
            NOTE,
        );
        self.label(tr("语言(&G)"), 1252, (page, [X0, 98, 320, 22]), BODY);
        let languages = self.add(
            "COMBOBOX",
            "",
            LANGUAGE_LIST,
            WS_TABSTOP
                | WS_VSCROLL
                | CBS_DROPDOWNLIST as u32
                | CBS_OWNERDRAWFIXED as u32
                | CBS_HASSTRINGS as u32,
            (page, [X0, 126, 360, 200]),
            Look(14, false, Tone::Field),
        );
        SetWindowSubclass(languages, Some(combo_proc), 1, 0);
        for choice in LANGUAGES {
            SendMessageW(
                languages,
                CB_ADDSTRING,
                0,
                w(&language_name(choice)).as_ptr() as isize,
            );
        }
        self.label(
            tr("说明"),
            1253,
            (page, [X0 + 20, 198, 440, 24]),
            Look(14, true, Tone::Info),
        );
        for (i, text) in [
            tr("保存后立即切换，无需重新启动。"),
            tr("跟随系统时使用 Windows 显示语言；没有对应翻译时显示英文。"),
            tr("文档内容与阅读字体不受影响。"),
        ]
        .into_iter()
        .enumerate()
        {
            self.label(
                text,
                1254 + i,
                (page, [X0 + 20, 230 + i as i32 * 28, 440, 22]),
                Look(13, false, Tone::InfoMuted),
            );
        }

        // Shared actions.
        self.button(tr("恢复默认"), DEFAULTS, (None, [X0, 500, 106, 32]));
        self.button(
            tr("取消"),
            IDCANCEL as usize,
            (None, [WIDTH - 220, 500, 86, 32]),
        );
        self.button(
            tr("保存"),
            IDOK as usize,
            (None, [WIDTH - 122, 500, 94, 32]),
        );
        self.set_draft();
        self.layout(true);
        self.update();
        self.show_editor();
        self.show_update();
        SendMessageW(self.hwnd, DM_SETDEFID, IDOK as usize, 0);
    }
    /// Starts a manual check; the result arrives through UPDATE_POLL.
    unsafe fn start_check(&mut self) {
        if self.checking.is_some() {
            return;
        }
        self.checking = Some(crate::updater::start());
        SetTimer(self.hwnd, UPDATE_POLL, 50, None);
        self.show_update();
    }
    /// Status line, detail, last-check time and button states for the update page.
    unsafe fn show_update(&mut self) {
        let current = feathermd::update::current();
        let latest = feathermd::update::Version::parse(&self.draft.update_latest);
        let (status, detail, tone) = if self.checking.is_some() {
            (tr("正在检查更新…").to_string(), String::new(), Tone::Muted)
        } else if let Some(release) = &self.found {
            let mut detail = release.title.clone();
            if !release.published.is_empty() {
                detail = trf("{}　发布于 {}", &[&detail, &release.published]);
            }
            match release.version.cmp(&current) {
                std::cmp::Ordering::Greater => (
                    trf("发现新版本 {}", &[&release.tag]),
                    detail.trim().to_string(),
                    Tone::Accent,
                ),
                std::cmp::Ordering::Equal => (
                    tr("已是最新版本").to_string(),
                    detail.trim().to_string(),
                    Tone::Text,
                ),
                std::cmp::Ordering::Less => (
                    tr("当前版本比 GitHub 最新发布更新").to_string(),
                    trf("最新发布 {}", &[&release.tag]),
                    Tone::Text,
                ),
            }
        } else if let Some(latest) = latest.filter(|latest| *latest > current) {
            (
                trf("发现新版本 {}", &[&format!("v{latest}")]),
                tr("点“立即检查”刷新信息").to_string(),
                Tone::Accent,
            )
        } else {
            (String::new(), String::new(), Tone::Text)
        };
        self.update_tone = tone;
        SetDlgItemTextW(self.hwnd, UPDATE_STATUS as i32, w(&status).as_ptr());
        SetDlgItemTextW(self.hwnd, UPDATE_DETAIL as i32, w(&detail).as_ptr());
        let last = trf(
            "上次成功检查：{}",
            &[&feathermd::update::ago(
                self.draft.update_checked,
                feathermd::update::now(),
            )],
        );
        SetDlgItemTextW(self.hwnd, UPDATE_LAST as i32, w(&last).as_ptr());
        EnableWindow(
            GetDlgItem(self.hwnd, CHECK_NOW as i32),
            self.checking.is_none() as i32,
        );
        let newer = self.release_url().is_some();
        ShowWindow(
            GetDlgItem(self.hwnd, UPDATE_OPEN as i32),
            if newer && self.page == 3 {
                SW_SHOWNA
            } else {
                SW_HIDE
            },
        );
        InvalidateRect(self.hwnd, null(), 0);
    }
    /// Release page for a newer version, from a fresh result or the saved tag.
    fn release_url(&self) -> Option<String> {
        let current = feathermd::update::current();
        match &self.found {
            Some(release) => (release.version > current).then(|| release.page()),
            None => feathermd::update::Version::parse(&self.draft.update_latest)
                .filter(|latest| *latest > current)
                .and_then(|_| feathermd::update::release_page(&self.draft.update_latest)),
        }
    }
    /// A finished manual check: shown here, recorded by the reader.
    unsafe fn finish_check(&mut self, outcome: &crate::updater::Outcome) {
        self.checking = None;
        KillTimer(self.hwnd, UPDATE_POLL);
        match outcome {
            Ok(release) => {
                self.draft.update_checked = feathermd::update::now();
                self.draft.update_latest = release.tag.clone();
                self.found = Some(release.clone());
                self.show_update();
            }
            Err(error) => {
                self.show_update();
                self.update_tone = Tone::Muted;
                SetDlgItemTextW(self.hwnd, UPDATE_STATUS as i32, w(tr("检查失败")).as_ptr());
                SetDlgItemTextW(self.hwnd, UPDATE_DETAIL as i32, w(error).as_ptr());
            }
        }
    }
    unsafe fn layout(&mut self, center: bool) {
        let old_ui =
            std::mem::replace(&mut self.ui, Fonts::new(GetDpiForWindow(self.hwnd).max(96)));
        self.preview = Fonts::with_family(self.ui.dpi, self.draft.reading_font());
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
        // "Go to download" only appears for a newer release.
        let download = self.release_url().is_some();
        for control in &self.controls {
            let [x, y, w, h] = control.rect;
            let Look(size, bold, _) = control.look;
            let font = self.ui.get(FontKey {
                size,
                bold,
                italic: false,
                mono: false,
                strike: false,
                underline: false,
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
            let visible = control.page.is_none_or(|page| page == self.page)
                && (download || GetDlgCtrlID(control.hwnd) != UPDATE_OPEN as i32);
            ShowWindow(control.hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
        }
        for id in [FAMILY, EDITOR_LIST, LANGUAGE_LIST] {
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
                0 => (CARD + self.draft.theme.gallery_index()) as i32,
                1 => FAMILY as i32,
                2 => EDITOR_LIST as i32,
                4 => LANGUAGE_LIST as i32,
                // "Check now" is disabled while a check runs.
                _ if self.checking.is_some() => AUTO_CHECK as i32,
                _ => CHECK_NOW as i32,
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
        for (i, candidate) in Theme::GALLERY.into_iter().enumerate() {
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
                tr("100% 不透明").to_string()
            } else {
                format!("{}%", self.draft.opacity)
            })
            .as_ptr(),
        );
        InvalidateRect(GetDlgItem(self.hwnd, OPACITY as i32), null(), 1);
    }
    unsafe fn pick_language(&mut self) {
        let index = SendDlgItemMessageW(self.hwnd, LANGUAGE_LIST as i32, CB_GETCURSEL, 0, 0);
        if let Some(choice) = usize::try_from(index).ok().and_then(|i| LANGUAGES.get(i)) {
            self.draft.language = *choice;
        }
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
                tr("使用 Windows 为 Markdown 登记的“编辑”方式；没有登记时改用记事本。").to_string()
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
            .position(|f| f == self.draft.reading_font())
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
        let language = LANGUAGES
            .iter()
            .position(|choice| *choice == self.draft.language)
            .unwrap_or(0);
        SendDlgItemMessageW(self.hwnd, LANGUAGE_LIST as i32, CB_SETCURSEL, language, 0);
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
                self.draft.images = defaults.images;
                SendDlgItemMessageW(self.hwnd, SHOW_IMAGES as i32, BM_SETCHECK, 0, 0);
                self.preview = Fonts::with_family(self.ui.dpi, self.draft.reading_font());
                self.set_draft();
                self.update();
            }
            2 => {
                self.draft.editor = defaults.editor;
                self.set_draft();
                self.show_editor();
            }
            3 => {
                self.draft.update_check = defaults.update_check;
                SendDlgItemMessageW(self.hwnd, AUTO_CHECK as i32, BM_SETCHECK, 0, 0);
            }
            _ => {
                self.draft.language = defaults.language;
                self.set_draft();
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
                tr("实时预览")
            } else {
                tr("请输入 12–36 之间的字号")
            })
            .as_ptr(),
        );
        EnableWindow(GetDlgItem(self.hwnd, IDOK), self.valid as i32);
        if let Some(size) = size
            && let Some(family) = self.families.get(index)
        {
            self.draft.font_size = size;
            if self.draft.reading_font() != family {
                // The language's own face stays automatic, so it follows a later
                // language change; any other face is remembered by name.
                self.draft.font_family = if family == i18n::current().reading_font() {
                    String::new()
                } else {
                    family.clone()
                };
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
                underline: false,
            }),
        );
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        let mut rect = rect;
        let text = w(text);
        snapshot::clip(self.hwnd, dc, &text, rect, flags);
        DrawTextW(dc, text.as_ptr(), -1, &mut rect, flags);
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
                chrome::chrome_round(dc, self.rect(X0, 196, 480, 100), p.panel, self.s(14));
                for (text, y, bold) in [
                    (tr("字里行间，慢慢读。"), 204, true),
                    ("FeatherMD · A little room to read.", 246, false),
                ] {
                    let font = self.preview.get(FontKey {
                        size: self.draft.font_size,
                        bold,
                        italic: false,
                        mono: false,
                        strike: false,
                        underline: false,
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
            3 => chrome::chrome_round(dc, self.rect(X0, 326, 480, 128), p.code, self.s(12)),
            4 => chrome::chrome_round(dc, self.rect(X0, 182, 480, 132), p.code, self.s(12)),
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
            tr(PAGES[index].0),
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
        let theme = Theme::GALLERY[index.min(Theme::GALLERY.len() - 1)];
        let selected = theme == self.draft.theme;
        let r = item.rcItem;
        Self::fill(dc, &r, p.page);
        let swatch = RECT {
            left: r.left + self.s(3),
            top: r.top + self.s(3),
            right: r.right - self.s(3),
            bottom: r.bottom - self.s(22),
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
            bottom: inner.top + self.s(9),
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
            left: inner.left + self.s(8),
            top: header.bottom + self.s(4),
            right: inner.right - self.s(8),
            bottom: inner.bottom - self.s(3),
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
        // A heading, a line of text and a link, sized to the sheet.
        let x = sheet.left + self.s(6);
        let room = (sheet.right - self.s(6) - x).max(1);
        for (y, percent, height, color) in [
            (5, 62, 4, skin.fg),
            (13, 100, 3, skin.muted),
            (20, 46, 3, skin.accent),
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
                    right: x + room * percent / 100,
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
        if id == LANGUAGE_LIST {
            return LANGUAGES
                .get(index)
                .map(|choice| (language_name(*choice), None));
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
    /// A real BS_AUTOCHECKBOX (keyboard, accessibility, BM_GETCHECK) drawn in the
    /// skin's colours; themed checkboxes ignore WM_CTLCOLOR text colours.
    unsafe fn checkbox(&mut self, draw: &NMCUSTOMDRAW) -> isize {
        if draw.dwDrawStage != CDDS_PREPAINT {
            return CDRF_DODEFAULT as isize;
        }
        let p = self.palette();
        let dc = draw.hdc;
        let saved = SaveDC(dc);
        let r = draw.rc;
        let checked = SendMessageW(draw.hdr.hwndFrom, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let state = draw.uItemState;
        Self::fill(dc, &r, p.page);
        let size = self.s(18);
        let top = (r.top + r.bottom - size) / 2;
        let bx = RECT {
            left: r.left + self.s(1),
            top,
            right: r.left + self.s(1) + size,
            bottom: top + size,
        };
        let hot = state & (CDIS_HOT | CDIS_SELECTED) != 0;
        if checked {
            chrome::chrome_round(dc, bx, p.accent, self.s(5));
            smooth::icon(
                dc,
                bx,
                smooth::CHECK,
                p.page,
                self.ui.dpi as f32 / 96.0 * 0.85,
            );
        } else {
            chrome::chrome_round(dc, bx, if hot { p.accent } else { p.muted }, self.s(5));
            let inner = RECT {
                left: bx.left + self.s(1).max(1),
                top: bx.top + self.s(1).max(1),
                right: bx.right - self.s(1).max(1),
                bottom: bx.bottom - self.s(1).max(1),
            };
            chrome::chrome_round(dc, inner, if hot { p.code } else { p.page }, self.s(4));
        }
        let mut text = [0u16; 128];
        GetWindowTextW(draw.hdr.hwndFrom, text.as_mut_ptr(), text.len() as i32);
        let label =
            String::from_utf16_lossy(&text[..text.iter().position(|c| *c == 0).unwrap_or(0)]);
        let mut label_rect = RECT {
            left: bx.right + self.s(10),
            ..r
        };
        self.text(
            dc,
            &label,
            label_rect,
            BODY,
            if state & CDIS_DISABLED != 0 {
                p.muted
            } else {
                p.fg
            },
            DT_SINGLELINE
                | DT_VCENTER
                | if state & CDIS_SHOWKEYBOARDCUES == 0 {
                    DT_HIDEPREFIX
                } else {
                    0
                },
        );
        if state & CDIS_FOCUS != 0 && state & CDIS_SHOWKEYBOARDCUES != 0 {
            let mut measure = label_rect;
            DrawTextW(
                dc,
                w(&label).as_ptr(),
                -1,
                &mut measure,
                DT_SINGLELINE | DT_CALCRECT,
            );
            label_rect.right = measure.right + self.s(3);
            label_rect.left -= self.s(3);
            label_rect.top = top - self.s(2);
            label_rect.bottom = top + size + self.s(2);
            DrawFocusRect(dc, &label_rect);
        }
        RestoreDC(dc, saved);
        CDRF_SKIPDEFAULT as isize
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

/// Language names stay in their own language so each reader can find theirs.
fn language_name(choice: Option<Lang>) -> String {
    match choice {
        Some(lang) => lang.native().to_string(),
        None => trf("跟随系统（{}）", &[&Lang::system().native()]),
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
    let card = (CARD..CARD + Theme::GALLERY.len()).contains(&control);
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
            let count = Theme::GALLERY.len() as i32;
            let next = match wp {
                0x25 => index - 1,
                0x27 => index + 1,
                0x26 => index - GALLERY_COLUMNS,
                _ => index + GALLERY_COLUMNS,
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
    let filter = w(tr("应用程序 (*.exe)\0*.exe\0"));
    let title = w(tr("选择编辑应用"));
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
            w(tr(
                "请选择本机磁盘上的 .exe 编辑应用（不能选择 FeatherMD 本身）。",
            ))
            .as_ptr(),
            w(tr("选择编辑应用")).as_ptr(),
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
        if state.check_now {
            state.start_check();
        }
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
        snapshot::clear_clips(hwnd);
        let captured = snapshot::capture(hwnd, |dc| {
            let mut state = (*ptr).borrow_mut();
            state.background(dc);
            // WM_PRINT also draws hidden windows, so follow each control's own
            // WS_VISIBLE style (the dialog itself may be hidden by the test).
            let children: Vec<HWND> = state
                .controls
                .iter()
                .filter(|c| GetWindowLongPtrW(c.hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0)
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
                    if id == FAMILY || id == EDITOR_LIST || id == LANGUAGE_LIST {
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
    if msg == WM_APP + 42 && wp == 5 {
        // Diagnostics: text of the last capture that does not fit (labels.txt).
        SetWindowLongPtrW(hwnd, DWLP_MSGRESULT as i32, snapshot::report(hwnd));
        return 1;
    }
    if msg == WM_COMMAND && wp & 0xffff == BROWSE {
        browse(ptr);
        return 1;
    }
    if msg == WM_COMMAND && wp & 0xffff == UPDATE_OPEN {
        // The link confirmation runs a modal loop; no Prefs borrow may cross it.
        let url = (*ptr).try_borrow().ok().and_then(|s| s.release_url());
        if let Some(url) = url {
            about::open_link(hwnd, &url);
        }
        return 1;
    }
    if msg == WM_TIMER && wp == UPDATE_POLL {
        let finished = (*ptr).try_borrow_mut().ok().and_then(|mut state| {
            let outcome = state.checking.as_ref()?.poll()?;
            state.finish_check(&outcome);
            Some((state.owner, outcome))
        });
        // The reader records the result (last check, latest tag, header badge).
        if let Some((owner, outcome)) = finished {
            SendMessageW(owner, UPDATE_RESULT, 0, &outcome as *const _ as isize);
        }
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
                    state.pick_language();
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
            } else if (CARD..CARD + Theme::GALLERY.len()).contains(&id) {
                state.select_skin(Theme::GALLERY[id - CARD]);
            } else if (id == FAMILY && code == CBN_SELCHANGE as usize)
                || (id == SIZE && code == EN_CHANGE as usize)
            {
                state.update();
            } else if id == EDITOR_LIST && code == CBN_SELCHANGE as usize {
                state.pick_editor();
            } else if id == LANGUAGE_LIST && code == CBN_SELCHANGE as usize {
                state.pick_language();
            } else if id == CHECK_NOW {
                state.start_check();
            } else if id == AUTO_CHECK {
                state.draft.update_check =
                    SendDlgItemMessageW(hwnd, AUTO_CHECK as i32, BM_GETCHECK, 0, 0)
                        == BST_CHECKED as isize;
            } else if id == SHOW_IMAGES {
                state.draft.images =
                    SendDlgItemMessageW(hwnd, SHOW_IMAGES as i32, BM_GETCHECK, 0, 0)
                        == BST_CHECKED as isize;
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
            if header.code == NM_CUSTOMDRAW
                && matches!(header.idFrom, OPACITY | AUTO_CHECK | SHOW_IMAGES)
            {
                let draw = &*(lp as *const NMCUSTOMDRAW);
                let result = if header.idFrom == OPACITY {
                    state.slider(draw)
                } else {
                    state.checkbox(draw)
                };
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
            if id == FAMILY || id == EDITOR_LIST || id == LANGUAGE_LIST {
                state.list_item(item);
            } else if (CARD..CARD + Theme::GALLERY.len()).contains(&id) {
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
            let tone = if GetDlgCtrlID(lp as HWND) == UPDATE_STATUS as i32 {
                state.update_tone
            } else {
                state
                    .controls
                    .iter()
                    .find(|c| c.hwnd == lp as HWND)
                    .map_or(Tone::Text, |c| c.look.2)
            };
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
/// Modal Settings opened on `page` (0 appearance, 1 reading, 2 editor, 3 updates,
/// 4 language);
/// `check_now` starts a manual update check. Returns the saved draft, or None
/// when cancelled. Check results are sent to the owner as UPDATE_RESULT either way.
pub(super) unsafe fn show(
    owner: HWND,
    settings: &Settings,
    page: usize,
    check_now: bool,
) -> Option<Settings> {
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
        preview: Fonts::with_family(dpi, settings.reading_font()),
        families: Vec::new(),
        editors: Vec::new(),
        valid: true,
        controls: Vec::new(),
        result: None,
        checking: None,
        found: None,
        update_tone: Tone::Text,
        check_now: check_now && page == 3,
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
            w(tr("无法创建设置窗口。")).as_ptr(),
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
