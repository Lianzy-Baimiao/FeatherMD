//! Opt-in, thread-local test containment. No global hooks or desktop input.
//! Confine test windows before they can be shown, and veto foreground activation.
use super::*;
use std::cell::RefCell;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

const ENV: &str = "FEATHERMD_TEST_SECONDARY_MONITOR";
const SUBCLASS: usize = 0x464d5448;

#[derive(Default)]
struct State {
    area: Option<RECT>,
    windows: usize,
    placements: usize,
    blocked_activation: usize,
    violations: usize,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(super) struct Host(HHOOK);

pub(super) unsafe fn key_pressed(key: i32) -> bool {
    // Synthetic secondary-monitor messages carry no physical modifiers. Do not
    // sample the user's game keyboard or change the system keyboard state.
    !STATE.with(|s| s.borrow().area.is_some()) && unsafe { GetKeyState(key) < 0 }
}

impl Host {
    pub(super) unsafe fn start() -> Result<Self, String> {
        if std::env::var_os(ENV).is_none() {
            return Ok(Self(null_mut()));
        }
        std::hint::black_box("FEATHERMD_SECONDARY_TEST_V1");
        if std::env::var_os("FEATHERMD_TEST_SNAPSHOT").is_none() {
            return Err("Secondary-monitor tests require a snapshot output path".into());
        }
        let mut areas = Vec::<RECT>::new();
        if unsafe {
            EnumDisplayMonitors(
                null_mut(),
                null(),
                Some(monitor),
                &mut areas as *mut _ as isize,
            )
        } == 0
            || areas.len() != 1
        {
            return Err(
                "Expected exactly one non-primary monitor; no test window was opened".into(),
            );
        }
        STATE.with(|state| state.borrow_mut().area = Some(areas[0]));
        let hook =
            unsafe { SetWindowsHookExW(WH_CBT, Some(cbt), null_mut(), GetCurrentThreadId()) };
        if hook.is_null() {
            return Err("Cannot contain test dialogs; no test window was opened".into());
        }
        Ok(Self(hook))
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            UnhookWindowsHookEx(self.0);
        }
        if let Some(path) = std::env::var_os("FEATHERMD_TEST_SNAPSHOT") {
            use std::io::Write;
            let report = STATE.with(|state| {
                let s = state.borrow();
                let a = s.area.unwrap_or_default();
                format!("pid={} area={},{},{},{} windows={} placements={} blocked_activation={} violations={}\n",
                    std::process::id(), a.left, a.top, a.right, a.bottom,
                    s.windows, s.placements, s.blocked_activation, s.violations)
            });
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(PathBuf::from(path).with_file_name("test-host.txt"))
            {
                let _ = file.write_all(report.as_bytes());
            }
        }
    }
}

unsafe extern "system" fn monitor(h: HMONITOR, _: HDC, _: *mut RECT, data: isize) -> i32 {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(h, &mut info) } != 0 && info.dwFlags & MONITORINFOF_PRIMARY == 0 {
        unsafe {
            (&mut *(data as *mut Vec<RECT>)).push(info.rcWork);
        }
    }
    1
}

fn confined(area: RECT, x: i32, y: i32, width: i32, height: i32) -> RECT {
    let width = width.clamp(1, (area.right - area.left).max(1));
    let height = height.clamp(1, (area.bottom - area.top).max(1));
    let left = x.clamp(area.left, area.right - width);
    let top = y.clamp(area.top, area.bottom - height);
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

unsafe extern "system" fn cbt(code: i32, wp: usize, lp: isize) -> isize {
    if code == HCBT_ACTIVATE as i32 || code == HCBT_SETFOCUS as i32 && wp == 0 {
        STATE.with(|s| s.borrow_mut().blocked_activation += 1);
        return 1;
    }
    if code == HCBT_CREATEWND as i32 {
        let create = unsafe { &mut *(*(lp as *mut CBT_CREATEWNDW)).lpcs };
        if create.style as u32 & WS_CHILD == 0 {
            let area = STATE.with(|s| s.borrow().area).unwrap();
            let width = if create.cx <= 0 { 960 } else { create.cx };
            let height = if create.cy <= 0 { 640 } else { create.cy };
            let rect = confined(area, area.left + 32, area.top + 32, width, height);
            create.x = rect.left;
            create.y = rect.top;
            create.cx = rect.right - rect.left;
            create.cy = rect.bottom - rect.top;
            create.dwExStyle |= WS_EX_NOACTIVATE;
            // A failed subclass must prevent creation, not leave an uncontained window.
            if unsafe { SetWindowSubclass(wp as HWND, Some(window), SUBCLASS, 0) } == 0 {
                return 1;
            }
            STATE.with(|s| s.borrow_mut().windows += 1);
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wp, lp) }
}

unsafe extern "system" fn window(
    hwnd: HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    _: usize,
    _: usize,
) -> isize {
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as isize;
    }
    if msg == WM_WINDOWPOSCHANGING {
        let area = STATE.with(|s| s.borrow().area).unwrap();
        let pos = unsafe { &mut *(lp as *mut WINDOWPOS) };
        let mut old = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut old);
        }
        let (x, y) = if pos.flags & SWP_NOMOVE != 0 {
            (old.left, old.top)
        } else {
            (pos.x, pos.y)
        };
        let (width, height) = if pos.flags & SWP_NOSIZE != 0 {
            (old.right - old.left, old.bottom - old.top)
        } else {
            (pos.cx, pos.cy)
        };
        let r = confined(area, x, y, width, height);
        pos.x = r.left;
        pos.y = r.top;
        pos.cx = r.right - r.left;
        pos.cy = r.bottom - r.top;
        if pos.x != old.left || pos.y != old.top {
            pos.flags &= !SWP_NOMOVE;
        }
        if pos.cx != old.right - old.left || pos.cy != old.bottom - old.top {
            pos.flags &= !SWP_NOSIZE;
        }
        pos.flags |= SWP_NOACTIVATE;
    }
    let result = unsafe { DefSubclassProc(hwnd, msg, wp, lp) };
    if msg == WM_WINDOWPOSCHANGED && unsafe { IsWindowVisible(hwnd) } != 0 {
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut rect);
        }
        let mut foreground_pid = 0;
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), &mut foreground_pid);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let area = s.area.unwrap();
            s.placements += 1;
            if rect.left < area.left
                || rect.top < area.top
                || rect.right > area.right
                || rect.bottom > area.bottom
                || foreground_pid == std::process::id()
            {
                s.violations += 1;
            }
        });
    }
    if msg == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(window), SUBCLASS);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_never_crosses_the_secondary_work_area() {
        for left in [-2048, 3072] {
            let area = RECT {
                left,
                top: -400,
                right: left + 2048,
                bottom: 880,
            };
            for (x, y, width, height) in [
                (0, 0, 720, 552),
                (i32::MIN, i32::MIN, 960, 640),
                (i32::MAX, i32::MAX, 5000, 4000),
                (0, 0, 0, 0),
            ] {
                let r = confined(area, x, y, width, height);
                assert!(r.left >= area.left && r.top >= area.top);
                assert!(r.right <= area.right && r.bottom <= area.bottom);
                assert!(r.right > r.left && r.bottom > r.top);
            }
        }
    }
}
