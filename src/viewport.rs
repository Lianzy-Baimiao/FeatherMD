//! Pixel-aware navigation over the already bounded layout, with no file I/O.
//! A partially visible final row is included and reused when paging forward.
pub trait RowHeight {
    fn height(&self) -> i32;
}
impl RowHeight for i32 {
    fn height(&self) -> i32 {
        *self
    }
}
pub struct Viewport<'a, R> {
    rows: &'a [R],
    pixels: i64,
}
impl<'a, R: RowHeight> Viewport<'a, R> {
    pub fn new(rows: &'a [R], pixels: i32) -> Self {
        Self {
            rows,
            pixels: i64::from(pixels.max(1)),
        }
    }
    fn height(&self, index: usize) -> i64 {
        i64::from(self.rows[index].height().max(1))
    }
    /// Exclusive end of rows actually intersecting the viewport.
    pub fn visible_end(&self, top: usize) -> usize {
        let mut end = top.min(self.rows.len());
        let mut used = 0;
        while end < self.rows.len() && used < self.pixels {
            used += self.height(end);
            end += 1;
        }
        end
    }
    /// Last row-aligned top that fills the viewport as much as possible at EOF.
    pub fn bottom_top(&self) -> usize {
        let mut top = self.rows.len().saturating_sub(1);
        let Some(_) = self.rows.get(top) else {
            return 0;
        };
        let mut used = self.height(top);
        while top > 0 && used + self.height(top - 1) <= self.pixels {
            top -= 1;
            used += self.height(top);
        }
        top
    }
    /// Keep one row of overlap where possible, always making progress.
    pub fn page_top(&self, top: usize, forward: bool) -> usize {
        let top = top.min(self.rows.len().saturating_sub(1));
        if self.rows.is_empty() {
            return 0;
        }
        if forward {
            self.visible_end(top).saturating_sub(1).max(top + 1)
        } else {
            let mut next = top;
            let mut used = self.height(top);
            while next > 0 && used + self.height(next - 1) <= self.pixels {
                next -= 1;
                used += self.height(next);
            }
            next.min(top.saturating_sub(1))
        }
    }
}
