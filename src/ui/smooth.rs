//! Native anti-aliased vector chrome. No icon fonts, bitmaps or external assets.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

pub(super) const EXTERNAL: usize = 10003;
pub(super) const FEATHER: usize = 10000;
pub(super) const CHECK: usize = 10001;
pub(super) const RESTORE: usize = 10002;

pub(super) struct Session(usize);
impl Session {
    pub(super) unsafe fn start() -> Result<Self, String> {
        let mut token = 0;
        let input = gp::GdiplusStartupInput {
            GdiplusVersion: 1,
            SuppressExternalCodecs: 1,
            ..zeroed()
        };
        let status = gp::GdiplusStartup(&mut token, &input, null_mut());
        if status == gp::Ok {
            Ok(Self(token))
        } else {
            Err(format!("无法初始化图形绘制：{status}"))
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            gp::GdiplusShutdown(self.0);
        }
    }
}

// A canvas owns all native resources; never keep it alive across GDI text drawing.
struct Canvas {
    graphics: *mut gp::GpGraphics,
    pen: *mut gp::GpPen,
    brush: *mut gp::GpSolidFill,
    x: f32,
    y: f32,
    scale: f32,
}
impl Canvas {
    unsafe fn new(dc: HDC, color: u32, width: f32, x: f32, y: f32, scale: f32) -> Option<Self> {
        let mut c = Self {
            graphics: null_mut(),
            pen: null_mut(),
            brush: null_mut(),
            x,
            y,
            scale,
        };
        let argb = 0xff000000 | ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255);
        if gp::GdipCreateFromHDC(dc, &mut c.graphics) != gp::Ok
            || gp::GdipCreatePen1(argb, width, gp::UnitPixel, &mut c.pen) != gp::Ok
            || gp::GdipCreateSolidFill(argb, &mut c.brush) != gp::Ok
        {
            return None;
        }
        gp::GdipSetSmoothingMode(c.graphics, gp::SmoothingModeAntiAlias8x8);
        gp::GdipSetPixelOffsetMode(c.graphics, gp::PixelOffsetModeHighQuality);
        gp::GdipSetPenStartCap(c.pen, gp::LineCapRound);
        gp::GdipSetPenEndCap(c.pen, gp::LineCapRound);
        gp::GdipSetPenLineJoin(c.pen, gp::LineJoinRound);
        Some(c)
    }
    unsafe fn line(&self, points: &[(f32, f32)]) {
        let points: Vec<_> = points
            .iter()
            .map(|&(x, y)| gp::PointF {
                X: self.x + x * self.scale,
                Y: self.y + y * self.scale,
            })
            .collect();
        if points.len() >= 2 {
            gp::GdipDrawLines(
                self.graphics,
                self.pen,
                points.as_ptr(),
                points.len() as i32,
            );
        }
    }
    unsafe fn bezier(&self, p: [(f32, f32); 4]) {
        let p = p.map(|(x, y)| (self.x + x * self.scale, self.y + y * self.scale));
        gp::GdipDrawBezier(
            self.graphics,
            self.pen,
            p[0].0,
            p[0].1,
            p[1].0,
            p[1].1,
            p[2].0,
            p[2].1,
            p[3].0,
            p[3].1,
        );
    }
    unsafe fn ellipse(&self, x: f32, y: f32, width: f32, height: f32, fill: bool) {
        let (x, y, w, h) = (
            self.x + x * self.scale,
            self.y + y * self.scale,
            width * self.scale,
            height * self.scale,
        );
        if fill {
            gp::GdipFillEllipse(self.graphics, self.brush.cast(), x, y, w, h);
        } else {
            gp::GdipDrawEllipse(self.graphics, self.pen, x, y, w, h);
        }
    }
    unsafe fn rounded(&self, bounds: [f32; 4], radius: f32, fill: bool) {
        let [x, y, w, h] = bounds;
        let (x, y, w, h, r) = (
            self.x + x * self.scale,
            self.y + y * self.scale,
            w * self.scale,
            h * self.scale,
            radius * self.scale,
        );
        let r = r.min(w / 2.0).min(h / 2.0).max(0.5);
        let mut path = null_mut();
        if gp::GdipCreatePath(gp::FillModeAlternate, &mut path) != gp::Ok {
            return;
        }
        for (x, y, start) in [
            (x, y, 180.0),
            (x + w - 2.0 * r, y, 270.0),
            (x + w - 2.0 * r, y + h - 2.0 * r, 0.0),
            (x, y + h - 2.0 * r, 90.0),
        ] {
            gp::GdipAddPathArc(path, x, y, 2.0 * r, 2.0 * r, start, 90.0);
        }
        gp::GdipClosePathFigure(path);
        if fill {
            gp::GdipFillPath(self.graphics, self.brush.cast(), path);
        } else {
            gp::GdipDrawPath(self.graphics, self.pen, path);
        }
        gp::GdipDeletePath(path);
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            if !self.brush.is_null() {
                gp::GdipDeleteBrush(self.brush.cast());
            }
            if !self.pen.is_null() {
                gp::GdipDeletePen(self.pen);
            }
            if !self.graphics.is_null() {
                gp::GdipDeleteGraphics(self.graphics);
            }
        }
    }
}

pub(super) unsafe fn round(dc: HDC, r: RECT, color: u32, radius: i32) {
    if r.right <= r.left || r.bottom <= r.top {
        return;
    }
    if let Some(c) = Canvas::new(dc, color, 1.0, r.left as f32, r.top as f32, 1.0) {
        c.rounded(
            [
                0.0,
                0.0,
                (r.right - r.left) as f32,
                (r.bottom - r.top) as f32,
            ],
            radius as f32,
            true,
        );
    }
}

