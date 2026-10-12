//! Test-only stand-in "editor" for scripts/editor-smoke.ps1. It records the
//! working directory and arguments it received, then exits without a window.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    let Some(output) = std::env::var_os("FEATHERMD_EDITOR_PROBE") else {
        return;
    };
    let output = std::path::PathBuf::from(output);
    let mut text = String::new();
    if let Ok(dir) = std::env::current_dir() {
        text.push_str(&format!("cwd={}\n", dir.display()));
    }
    for arg in std::env::args_os().skip(1) {
        text.push_str(&format!("arg={}\n", arg.to_string_lossy()));
    }
    // Write then rename, so the test never reads a partially written file.
    let temp = output.with_extension("partial");
    if std::fs::write(&temp, text).is_ok() {
        let _ = std::fs::rename(&temp, &output);
    }
}
