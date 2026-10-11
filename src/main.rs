#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod association;
#[cfg(windows)]
mod layout;
#[cfg(windows)]
mod ui;
fn main() {
    #[cfg(windows)]
    {
        let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
        if let Err(error) = ui::run(path) {
            let text: Vec<u16> = format!("FeatherMD 启动失败：{error}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    text.as_ptr(),
                    std::ptr::null(),
                    0x10,
                );
            }
        }
    }
    #[cfg(not(windows))]
    eprintln!(
        "FeatherMD's native GUI currently requires Windows 10/11. The reader core is cross-platform."
    );
}
