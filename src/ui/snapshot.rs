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
