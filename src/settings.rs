use std::{
    fs, io,
    path::{Path, PathBuf},
};
/// Reader skins. `System` follows the Windows app theme; every other value is fixed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
    Sepia,
    Book,
    Ancient,
    Green,
    Sky,
    Pink,
    Gray,
    Midnight,
    Black,
}
impl Theme {
    /// Display and persistence order; indices are stable for diagnostics.
    pub const ALL: [Self; 12] = [
        Self::System,
        Self::Light,
        Self::Dark,
        Self::Sepia,
        Self::Book,
        Self::Ancient,
        Self::Green,
        Self::Sky,
        Self::Pink,
        Self::Gray,
        Self::Midnight,
        Self::Black,
    ];
    pub fn resolve(self, system_dark: bool) -> Self {
        match self {
            Self::System if system_dark => Self::Dark,
            Self::System => Self::Light,
            theme => theme,
        }
    }
    /// Call on a resolved theme; `System` itself is neither light nor dark.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::Midnight | Self::Black)
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Sepia => "sepia",
            Self::Book => "book",
            Self::Ancient => "ancient",
            Self::Green => "green",
            Self::Sky => "sky",
            Self::Pink => "pink",
            Self::Gray => "gray",
            Self::Midnight => "midnight",
            Self::Black => "black",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }
}
/// External application used for "Edit". FeatherMD itself never writes documents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Editor {
    /// The registered Windows "edit" verb, falling back to Notepad.
    #[default]
    System,
    Notepad,
    /// A validated absolute path to an executable chosen by the user.
    Program(String),
}
impl Editor {
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "system" | "" => Self::System,
            "notepad" => Self::Notepad,
            path => Self::validate_program(path).map_or(Self::System, |p| Self::Program(p.into())),
        }
    }
    pub fn key(&self) -> &str {
        match self {
            Self::System => "system",
            Self::Notepad => "notepad",
            Self::Program(path) => Self::validate_program(path).unwrap_or("system"),
        }
    }
    /// Local drive-letter `.exe` paths only: no quotes, control characters,
    /// streams, wildcards, relative or UNC paths. The document path is passed
    /// as a separate quoted argument, never concatenated into this value.
    pub fn validate_program(value: &str) -> Option<&str> {
        let value = value.trim();
        let bytes = value.as_bytes();
        (value.len() >= 8
            && value.encode_utf16().count() <= 1024
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
            && !value
                .chars()
                .any(|c| c.is_control() || "\"<>|?*/".contains(c))
            && !value[2..].contains(':')
            && value.to_ascii_lowercase().ends_with(".exe"))
        .then_some(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub font_family: String,
    pub font_size: i32,
    pub theme: Theme,
    /// Whole-window opacity in percent, in steps of 5.
    pub opacity: u8,
    pub editor: Editor,
    pub wrap: bool,
    pub raw: bool,
    pub wide: bool,
    pub outline: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font_family: "Microsoft YaHei UI".into(),
            font_size: 18,
            theme: Theme::System,
            opacity: 100,
            editor: Editor::System,
            wrap: true,
            raw: false,
            wide: false,
            outline: true,
        }
    }
}
impl Settings {
    /// The window must never become invisible, so 30% is the floor.
    pub const MIN_OPACITY: u8 = 30;

    /// LOGFONTW has room for 31 UTF-16 units plus NUL; reject INI/control injection.
    pub fn validate_font_family(value: &str) -> Option<&str> {
        let value = value.trim();
        (!value.is_empty()
            && value.encode_utf16().count() <= 31
            && !value.chars().any(char::is_control)
            && !value.starts_with('@'))
        .then_some(value)
    }

    /// Clamp to 30–100 and round to the nearest step of 5.
    pub fn clamp_opacity(value: i64) -> u8 {
        let value = value.clamp(i64::from(Self::MIN_OPACITY), 100);
        (((value + 2) / 5) * 5) as u8
    }

    pub fn path() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            let portable = exe.with_file_name("FeatherMD.ini");
            if portable.exists() {
                return portable;
            }
        }
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("FeatherMD")
            .join("settings.ini")
    }
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        // Notepad and PowerShell 5 may save UTF-8 with a BOM; keep the first key readable.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.lines() {
            if let Some((key, value)) = line.trim().split_once('=') {
                match key.trim() {
                    "font_family" => {
                        if let Some(family) = Self::validate_font_family(value) {
                            s.font_family = family.to_string();
                        }
                    }
                    "font_size" => {
                        if let Ok(v) = value.trim().parse::<i32>() {
                            s.font_size = v.clamp(12, 36);
                        }
                    }
                    "theme" => s.theme = Theme::from_key(value.trim()).unwrap_or_default(),
                    "opacity" => {
                        if let Ok(v) = value.trim().trim_end_matches('%').parse::<i64>() {
                            s.opacity = Self::clamp_opacity(v);
                        }
                    }
                    "editor" => s.editor = Editor::parse(value),
                    "wrap" => s.wrap = value.trim() != "false",
                    "raw" => s.raw = value.trim() == "true",
                    "wide" => s.wide = value.trim() == "true",
                    "outline" => s.outline = value.trim() != "false",
                    _ => {}
                }
            }
        }
        s
    }
    pub fn serialize(&self) -> String {
        format!(
            "# FeatherMD settings\nfont_family={}\nfont_size={}\ntheme={}\nopacity={}\neditor={}\nwrap={}\nraw={}\nwide={}\noutline={}\n",
            Self::validate_font_family(&self.font_family).unwrap_or("Microsoft YaHei UI"),
            self.font_size,
            self.theme.key(),
            Self::clamp_opacity(i64::from(self.opacity)),
            self.editor.key(),
            self.wrap,
            self.raw,
            self.wide,
            self.outline
        )
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temp, self.serialize())?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
            };
            let a: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let b: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe {
                MoveFileExW(
                    a.as_ptr(),
                    b.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                let e = io::Error::last_os_error();
                let _ = fs::remove_file(&temp);
                return Err(e);
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            fs::rename(temp, path)
        }
    }
}
