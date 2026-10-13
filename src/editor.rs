//! External editing. FeatherMD stays read-only: "Edit" hands the current
//! document to the user's chosen application as one quoted argument.
//! Author / company / last editor: lianzy.
use feathermd::{i18n::tr, settings::Editor};
use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::*,
    System::Registry::*,
    UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Well-known editors. Detection only checks metadata of these fixed paths and
/// per-application App Paths entries; nothing is launched or enumerated.
const KNOWN: [(&str, &str, &[&str]); 9] = [
    (
        "VS Code",
        "Code.exe",
        &[
            r"%LOCALAPPDATA%\Programs\Microsoft VS Code",
            r"%ProgramFiles%\Microsoft VS Code",
        ],
    ),
    (
        "VSCodium",
        "VSCodium.exe",
        &[
            r"%LOCALAPPDATA%\Programs\VSCodium",
            r"%ProgramFiles%\VSCodium",
        ],
    ),
    ("Cursor", "Cursor.exe", &[r"%LOCALAPPDATA%\Programs\cursor"]),
    (
        "Typora",
        "Typora.exe",
        &[r"%ProgramFiles%\Typora", r"%LOCALAPPDATA%\Programs\Typora"],
    ),
    (
        "MarkText",
        "MarkText.exe",
        &[
            r"%LOCALAPPDATA%\Programs\marktext",
            r"%ProgramFiles%\MarkText",
        ],
    ),
    (
        "Notepad++",
        "notepad++.exe",
        &[
            r"%ProgramFiles%\Notepad++",
            r"%ProgramFiles(x86)%\Notepad++",
        ],
    ),
    (
        "Sublime Text",
        "sublime_text.exe",
        &[
            r"%ProgramFiles%\Sublime Text",
            r"%ProgramFiles%\Sublime Text 3",
        ],
    ),
    ("Zed", "Zed.exe", &[r"%LOCALAPPDATA%\Programs\Zed"]),
    ("EmEditor", "EmEditor.exe", &[r"%ProgramFiles%\EmEditor"]),
];

fn expand(template: &str) -> Option<PathBuf> {
    let rest = template.strip_prefix('%')?;
    let (name, tail) = rest.split_once('%')?;
    let base = std::env::var_os(name)?;
    Some(PathBuf::from(base).join(tail.trim_start_matches('\\')))
}

fn app_path(exe: &str) -> Option<PathBuf> {
    let key = wide(&format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe}"
    ));
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut buffer = vec![0u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        let code = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if code == 0 {
            let len = (bytes as usize / 2).saturating_sub(1).min(buffer.len());
            let text = String::from_utf16_lossy(&buffer[..len]);
            let text = text.trim_matches(['"', ' ', '\0']);
            let path = PathBuf::from(text);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// Installed editors that pass `Editor::validate_program`, deduplicated.
pub fn detect() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    for (name, exe, dirs) in KNOWN {
        let hit = dirs
            .iter()
            .filter_map(|dir| expand(dir))
            .map(|dir| dir.join(exe))
            .find(|path| path.is_file())
            .or_else(|| app_path(exe));
        if let Some(path) = hit
            && let Some(path) = path.to_str()
            && let Some(valid) = Editor::validate_program(path)
            && !found.iter().any(|(_, p)| p.eq_ignore_ascii_case(valid))
        {
            found.push((name.to_string(), valid.to_string()));
        }
    }
    found
}

/// Short user-facing name for the configured editor.
pub fn name(editor: &Editor) -> String {
    match editor {
        Editor::System => tr("系统默认").into(),
        Editor::Notepad => tr("记事本").into(),
        Editor::Program(path) => {
            let file = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            KNOWN
                .iter()
                .find(|(_, exe, _)| exe.eq_ignore_ascii_case(file))
                .map(|(name, _, _)| name.to_string())
                .unwrap_or_else(|| {
                    Path::new(path)
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path)
                        .to_string()
                })
        }
    }
}

pub fn notepad() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join(r"System32\notepad.exe")
}

