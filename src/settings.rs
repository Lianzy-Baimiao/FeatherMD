use std::{
    fs, io,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
    Sepia,
}
impl Theme {
    pub fn resolve(self, system_dark: bool) -> Self {
        match self {
            Self::System if system_dark => Self::Dark,
            Self::System => Self::Light,
            theme => theme,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub font_family: String,
    pub font_size: i32,
    pub theme: Theme,
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
            wrap: true,
            raw: false,
            wide: false,
            outline: true,
        }
    }
}
impl Settings {
    /// LOGFONTW has room for 31 UTF-16 units plus NUL; reject INI/control injection.
    pub fn validate_font_family(value: &str) -> Option<&str> {
        let value = value.trim();
        (!value.is_empty()
            && value.encode_utf16().count() <= 31
            && !value.chars().any(char::is_control)
            && !value.starts_with('@'))
        .then_some(value)
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
                    "theme" => {
                        s.theme = match value.trim() {
                            "dark" => Theme::Dark,
                            "sepia" => Theme::Sepia,
                            "light" => Theme::Light,
                            _ => Theme::System,
                        }
                    }
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
            "# FeatherMD settings\nfont_family={}\nfont_size={}\ntheme={}\nwrap={}\nraw={}\nwide={}\noutline={}\n",
            Self::validate_font_family(&self.font_family).unwrap_or("Microsoft YaHei UI"),
            self.font_size,
            match self.theme {
                Theme::System => "system",
                Theme::Light => "light",
                Theme::Dark => "dark",
                Theme::Sepia => "sepia",
            },
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
