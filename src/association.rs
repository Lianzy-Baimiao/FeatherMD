//! Per-user registration only. Never write extension defaults or UserChoice.
//! Author / company / last editor: lianzy.
use feathermd::i18n::{tr, trf};
use std::{io, os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::{System::Registry::*, UI::Shell::*};

const PROGID: &str = "FeatherMD.Document";
const PROGID_PATH: &str = "Software\\Classes\\FeatherMD.Document";
const CAPABILITIES: &str = "Software\\FeatherMD\\Capabilities";
const REGISTERED_APPLICATIONS: &str = "Software\\RegisteredApplications";
const EXTENSIONS: [&str; 4] = [".md", ".markdown", ".mdown", ".mkd"];
const DESCRIPTION: &str = feathermd::key!("轻量原生 Markdown 查看器");

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[derive(Clone, Copy)]
enum Operation {
    Create,
    Open,
    Write,
    Delete,
}

fn registry_error(code: u32, operation: Operation, path: &str, name: &str) -> io::Error {
    let source = io::Error::from_raw_os_error(code as i32);
    // Whole sentences per operation, so each language can order its own words.
    let template = match operation {
        Operation::Create => feathermd::key!("创建注册表项 {}\\{} 失败：{}"),
        Operation::Open => feathermd::key!("打开注册表项 {}\\{} 失败：{}"),
        Operation::Write => feathermd::key!("写入注册表项 {}\\{} 失败：{}"),
        Operation::Delete => feathermd::key!("删除注册表项 {}\\{} 失败：{}"),
    };
    io::Error::new(source.kind(), trf(template, &[&path, &name, &source]))
}

fn is_missing(code: u32) -> bool {
    code == 2 || code == 3 // ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

/// The root is borrowed. Production uses HKCU; tests supply a disposable subtree.
struct Registry {
    root: HKEY,
}

impl Registry {
    fn create(&self, path: &str, access: u32) -> io::Result<Key> {
        let mut key = std::ptr::null_mut();
        let code = unsafe {
            RegCreateKeyExW(
                self.root,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if code != 0 {
            return Err(registry_error(code, Operation::Create, path, ""));
        }
        Ok(Key(key))
    }

    fn open(&self, path: &str, access: u32) -> io::Result<Option<Key>> {
        let mut key = std::ptr::null_mut();
        let code = unsafe { RegOpenKeyExW(self.root, wide(path).as_ptr(), 0, access, &mut key) };
        if is_missing(code) {
            return Ok(None);
        }
        if code != 0 {
            return Err(registry_error(code, Operation::Open, path, ""));
        }
        Ok(Some(Key(key)))
    }

    fn set(&self, path: &str, name: &str, kind: u32, data: &[u8]) -> io::Result<()> {
        let key = self.create(path, KEY_SET_VALUE)?;
        let code = unsafe {
            RegSetValueExW(
                key.0,
                wide(name).as_ptr(),
                0,
                kind,
                if data.is_empty() {
                    std::ptr::null()
                } else {
                    data.as_ptr()
                },
                data.len() as u32,
            )
        };
        if code != 0 {
            return Err(registry_error(code, Operation::Write, path, name));
        }
        Ok(())
    }

    fn text(&self, path: &str, name: &str, text: &str) -> io::Result<()> {
        let data: Vec<u8> = wide(text).into_iter().flat_map(u16::to_le_bytes).collect();
        self.set(path, name, REG_SZ, &data)
    }

    fn delete_tree(&self, path: &str) -> io::Result<()> {
        let code = unsafe { RegDeleteTreeW(self.root, wide(path).as_ptr()) };
        if code != 0 && !is_missing(code) {
            return Err(registry_error(code, Operation::Delete, path, ""));
        }
        Ok(())
    }

    fn delete_value(&self, path: &str, name: &str) -> io::Result<()> {
        let Some(key) = self.open(path, KEY_SET_VALUE)? else {
            return Ok(());
        };
        let code = unsafe { RegDeleteValueW(key.0, wide(name).as_ptr()) };
        if code != 0 && !is_missing(code) {
            return Err(registry_error(code, Operation::Delete, path, name));
        }
        Ok(())
    }

    fn register(&self, exe: &Path) -> io::Result<()> {
        let exe = exe.as_os_str().encode_wide().collect::<Vec<_>>();
        let exe = String::from_utf16(&exe)
            .map_err(|_| io::Error::other(tr("可执行文件路径包含无法注册的 Unicode 字符")))?;
        if exe.contains(['\0', '"']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                tr("可执行文件路径包含无效的空字符或引号"),
            ));
        }
        let icon = format!("\"{exe}\",0");
        self.text(PROGID_PATH, "", tr("Markdown 文档 (FeatherMD)"))?;
        self.text(&format!("{PROGID_PATH}\\DefaultIcon"), "", &icon)?;
        self.text(
            &format!("{PROGID_PATH}\\shell\\open\\command"),
            "",
            &format!("\"{exe}\" \"%1\""),
        )?;
        // The ProgID's Application metadata supplies a friendly Open With entry.
        for path in [
            format!("{PROGID_PATH}\\Application"),
            CAPABILITIES.to_owned(),
        ] {
            self.text(&path, "ApplicationName", "FeatherMD")?;
            self.text(&path, "ApplicationDescription", tr(DESCRIPTION))?;
            self.text(&path, "ApplicationIcon", &icon)?;
        }
        for ext in EXTENSIONS {
            self.text(&format!("{CAPABILITIES}\\FileAssociations"), ext, PROGID)?;
            // A REG_NONE marker offers Open With without taking the default.
            self.set(
                &format!("Software\\Classes\\{ext}\\OpenWithProgids"),
                PROGID,
                REG_NONE,
                &[],
            )?;
        }
        self.text(REGISTERED_APPLICATIONS, "FeatherMD", CAPABILITIES)
    }

    fn unregister(&self) -> io::Result<()> {
        // Attempt all owned entries even when one fails, preserving the first error.
        let mut first_error = None;
        for result in [
            self.delete_tree(PROGID_PATH),
            self.delete_tree(CAPABILITIES),
            self.delete_value(REGISTERED_APPLICATIONS, "FeatherMD"),
        ]
        .into_iter()
        .chain(EXTENSIONS.map(|ext| {
            self.delete_value(
                &format!("Software\\Classes\\{ext}\\OpenWithProgids"),
                PROGID,
            )
        })) {
            if let Err(error) = result {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn notify_associations_changed() {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

pub fn register(exe: &Path) -> io::Result<()> {
    require_portable()?;
    Registry {
        root: HKEY_CURRENT_USER,
    }
    .register(exe)?;
    notify_associations_changed();
    Ok(())
}

pub fn unregister() -> io::Result<()> {
    require_portable()?;
    Registry {
        root: HKEY_CURRENT_USER,
    }
    .unregister()?;
    notify_associations_changed();
    Ok(())
}

fn require_portable() -> io::Result<()> {
    if feathermd::distribution::store() {
        Err(io::Error::other(tr(
            "文件关联由 Windows 随商店包管理，无需手动注册",
        )))
    } else {
        Ok(())
    }
}

#[cfg(all(test, feature = "store"))]
#[test]
fn store_registry_entry_points_are_refused_before_any_write() {
    assert!(register(Path::new("not-an-exe")).is_err());
    assert!(unregister().is_err());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    /// No predefined-key override, shell notification, or real association writes.
    struct Sandbox {
        key: Option<Key>,
        path: String,
    }

    impl Sandbox {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let path = format!(
                "Software\\FeatherMD.AssociationTests\\{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            );
            let key = Registry {
                root: HKEY_CURRENT_USER,
            }
            .create(&path, KEY_ALL_ACCESS)
            .unwrap();
            Self {
                key: Some(key),
                path,
            }
        }

        fn registry(&self) -> Registry {
            Registry {
                root: self.key.as_ref().unwrap().0,
            }
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            drop(self.key.take());
            let result = Registry {
                root: HKEY_CURRENT_USER,
            }
            .delete_tree(&self.path);
            if !std::thread::panicking() {
                result.expect("remove isolated association test subtree");
            }
        }
    }

    fn read(registry: &Registry, path: &str, name: &str) -> Option<(u32, Vec<u8>)> {
        let key = registry.open(path, KEY_QUERY_VALUE).unwrap()?;
        let mut kind = 0;
        let mut size = 0;
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                wide(name).as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if is_missing(code) {
            return None;
        }
        assert_eq!(code, 0, "query size: {path}\\{name}");
        let mut data = vec![0u8; size as usize];
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                wide(name).as_ptr(),
                std::ptr::null(),
                &mut kind,
                data.as_mut_ptr(),
                &mut size,
            )
        };
        assert_eq!(code, 0, "query data: {path}\\{name}");
        data.truncate(size as usize);
        Some((kind, data))
    }

    fn assert_text(registry: &Registry, path: &str, name: &str, value: &str) {
        let bytes = wide(value).into_iter().flat_map(u16::to_le_bytes).collect();
        assert_eq!(
            read(registry, path, name),
            Some((REG_SZ, bytes)),
            "{path}\\{name}"
        );
    }

    #[test]
    fn association_register_is_complete_repeatable_and_quotes_unicode_paths() {
        let sandbox = Sandbox::new();
        let registry = sandbox.registry();
        // Re-register also updates the executable location after moving a portable copy.
        registry
            .register(Path::new(r"C:\旧位置\FeatherMD.exe"))
            .unwrap();
        let exe = r"C:\用户 程序\Feather MD\FeatherMD.exe";
        registry.register(Path::new(exe)).unwrap();
        registry.register(Path::new(exe)).unwrap();
        assert_text(&registry, PROGID_PATH, "", "Markdown 文档 (FeatherMD)");
        assert_text(
            &registry,
            &format!("{PROGID_PATH}\\shell\\open\\command"),
            "",
            &format!("\"{exe}\" \"%1\""),
        );
        let icon = format!("\"{exe}\",0");
        assert_text(&registry, &format!("{PROGID_PATH}\\DefaultIcon"), "", &icon);
        for path in [
            format!("{PROGID_PATH}\\Application"),
            CAPABILITIES.to_owned(),
        ] {
            assert_text(&registry, &path, "ApplicationName", "FeatherMD");
            assert_text(&registry, &path, "ApplicationDescription", DESCRIPTION);
            assert_text(&registry, &path, "ApplicationIcon", &icon);
        }
        assert_text(
            &registry,
            REGISTERED_APPLICATIONS,
            "FeatherMD",
            CAPABILITIES,
        );
        for ext in EXTENSIONS {
            assert_text(
                &registry,
                &format!("{CAPABILITIES}\\FileAssociations"),
                ext,
                PROGID,
            );
            assert_eq!(
                read(
                    &registry,
                    &format!("Software\\Classes\\{ext}\\OpenWithProgids"),
                    PROGID
                ),
                Some((REG_NONE, vec![])),
            );
            assert_eq!(
                read(&registry, &format!("Software\\Classes\\{ext}"), ""),
                None
            );
        }
    }

    #[test]
    fn association_unregister_preserves_other_apps_and_defaults_and_is_idempotent() {
        let sandbox = Sandbox::new();
        let registry = sandbox.registry();
        registry.unregister().unwrap();
        let mut preserved = vec![
            (
                REGISTERED_APPLICATIONS.to_owned(),
                "OtherApp",
                "Software\\OtherApp\\Capabilities",
            ),
            (
                "Software\\Classes\\OtherApp.Document".to_owned(),
                "",
                "Other document",
            ),
            (
                "Software\\FeatherMD".to_owned(),
                "UnrelatedPreference",
                "keep",
            ),
        ];
        for ext in EXTENSIONS {
            preserved.extend([
                (format!("Software\\Classes\\{ext}"), "", "OtherApp.Document"),
                (format!("Software\\Classes\\{ext}\\OpenWithProgids"), "OtherApp.Document", ""),
                (format!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{ext}\\UserChoice"), "ProgId", "OtherApp.Document"),
                (format!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{ext}\\UserChoice"), "Hash", "untouched-test-hash"),
            ]);
        }
        for (path, name, value) in &preserved {
            registry.text(path, name, value).unwrap();
        }
        registry
            .register(Path::new(r"C:\应用\FeatherMD.exe"))
            .unwrap();
        for (path, name, value) in &preserved {
            assert_text(&registry, path, name, value);
        }
        registry.unregister().unwrap();
        registry.unregister().unwrap();
        for (path, name, value) in &preserved {
            assert_text(&registry, path, name, value);
        }
        assert!(registry.open(PROGID_PATH, KEY_READ).unwrap().is_none());
        assert!(registry.open(CAPABILITIES, KEY_READ).unwrap().is_none());
        assert_eq!(read(&registry, REGISTERED_APPLICATIONS, "FeatherMD"), None);
        for ext in EXTENSIONS {
            assert_eq!(
                read(
                    &registry,
                    &format!("Software\\Classes\\{ext}\\OpenWithProgids"),
                    PROGID
                ),
                None
            );
        }
    }

    #[test]
    fn association_invalid_executable_is_rejected_before_registry_writes() {
        let sandbox = Sandbox::new();
        let registry = sandbox.registry();
        for exe in ["C:\\broken\0path.exe", "C:\\bad\"path.exe"] {
            let error = registry.register(Path::new(exe)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
        assert!(registry.open("Software", KEY_READ).unwrap().is_none());
    }
}
