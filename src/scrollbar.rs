//! Bounded scrollbar geometry. Values can be pixels or 64-bit file offsets.
#[derive(Clone, Copy, Debug)]
pub struct ScrollTrack {
    pub start: i32,
    pub length: i32,
    pub thumb_start: i32,
    pub thumb_length: i32,
    pub maximum: u64,
}

impl ScrollTrack {
    pub fn new(
        start: i32,
        length: i32,
        maximum: u64,
        visible: u64,
        position: u64,
        minimum_thumb: i32,
    ) -> Self {
        let length = length.max(1);
        let total = u128::from(maximum) + u128::from(visible.max(1));
        let thumb_length = ((length as u128 * u128::from(visible.max(1))) / total) as i32;
        let thumb_length = thumb_length.clamp(minimum_thumb.clamp(1, length), length);
        let travel = length - thumb_length;
        let offset = if maximum == 0 {
            0
        } else {
            (u128::from(position.min(maximum)) * travel as u128 / u128::from(maximum)) as i32
        };
        Self {
            start,
            length,
            thumb_start: start + offset,
            thumb_length,
            maximum,
        }
    }

    pub fn value_at(self, pointer: i32, grab: i32) -> u64 {
        let travel = self.length - self.thumb_length;
        if travel <= 0 {
            return 0;
        }
        let offset = (i64::from(pointer) - i64::from(grab) - i64::from(self.start))
            .clamp(0, i64::from(travel));
        (offset as u128 * u128::from(self.maximum) / travel as u128) as u64
    }
}
