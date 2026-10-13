//! GitHub-style pipe tables: cell splitting, delimiter rows, column fitting and
//! cell line breaking. Pure functions; measuring and drawing live in the GUI.

/// Wider rows are not treated as tables, which bounds layout work per line.
pub const MAX_COLUMNS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// A `|` that separates cells: GFM treats `\|` as part of the cell text.
pub fn has_pipe(line: &str) -> bool {
    let bytes = line.as_bytes();
    (0..bytes.len()).any(|i| bytes[i] == b'|' && (i == 0 || bytes[i - 1] != b'\\'))
}

/// Cells of one row, trimmed; a leading and a trailing pipe are optional.
/// `\|` becomes a literal `|`; other escapes are left for inline parsing.
pub fn split_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut open = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            cell.push('|');
            open = true;
        } else if c == '|' {
            cells.push(cell.trim().to_owned());
            cell.clear();
            open = false;
        } else {
            cell.push(c);
            open = true;
        }
    }
    if open || cells.is_empty() {
        cells.push(cell.trim().to_owned());
    }
    cells
}

/// Column alignments when `line` is a delimiter row such as `| :-- | :-: | --: |`.
pub fn delimiter(line: &str) -> Option<Vec<Align>> {
    if !has_pipe(line) {
        return None;
    }
    let cells = split_row(line);
    if cells.len() > MAX_COLUMNS {
        return None;
    }
    cells
        .iter()
        .map(|cell| {
            let left = cell.starts_with(':');
            let right = cell.len() > 1 && cell.ends_with(':');
            let inner = &cell[left as usize..cell.len() - right as usize];
            (!inner.is_empty() && inner.bytes().all(|b| b == b'-')).then_some(match (left, right) {
                (true, true) => Align::Center,
                (false, true) => Align::Right,
                _ => Align::Left,
            })
        })
        .collect()
}

/// Column widths for `available` pixels. `natural` is each column's unwrapped width
/// and `minimum` the narrowest it should become (both including padding). Columns
/// keep their natural width when the table fits. Otherwise the widest columns are
/// narrowed first, to a common width, so short columns never wrap to save a few
/// pixels; no column goes below its minimum. When even the minimums do not fit, the
/// minimums are returned and the table scrolls.
pub fn fit_columns(natural: &[i32], minimum: &[i32], available: i32) -> Vec<i32> {
    let natural: Vec<i32> = natural.iter().map(|&n| n.max(0)).collect();
    let floor: Vec<i32> = natural
        .iter()
        .zip(minimum)
        .map(|(&n, &m)| m.clamp(0, n))
        .collect();
    let at = |cap: i32| -> Vec<i32> {
        natural
            .iter()
            .zip(&floor)
            .map(|(&n, &f)| cap.clamp(f, n))
            .collect()
    };
    let sum = |cap: i32| at(cap).iter().map(|&w| i64::from(w)).sum::<i64>();
    let widest = natural.iter().copied().max().unwrap_or(0);
    if sum(widest) <= i64::from(available) {
        return natural;
    }
    if sum(0) >= i64::from(available) {
        return floor;
    }
    // Largest common cap that fits; sum(0) fits and sum(widest) does not.
    let (mut lo, mut hi) = (0, widest);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if sum(mid) <= i64::from(available) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let mut widths = at(lo);
    // Fewer pixels remain than columns sitting at the cap; give them one each.
    let mut left = available - widths.iter().sum::<i32>();
    for (index, width) in widths.iter_mut().enumerate() {
        if left == 0 {
            break;
        }
        if *width == lo && lo >= floor[index] && lo < natural[index] {
            *width += 1;
            left -= 1;
        }
    }
    widths
}

fn high(unit: u16) -> bool {
    (0xd800..0xdc00).contains(&unit)
}

fn wide(unit: u16) -> bool {
    matches!(
        unit,
        0x1100..=0x115f
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe30..=0xfe4f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
    )
}

/// Closing punctuation that should not start a line.
fn closing(unit: u16) -> bool {
    matches!(
        char::from_u32(u32::from(unit)),
        Some(
            '，' | '。'
                | '、'
                | '；'
                | '：'
                | '？'
                | '！'
                | '）'
                | '】'
                | '》'
                | '」'
                | '』'
                | '〉'
                | '〕'
                | '］'
                | '｝'
                | '…'
                | '—'
                | '·'
                | '”'
                | '’'
                | ','
                | '.'
                | ';'
                | ':'
                | '?'
                | '!'
                | ')'
                | ']'
                | '}'
        )
    )
}
/// Opening punctuation that should not end a line.
fn opening(unit: u16) -> bool {
    matches!(
        char::from_u32(u32::from(unit)),
        Some(
            '（' | '【'
                | '《'
                | '「'
                | '『'
                | '〈'
                | '〔'
                | '［'
                | '｛'
                | '“'
                | '‘'
                | '('
                | '['
                | '{'
        )
    )
}
/// A line may break before `units[i]`: after a space, or next to a CJK character,
/// but not before closing or after opening punctuation.
fn breakable(units: &[u16], i: usize) -> bool {
    if i == 0 || i >= units.len() || high(units[i - 1]) {
        return false;
    }
    let (before, after) = (units[i - 1], units[i]);
    after != 32
        && !closing(after)
        && !opening(before)
        && (before == 32 || wide(before) || wide(after))
}

/// Greedy cell line breaking over UTF-16 `units`; `ends[i]` is the pen position
/// after unit `i`. Returns `[start, end)` ranges with breaking spaces removed. A word
/// wider than `width` is split; a surrogate pair is never split.
pub fn break_lines(units: &[u16], ends: &[i32], width: i32) -> Vec<(usize, usize)> {
    let at = |i: usize| if i == 0 { 0 } else { ends[i - 1] };
    let mut lines = Vec::new();
    let mut start = 0;
    while start < units.len() && units[start] == 32 {
        start += 1;
    }
    while start < units.len() {
        let origin = at(start);
        let mut fit = start;
        while fit < units.len() && ends[fit] - origin <= width {
            fit += 1;
        }
        let mut end = if fit == units.len() {
            fit
        } else {
            (start + 1..=fit)
                .rev()
                .find(|&i| breakable(units, i))
                .unwrap_or(fit)
        };
        if end == start {
            end = start + 1;
        }
        // Only a forced split can land inside a surrogate pair: keep the pair whole.
        if end < units.len() && high(units[end - 1]) {
            if end - 1 > start {
                end -= 1;
            } else {
                end += 1;
            }
        }
        let mut trimmed = end;
        while trimmed > start && units[trimmed - 1] == 32 {
            trimmed -= 1;
        }
        lines.push((start, trimmed));
        start = end;
        while start < units.len() && units[start] == 32 {
            start += 1;
        }
    }
    lines
}

/// Width of the widest piece `break_lines` keeps together when it can.
pub fn widest_token(units: &[u16], ends: &[i32]) -> i32 {
    let at = |i: usize| if i == 0 { 0 } else { ends[i - 1] };
    let mut widest = 0;
    let mut start = 0;
    for i in 1..=units.len() {
        if i == units.len() || breakable(units, i) {
            let (mut a, mut b) = (start, i);
            while a < b && units[a] == 32 {
                a += 1;
            }
            while b > a && units[b - 1] == 32 {
                b -= 1;
            }
            if b > a {
                widest = widest.max(at(b) - at(a));
            }
            start = i;
        }
    }
    widest
}
