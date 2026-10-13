//! Native anti-aliased vector chrome. No icon fonts, bitmaps or external assets.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

pub(super) const EXTERNAL: usize = 10003;
pub(super) const FEATHER: usize = 10000;
pub(super) const CHECK: usize = 10001;
pub(super) const RESTORE: usize = 10002;
pub(super) const PALETTE: usize = 10004;
pub(super) const BOOK: usize = 10005;
pub(super) const PENCIL: usize = 10006;
pub(super) const UPDATE: usize = 10007;

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
    /// Round offsets to whole device pixels so straight edges land on the grid.
    crisp: bool,
}
impl Canvas {
    unsafe fn new(
        dc: HDC,
        color: u32,
        width: f32,
        (x, y): (f32, f32),
        scale: f32,
        crisp: bool,
    ) -> Option<Self> {
        let mut c = Self {
            graphics: null_mut(),
            pen: null_mut(),
            brush: null_mut(),
            x,
            y,
            scale,
            crisp,
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
    /// Design units to device pixels. Symmetric rounding keeps mirrored glyphs mirrored.
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x * self.scale, y * self.scale);
        if self.crisp {
            (self.x + dx.round(), self.y + dy.round())
        } else {
            (self.x + dx, self.y + dy)
        }
    }
    unsafe fn line(&self, points: &[(f32, f32)]) {
        let points: Vec<_> = points
            .iter()
            .map(|&(x, y)| {
                let (x, y) = self.at(x, y);
                gp::PointF { X: x, Y: y }
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
        let p = p.map(|(x, y)| self.at(x, y));
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
        let (left, top) = self.at(x, y);
        let (right, bottom) = self.at(x + width, y + height);
        let (x, y, w, h) = (left, top, right - left, bottom - top);
        if fill {
            gp::GdipFillEllipse(self.graphics, self.brush.cast(), x, y, w, h);
        } else {
            gp::GdipDrawEllipse(self.graphics, self.pen, x, y, w, h);
        }
    }
    /// Open arc on a circle, in degrees clockwise from +x (GDI+ y points down).
    unsafe fn arc(&self, center: (f32, f32), radius: f32, start: f32, sweep: f32) {
        let (left, top) = self.at(center.0 - radius, center.1 - radius);
        let (right, bottom) = self.at(center.0 + radius, center.1 + radius);
        gp::GdipDrawArc(
            self.graphics,
            self.pen,
            left,
            top,
            right - left,
            bottom - top,
            start,
            sweep,
        );
    }
    unsafe fn rounded(&self, bounds: [f32; 4], radius: f32, fill: bool) {
        let [x, y, w, h] = bounds;
        let (left, top) = self.at(x, y);
        let (right, bottom) = self.at(x + w, y + h);
        let (x, y, w, h, r) = (left, top, right - left, bottom - top, radius * self.scale);
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
    if let Some(c) = Canvas::new(dc, color, 1.0, (r.left as f32, r.top as f32), 1.0, false) {
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

/// Stroke width in device pixels, snapped to half pixels with a 1 px floor.
/// Small icons stay legible instead of fading into a fractional blur.
fn stroke(weight: f32, scale: f32) -> f32 {
    ((weight * scale * 2.0).round() / 2.0).max(1.0)
}

/// Where a stroke centre sits so a horizontal/vertical line covers as many whole
/// pixel rows as possible. GDI+ high-quality offsets put pixel centres at x.5:
/// 1 or 1.5 px strokes centre on a half pixel, 2 or 2.5 px strokes on a whole one.
fn grid(value: f32, width: f32) -> f32 {
    if (width.floor() as i32) % 2 == 1 {
        value.floor() + 0.5
    } else {
        value.round()
    }
}

pub(super) unsafe fn icon(dc: HDC, r: RECT, id: usize, color: u32, scale: f32) {
    let scale = if id == FEATHER {
        (r.right - r.left).min(r.bottom - r.top) as f32 / 24.0
    } else {
        scale
    };
    let weight = stroke(
        if matches!(id, MINIMIZE | MAXIMIZE | RESTORE | EXIT) {
            1.2
        } else {
            1.55
        },
        scale,
    );
    // Rectangular glyphs snap every coordinate; curved ones only snap their centre.
    let crisp = matches!(
        id,
        MINIMIZE | MAXIMIZE | RESTORE | OUTLINE | THEME_SYSTEM | ZOOM_IN | ZOOM_OUT | OPEN
    );
    let center = (
        grid((r.left + r.right) as f32 / 2.0, weight),
        grid((r.top + r.bottom) as f32 / 2.0, weight),
    );
    let Some(c) = Canvas::new(dc, color, weight, center, scale, crisp) else {
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
        PALETTE => {
            // Round palette with a thumb notch on the right and three paint wells.
            c.arc((0.0, 0.0), 8.0, 40.0, 290.0);
            c.bezier([(6.13, 5.14), (2.5, 3.0), (3.0, -1.5), (6.93, -4.0)]);
            for (x, y) in [(-4.0, -1.0), (-0.8, -4.6), (-2.6, 3.4)] {
                c.ellipse(x - 1.3, y - 1.3, 2.6, 2.6, true);
            }
        }
        BOOK => {
            // Two open pages meeting at the spine.
            for side in [-1.0f32, 1.0] {
                c.bezier([
                    (0.0, -5.5),
                    (side * 2.5, -7.5),
                    (side * 6.5, -7.5),
                    (side * 9.0, -6.5),
                ]);
                c.line(&[(side * 9.0, -6.5), (side * 9.0, 6.5)]);
                c.bezier([
                    (side * 9.0, 6.5),
                    (side * 6.5, 5.5),
                    (side * 2.5, 5.5),
                    (0.0, 7.5),
                ]);
            }
            c.line(&[(0.0, -5.5), (0.0, 7.5)]);
        }
        UPDATE => {
            // Clockwise circular arrow: open circle, arrowhead pointing into the gap.
            c.arc((0.0, 0.0), 6.0, 60.0, 290.0);
            c.line(&[(3.81, -4.02), (5.91, -1.04), (8.69, -2.82)]);
        }
        PENCIL => {
            // A pencil at 45°: parallel body edges, tip, and the ferrule line.
            c.line(&[
                (-7.5, 7.5),
                (-6.5, 3.5),
                (4.5, -7.5),
                (7.5, -4.5),
                (-3.5, 6.5),
                (-7.5, 7.5),
            ]);
            c.line(&[(2.0, -5.0), (5.0, -2.0)]);
        }
        FEATHER => {
            c.bezier([(-6.0, 7.0), (-10.0, -4.0), (0.0, -10.0), (9.0, -9.0)]);
            c.bezier([(9.0, -9.0), (10.0, 1.0), (3.0, 10.0), (-6.0, 7.0)]);
            c.line(&[(-10.0, 11.0), (5.0, -5.0)]);
            c.line(&[(-2.0, 2.5), (3.0, 2.5)]);
        }
        _ => {}
    }
}