pub enum Launched {
    Editor,
    /// Windows has no registered edit verb for this file type.
    NotepadFallback,
}

pub enum LaunchError {
    MissingProgram(String),
    Failed(io::Error),
}

pub fn launch(editor: &Editor, owner: HWND, document: &Path) -> Result<Launched, LaunchError> {
    // A relative command-line path must not be resolved against the editor's directory.
    let document = std::path::absolute(document).map_err(LaunchError::Failed)?;
    if !document.is_file() {
        return Err(LaunchError::Failed(io::Error::new(
            io::ErrorKind::NotFound,
            tr("文档已不存在或已被移动"),
        )));
    }
    let directory = document.parent().map(Path::to_path_buf);
    let text = document.as_os_str().to_string_lossy();
    // Windows file names cannot contain quotes; reject anything that could split arguments.
    if text.contains('"') {
        return Err(LaunchError::Failed(io::Error::new(
            io::ErrorKind::InvalidInput,
            tr("文档路径包含无效字符"),
        )));
    }
    let argument = format!("\"{text}\"");
    match editor {
        Editor::System => {
            match execute(owner, Some("edit"), &document, None, directory.as_deref()) {
                Ok(()) => Ok(Launched::Editor),
                Err(error)
                    if error.raw_os_error().is_some_and(|code| {
                        code as u32 == ERROR_NO_ASSOCIATION || code == SE_ERR_NOASSOC as i32
                    }) =>
                {
                    execute(
                        owner,
                        None,
                        &notepad(),
                        Some(&argument),
                        directory.as_deref(),
                    )
                    .map(|()| Launched::NotepadFallback)
                    .map_err(LaunchError::Failed)
                }
                Err(error) => Err(LaunchError::Failed(error)),
            }
        }
        Editor::Notepad => execute(
            owner,
            None,
            &notepad(),
            Some(&argument),
            directory.as_deref(),
        )
        .map(|()| Launched::Editor)
        .map_err(LaunchError::Failed),
        Editor::Program(path) => {
            let program = Path::new(path);
            if Editor::validate_program(path).is_none() || !program.is_file() {
                return Err(LaunchError::MissingProgram(path.clone()));
            }
            execute(owner, None, program, Some(&argument), directory.as_deref())
                .map(|()| Launched::Editor)
                .map_err(LaunchError::Failed)
        }
    }
}

fn execute(
    owner: HWND,
    verb: Option<&str>,
    file: &Path,
    parameters: Option<&str>,
    directory: Option<&Path>,
) -> io::Result<()> {
    let to_wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain(Some(0)).collect() };
    let verb = verb.map(wide);
    let file = to_wide(file);
    let parameters = parameters.map(wide);
    let directory = directory.map(to_wide);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    // No shell error UI: FeatherMD reports the failure itself. NOASYNC keeps
    // DDE/COM verbs from depending on this thread after the call returns.
    info.fMask = SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
    info.hwnd = owner;
    info.lpVerb = verb.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
    info.lpDirectory = directory.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
    info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut info) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_names_and_templates_are_stable() {
        assert_eq!(name(&Editor::System), "系统默认");
        assert_eq!(name(&Editor::Notepad), "记事本");
        assert_eq!(
            name(&Editor::Program(
                r"C:\Users\a\AppData\Local\Programs\Microsoft VS Code\Code.exe".into()
            )),
            "VS Code"
        );
        assert_eq!(
            name(&Editor::Program(r"D:\工具\MyEditor.EXE".into())),
            "MyEditor"
        );
        assert!(notepad().ends_with(r"System32\notepad.exe"));
        assert_eq!(expand("relative"), None);
        for (_, exe, _) in KNOWN {
            assert!(exe.to_ascii_lowercase().ends_with(".exe"));
        }
        // Detection never returns a path the settings parser would reject.
        for (_, path) in detect() {
            assert_eq!(Editor::validate_program(&path), Some(path.as_str()));
        }
    }
}
