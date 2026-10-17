//! Distribution boundaries. Package detection errors fail closed: never fall
//! back to portable registration, updates or shared settings inside a package.
use std::{io, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Identity {
    Portable,
    Package(String),
    Unavailable,
}

pub fn identity() -> &'static Identity {
    static IDENTITY: std::sync::OnceLock<Identity> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(detect)
}

pub fn store() -> bool {
    if cfg!(feature = "store") {
        // The packer rejects a portable binary, even if somebody renames it.
        std::hint::black_box("FEATHERMD_STORE_CHANNEL_V1");
        true
    } else {
        !matches!(identity(), Identity::Portable)
    }
}

#[cfg(not(windows))]
fn detect() -> Identity {
    Identity::Portable
}

#[cfg(windows)]
fn detect() -> Identity {
    use windows_sys::Win32::{
        Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_SUCCESS},
        Storage::Packaging::Appx::GetCurrentPackageFamilyName,
    };
    let mut buffer = [0u16; 256];
    let mut len = buffer.len() as u32;
    let status = unsafe { GetCurrentPackageFamilyName(&mut len, buffer.as_mut_ptr()) };
    if status == APPMODEL_ERROR_NO_PACKAGE {
        return Identity::Portable;
    }
    if status == ERROR_SUCCESS
        && len > 1
        && len as usize <= buffer.len()
        && let Ok(family) = String::from_utf16(&buffer[..len as usize - 1])
        && valid_family(&family)
    {
        return Identity::Package(family);
    }
    Identity::Unavailable
}

fn valid_family(family: &str) -> bool {
    !family.is_empty()
        && family.len() <= 255
        && family.contains('_')
        && family
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
}

/// Resolve against unredirected LocalAppData, not the read-only installation
/// directory and not the portable edition's roaming INI.
pub fn data_path(id: &Identity, local: PathBuf) -> io::Result<PathBuf> {
    match id {
        Identity::Package(family) if valid_family(family) => Ok(local
            .join("Packages")
            .join(family)
            .join("LocalState")
            .join("settings.ini")),
        Identity::Portable => Ok(local
            .join("FeatherMD")
            .join("StorePreview")
            .join("settings.ini")),
        _ => Err(io::Error::other(
            "Cannot resolve the package settings folder",
        )),
    }
}

#[cfg(windows)]
pub fn settings_path() -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_NO_PACKAGE_REDIRECTION, SHGetKnownFolderPath},
    };
    let mut path = std::ptr::null_mut();
    let result = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_LocalAppData,
            KF_FLAG_NO_PACKAGE_REDIRECTION as u32,
            std::ptr::null_mut(),
            &mut path,
        )
    };
    if result < 0 || path.is_null() {
        return Err(io::Error::other(format!(
            "Cannot resolve LocalAppData: {result:#x}"
        )));
    }
    let local = unsafe {
        let mut len = 0;
        while *path.add(len) != 0 {
            len += 1;
        }
        let local = std::ffi::OsString::from_wide(std::slice::from_raw_parts(path, len));
        CoTaskMemFree(path.cast());
        PathBuf::from(local)
    };
    data_path(identity(), local)
}

#[cfg(not(windows))]
pub fn settings_path() -> io::Result<PathBuf> {
    Err(io::Error::other("Store settings require Windows"))
}

pub const STORE_LIBRARY: &str = "ms-windows-store://downloadsandupdates";

pub fn default_apps_uri(id: &Identity) -> String {
    match id {
        Identity::Package(family) if valid_family(family) => {
            format!("ms-settings:defaultapps?registeredAUMID={family}%21FeatherMD")
        }
        Identity::Portable if !cfg!(feature = "store") => {
            "ms-settings:defaultapps?registeredAppUser=FeatherMD".into()
        }
        _ => "ms-settings:defaultapps".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_data_is_private_and_identity_errors_do_not_fall_back() {
        let local = PathBuf::from("local");
        let package = Identity::Package("Example.FeatherMD_abc123".into());
        assert_eq!(
            data_path(&package, local.clone()).unwrap(),
            local.join("Packages/Example.FeatherMD_abc123/LocalState/settings.ini")
        );
        assert_ne!(
            data_path(&package, local.clone()).unwrap(),
            data_path(&Identity::Portable, local.clone()).unwrap()
        );
        assert!(data_path(&Identity::Unavailable, local.clone()).is_err());
        for bad in ["../escape_x", "a\\b_x", "a:b_x", "", "a_x?bad"] {
            assert!(data_path(&Identity::Package(bad.into()), local.clone()).is_err());
        }
    }

    #[test]
    fn defaults_target_the_packaged_application_not_a_versioned_exe() {
        assert_eq!(
            default_apps_uri(&Identity::Package("Example.FeatherMD_abc123".into())),
            "ms-settings:defaultapps?registeredAUMID=Example.FeatherMD_abc123%21FeatherMD"
        );
        assert_eq!(
            default_apps_uri(&Identity::Unavailable),
            "ms-settings:defaultapps"
        );
    }
}
