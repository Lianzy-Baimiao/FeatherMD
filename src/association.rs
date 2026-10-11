//! Per-user registration only. Never write the protected UserChoice key.
use std::{io, os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::{System::Registry::*, UI::Shell::*};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn value(path: &str, name: &str, text: &str) -> io::Result<()> {
    unsafe {
        let mut key = std::ptr::null_mut();
        let code = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let data = wide(text);
        let code = RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        );
        RegCloseKey(key);
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
    }
    Ok(())
}
pub fn register(exe: &Path) -> io::Result<()> {
    let exe = exe.as_os_str().encode_wide().collect::<Vec<_>>();
    let exe = String::from_utf16(&exe)
        .map_err(|_| io::Error::other("可执行文件路径包含无法注册的 Unicode 字符"))?;
    let command = format!("\"{exe}\" \"%1\"");
    value(
        "Software\\Classes\\FeatherMD.Document",
        "",
        "Markdown 文档 (FeatherMD)",
    )?;
    value(
        "Software\\Classes\\FeatherMD.Document\\DefaultIcon",
        "",
        &format!("\"{exe}\",0"),
    )?;
    value(
        "Software\\Classes\\FeatherMD.Document\\shell\\open\\command",
        "",
        &command,
    )?;
    value(
        "Software\\FeatherMD\\Capabilities",
        "ApplicationName",
        "FeatherMD",
    )?;
    value(
        "Software\\FeatherMD\\Capabilities",
        "ApplicationDescription",
        "轻量原生 Markdown 查看器",
    )?;
    for ext in [".md", ".markdown", ".mdown", ".mkd"] {
        value(
            "Software\\FeatherMD\\Capabilities\\FileAssociations",
            ext,
            "FeatherMD.Document",
        )?;
        // OpenWithProgids entries are REG_NONE, not extension defaults.
        unsafe {
            let mut key = std::ptr::null_mut();
            let path = wide(&format!("Software\\Classes\\{ext}\\OpenWithProgids"));
            let code = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            );
            if code != 0 {
                return Err(io::Error::from_raw_os_error(code as i32));
            }
            let result = RegSetValueExW(
                key,
                wide("FeatherMD.Document").as_ptr(),
                0,
                REG_NONE,
                std::ptr::null(),
                0,
            );
            RegCloseKey(key);
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result as i32));
            }
        }
    }
    value(
        "Software\\RegisteredApplications",
        "FeatherMD",
        "Software\\FeatherMD\\Capabilities",
    )?;
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
    Ok(())
}
pub fn unregister() -> io::Result<()> {
    let mut first_error = None;
    unsafe {
        for path in [
            "Software\\Classes\\FeatherMD.Document",
            "Software\\FeatherMD\\Capabilities",
        ] {
            let code = RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr());
            if code != 0 && code != 2 {
                first_error.get_or_insert(io::Error::from_raw_os_error(code as i32));
            }
        }
        for (path, name) in
            std::iter::once(("Software\\RegisteredApplications".to_owned(), "FeatherMD")).chain(
                [".md", ".markdown", ".mdown", ".mkd"].map(|e| {
                    (
                        format!("Software\\Classes\\{e}\\OpenWithProgids"),
                        "FeatherMD.Document",
                    )
                }),
            )
        {
            let mut key = std::ptr::null_mut();
            let code = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(&path).as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            );
            if code == 0 {
                let result = RegDeleteValueW(key, wide(name).as_ptr());
                RegCloseKey(key);
                if result != 0 && result != 2 {
                    first_error.get_or_insert(io::Error::from_raw_os_error(result as i32));
                }
            } else if code != 2 {
                first_error.get_or_insert(io::Error::from_raw_os_error(code as i32));
            }
        }
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
    first_error.map_or(Ok(()), Err)
}
