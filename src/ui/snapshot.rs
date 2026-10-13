//! Opt-in test capture: HDCs stay in-process; hidden cross-process PrintWindow is unreliable.
use super::*;

pub(super) unsafe fn capture(hwnd: HWND, draw: impl FnOnce(HDC)) -> bool {
    let Some(path) = std::env::var_os("FEATHERMD_TEST_SNAPSHOT") else {
        return false;
    };
    let mut client = RECT::default();
    GetClientRect(hwnd, &mut client);
    let width = client.right;
    let height = client.bottom;
    if width <= 0 || height <= 0 {
        return false;
    }
    let screen = GetDC(hwnd);
    let dc = CreateCompatibleDC(screen);
    let bitmap = CreateCompatibleBitmap(screen, width, height);
    let old = SelectObject(dc, bitmap);
    draw(dc);
    SelectObject(dc, old);
    let mut info: BITMAPINFO = zeroed();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let read = GetDIBits(
        screen,
        bitmap,
        0,
        height as u32,
        pixels.as_mut_ptr().cast(),
        &mut info,
        DIB_RGB_COLORS,
    );
    DeleteObject(bitmap);
    DeleteDC(dc);
    ReleaseDC(hwnd, screen);
    if read == 0 {
        return false;
    }
    let mut out = Vec::with_capacity(pixels.len() + 54);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((pixels.len() + 54) as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&(-height).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    out.extend_from_slice(&pixels);
    std::fs::write(path, out).is_ok()
}

// Test diagnostics for translated text that does not fit. Painted text is checked
// where it is drawn (`clip`); STATIC controls are measured on request. Nothing is
// recorded unless FEATHERMD_TEST_SNAPSHOT is set.
thread_local! {
    static CLIPPED: std::cell::RefCell<Vec<(usize, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn testing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("FEATHERMD_TEST_SNAPSHOT").is_some())
}
/// Forget what `owner` recorded; call before it paints again.
pub(super) fn clear_clips(owner: HWND) {
    if testing() {
        CLIPPED.with(|c| c.borrow_mut().retain(|(h, _)| *h != owner as usize));
    }
}
/// Records `text` when drawing it into `rect` with `flags` would cut it off. The
/// font must already be selected; ellipsis text shortens on purpose and is skipped.
pub(super) unsafe fn clip(owner: HWND, dc: HDC, text: &[u16], rect: RECT, flags: u32) {
    if !testing() || flags & (DT_END_ELLIPSIS | DT_PATH_ELLIPSIS | DT_WORD_ELLIPSIS) != 0 {
        return;
    }
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let mut need = RECT {
        right: width,
        ..RECT::default()
    };
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    DrawTextW(
        dc,
        text.as_ptr(),
        len as i32,
        &mut need,
        (flags & (DT_SINGLELINE | DT_WORDBREAK | DT_NOPREFIX)) | DT_CALCRECT,
    );
    if need.right > width || (flags & DT_WORDBREAK != 0 && need.bottom > height) {
        let text = String::from_utf16_lossy(&text[..len]);
        CLIPPED.with(|c| c.borrow_mut().push((owner as usize, text)));
    }
}
/// Visible STATIC children of `parent` whose text does not fit, measured with the
/// control's own font. Owner-drawn buttons report through `clip` when they paint.
unsafe fn overflowing(parent: HWND) -> Vec<String> {
    let mut out = Vec::new();
    let dc = GetDC(parent);
    let mut child = GetWindow(parent, GW_CHILD);
    while !child.is_null() {
        let style = GetWindowLongPtrW(child, GWL_STYLE) as u32;
        let mut class = [0u16; 16];
        let len = GetClassNameW(child, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let length = GetWindowTextLengthW(child);
        // Ellipsis statics (SS_*ELLIPSIS) shorten long paths on purpose.
        if style & WS_VISIBLE != 0
            && style & 0xc000 == 0
            && length > 0
            && String::from_utf16_lossy(&class[..len]).eq_ignore_ascii_case("Static")
        {
            let mut text = vec![0u16; length as usize + 1];
            GetWindowTextW(child, text.as_mut_ptr(), text.len() as i32);
            let mut client = RECT::default();
            GetClientRect(child, &mut client);
            let font = SendMessageW(child, WM_GETFONT, 0, 0) as HGDIOBJ;
            let old = SelectObject(dc, font);
            // Simple and no-wrap statics draw one line; others wrap at words.
            let single = matches!(style & 0x1f, 0xb | 0xc);
            let prefix = if style & 0x80 != 0 { DT_NOPREFIX } else { 0 };
            let mut r = RECT {
                right: client.right,
                ..RECT::default()
            };
            DrawTextW(
                dc,
                text.as_ptr(),
                -1,
                &mut r,
                DT_CALCRECT | prefix | if single { DT_SINGLELINE } else { DT_WORDBREAK },
            );
            SelectObject(dc, old);
            if r.right > client.right || r.bottom > client.bottom {
                out.push(String::from_utf16_lossy(&text[..length as usize]));
            }
        }
        child = GetWindow(child, GW_HWNDNEXT);
    }
    ReleaseDC(parent, dc);
    out
}
/// Diagnostics: text of `window` that does not fit (STATIC children plus text
/// clipped in its last paint), written one per line to labels.txt beside the
/// snapshot. Returns the count, or -1 outside tests.
pub(super) unsafe fn report(window: HWND) -> isize {
    let Some(path) = std::env::var_os("FEATHERMD_TEST_SNAPSHOT") else {
        return -1;
    };
    let mut all = overflowing(window);
    CLIPPED.with(|c| {
        all.extend(
            c.borrow()
                .iter()
                .filter(|(h, _)| *h == window as usize)
                .map(|(_, t)| t.clone()),
        )
    });
    let path = std::path::PathBuf::from(path).with_file_name("labels.txt");
    let _ = std::fs::write(path, all.join("\r\n"));
    all.len() as isize
}
