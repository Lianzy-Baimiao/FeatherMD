use std::{
    fs, io,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    Sepia,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
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
            font_size: 18,
            theme: Theme::Light,
            wrap: true,
            raw: false,
            wide: false,
            outline: true,
        }
    }
}
impl Settings {
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
                    "font_size" => {
                        if let Ok(v) = value.trim().parse::<i32>() {
                            s.font_size = v.clamp(12, 36);
                        }
                    }
                    "theme" => {
                        s.theme = match value.trim() {
                            "dark" => Theme::Dark,
                            "sepia" => Theme::Sepia,
                            _ => Theme::Light,
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
            "# FeatherMD settings\nfont_size={}\ntheme={}\nwrap={}\nraw={}\nwide={}\noutline={}\n",
            self.font_size,
            match self.theme {
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