pub(super) unsafe fn icon(dc: HDC, r: RECT, id: usize, color: u32, scale: f32) {
    let scale = if id == FEATHER {
        (r.right - r.left).min(r.bottom - r.top) as f32 / 24.0
    } else {
        scale
    };
    let weight = if matches!(id, MINIMIZE | MAXIMIZE | RESTORE | EXIT) {
        1.2
    } else {
        1.55
    };
    let Some(c) = Canvas::new(
        dc,
        color,
        weight * scale,
        (r.left + r.right) as f32 / 2.0,
        (r.top + r.bottom) as f32 / 2.0,
        scale,
    ) else {
        return;
    };
    match id {
        PREVIEW => {
            c.bezier([(-9.0, 0.0), (-5.0, -8.0), (5.0, -8.0), (9.0, 0.0)]);
            c.bezier([(-9.0, 0.0), (-5.0, 8.0), (5.0, 8.0), (9.0, 0.0)]);
            c.ellipse(-2.5, -2.5, 5.0, 5.0, false);
        }
        SOURCE => {
            c.line(&[(-5.0, -5.0), (-10.0, 0.0), (-5.0, 5.0)]);
            c.line(&[(5.0, -5.0), (10.0, 0.0), (5.0, 5.0)]);
            c.line(&[(2.0, -7.0), (-2.0, 7.0)]);
        }
        OPEN => {
            c.line(&[(-9.0, 5.0), (-9.0, -5.0)]);
            c.bezier([(-9.0, -5.0), (-9.0, -6.5), (-8.5, -7.0), (-7.0, -7.0)]);
            c.line(&[(-7.0, -7.0), (-3.0, -7.0), (0.0, -4.0), (6.0, -4.0)]);
            c.bezier([(6.0, -4.0), (7.5, -4.0), (8.0, -3.5), (8.0, -2.0)]);
            c.line(&[
                (-9.0, 6.0),
                (-6.0, -1.0),
                (10.0, -1.0),
                (7.0, 6.0),
                (-9.0, 6.0),
            ]);
        }
        FIND => {
            c.ellipse(-7.0, -7.0, 11.0, 11.0, false);
            c.line(&[(2.5, 2.5), (8.0, 8.0)]);
        }
        THEME_LIGHT | THEME_SEPIA => {
            c.ellipse(-4.0, -4.0, 8.0, 8.0, false);
            for (x, y) in [
                (1.0, 0.0),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (0.707, 0.707),
                (-0.707, 0.707),
                (0.707, -0.707),
                (-0.707, -0.707),
            ] {
                c.line(&[(x * 7.0, y * 7.0), (x * 9.0, y * 9.0)]);
            }
        }
        THEME_DARK => {
            c.bezier([(-1.5, -8.0), (-11.0, -7.0), (-10.0, 8.0), (0.0, 8.0)]);
            c.bezier([(0.0, 8.0), (4.0, 8.0), (7.0, 5.5), (8.0, 2.5)]);
            c.bezier([(8.0, 2.5), (-1.0, 5.5), (-5.0, -2.0), (-1.5, -8.0)]);
        }
        THEME_SYSTEM => {
            c.rounded([-8.0, -7.0, 16.0, 12.0], 1.7, false);
            c.line(&[(0.0, 5.0), (0.0, 8.0)]);
            c.line(&[(-4.0, 8.0), (4.0, 8.0)]);
        }
        THEME_PICKER => c.line(&[(-2.5, -1.0), (0.0, 1.5), (2.5, -1.0)]),
        SETTINGS => {
            for x in [-6.0, 0.0, 6.0] {
                c.ellipse(x - 1.2, -1.2, 2.4, 2.4, true);
            }
        }
        MINIMIZE | ZOOM_OUT | ZOOM_IN => {
            c.line(&[(-5.0, 0.0), (5.0, 0.0)]);
            if id == ZOOM_IN {
                c.line(&[(0.0, -5.0), (0.0, 5.0)]);
            }
        }
        MAXIMIZE => c.rounded([-5.0, -5.0, 10.0, 10.0], 0.8, false),
        RESTORE => {
            c.line(&[
                (-2.0, -3.0),
                (-2.0, -6.0),
                (6.0, -6.0),
                (6.0, 2.0),
                (4.0, 2.0),
            ]);
            c.rounded([-5.0, -3.0, 9.0, 9.0], 0.8, false);
        }
        EXIT | 0 => {
            c.line(&[(-4.5, -4.5), (4.5, 4.5)]);
            c.line(&[(-4.5, 4.5), (4.5, -4.5)]);
        }
        OUTLINE => {
            c.rounded([-7.0, -6.0, 14.0, 12.0], 1.5, false);
            c.line(&[(-2.0, -6.0), (-2.0, 6.0)]);
        }
        EXTERNAL => {
            c.line(&[(-5.0, 5.0), (5.0, -5.0), (-4.0, -5.0)]);
            c.line(&[(5.0, -5.0), (5.0, 4.0)]);
        }
        CHECK => c.line(&[(-5.0, 0.0), (-1.5, 3.5), (5.5, -4.0)]),
        FEATHER => {
            c.bezier([(-6.0, 7.0), (-10.0, -4.0), (0.0, -10.0), (9.0, -9.0)]);
            c.bezier([(9.0, -9.0), (10.0, 1.0), (3.0, 10.0), (-6.0, 7.0)]);
            c.line(&[(-10.0, 11.0), (5.0, -5.0)]);
            c.line(&[(-2.0, 2.5), (3.0, 2.5)]);
        }
        _ => {}
    }
}
