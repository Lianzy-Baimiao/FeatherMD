//! Preview character selection. A position is a block (its first source byte) and a
//! displayed UTF-16 unit inside it, so a selection survives relayout, resizing and
//! images arriving. Tables select whole rows; an image is one unit and copies its
//! alternative text. Hit testing measures text with the run's font through `pens`.
use crate::layout::{Row, Run};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Spot {
    pub(super) block: u64,
    pub(super) unit: usize,
}

/// First displayed unit after `row`.
pub(super) fn row_end(row: &Row) -> usize {
    row.unit + row.runs.iter().map(|r| r.text.len()).sum::<usize>()
}

/// Units `[a, b)` of `row` (relative to its block) inside the selection `from..to`,
/// or None when the row is outside it. Table rows are all or nothing; an empty row
/// inside the selection is selected with nothing to paint (a blank line to copy).
pub(super) fn covered(row: &Row, from: Spot, to: Spot) -> Option<(usize, usize)> {
    if from >= to || row.start < from.block || row.start > to.block {
        return None;
    }
    if row.table.is_some() {
        return Some((0, 0));
    }
    let end = row_end(row);
    let a = if row.start == from.block {
        from.unit
    } else {
        0
    }
    .max(row.unit);
    let b = if row.start == to.block {
        to.unit
    } else {
        usize::MAX
    }
    .min(end);
    if a < b {
        Some((a, b))
    } else if end == row.unit && row.start > from.block && row.start < to.block {
        Some((row.unit, row.unit))
    } else {
        None
    }
}

/// A run's selectable units: images, task boxes and disclosure marks are one piece.
fn atomic(run: &Run) -> bool {
    run.picture.is_some() || run.check.is_some() || run.disclosure
}

/// The unit boundary of `row` nearest to `x` (relative to the text origin, like
/// `Run::x`). `pens(run)` gives the pen position after each unit of a text run.
pub(super) fn nearest(row: &Row, x: i32, mut pens: impl FnMut(&Run) -> Vec<i32>) -> usize {
    let mut best = (i32::MAX, row.unit);
    let mut consider = |unit: usize, at: i32| {
        let distance = (at - x).abs();
        if distance < best.0 {
            best = (distance, unit);
        }
    };
    let mut unit = row.unit;
    for run in &row.runs {
        consider(unit, run.x);
        if atomic(run) || run.text.is_empty() {
            consider(unit + run.text.len(), run.x + run.width);
        } else {
            for (i, pen) in pens(run).into_iter().enumerate() {
                consider(unit + i + 1, run.x + pen);
            }
        }
        unit += run.text.len();
    }
    best.1
}

/// Pen position (like `Run::x`) of the boundary before `unit` of `row`; the inverse
/// of `nearest`.
pub(super) fn x_of(row: &Row, unit: usize, mut pens: impl FnMut(&Run) -> Vec<i32>) -> i32 {
    let mut first = row.unit;
    for (i, run) in row.runs.iter().enumerate() {
        let last = first + run.text.len();
        if unit < last || i + 1 == row.runs.len() {
            let k = unit.clamp(first, last) - first;
            return match k {
                0 => run.x,
                _ if atomic(run) || k == run.text.len() => run.x + run.width,
                _ => run.x + pens(run).get(k - 1).copied().unwrap_or(run.width),
            };
        }
        first = last;
    }
    0
}

/// Horizontal extent of the selected units `[a, b)` of `run`, which starts at unit
/// `first`; None when they do not meet. `pens` as in `nearest`.
pub(super) fn extent(
    run: &Run,
    first: usize,
    (a, b): (usize, usize),
    pens: impl FnOnce(&Run) -> Vec<i32>,
) -> Option<(i32, i32)> {
    let last = first + run.text.len();
    let (from, to) = (a.max(first), b.min(last));
    if from >= to {
        return None;
    }
    if atomic(run) || (from == first && to == last) {
        return Some((run.x, run.x + run.width));
    }
    let pens = pens(run);
    let at = |k: usize| {
        if k == 0 {
            0
        } else {
            pens.get(k - 1).copied().unwrap_or(run.width)
        }
    };
    Some((run.x + at(from - first), run.x + at(to - first)))
}

/// Copy text of the selection over `rows`: rows of one block join (restoring the
/// spaces a line break removed), blocks are separated by CRLF, a table row gives its
/// tab-separated cells once, an image its alternative text.
pub(super) fn text(rows: &[Row], from: Spot, to: Spot) -> String {
    let mut out = String::new();
    // Block and end unit of the last row copied.
    let mut last: Option<(u64, usize)> = None;
    let mut table: Option<u64> = None;
    for row in rows {
        let Some((a, b)) = covered(row, from, to) else {
            continue;
        };
        if let Some(line) = &row.table {
            if table != Some(row.start) {
                if last.is_some() {
                    out.push_str("\r\n");
                }
                table = Some(row.start);
                out.push_str(&line.copy);
                last = Some((row.start, 0));
            }
            continue;
        }
        table = None;
        match last {
            Some((block, end)) if block == row.start => {
                out.extend(std::iter::repeat_n(' ', a.saturating_sub(end)));
            }
            Some(_) => out.push_str("\r\n"),
            None => {}
        }
        let mut unit = row.unit;
        for run in &row.runs {
            let (from, to) = (a.max(unit), b.min(unit + run.text.len()));
            if from < to {
                match &run.picture {
                    Some(picture) => out.push_str(&picture.alt),
                    None => {
                        out.push_str(&String::from_utf16_lossy(&run.text[from - unit..to - unit]))
                    }
                }
            }
            unit += run.text.len();
        }
        last = Some((row.start, b));
    }
    out
}

/// The word (or space run, or symbol run) of `block` around `unit`, as displayed.
pub(super) fn word(rows: &[Row], block: u64, unit: usize) -> (Spot, Spot) {
    // The block's displayed units; spaces removed at line breaks come back.
    let mut units: Vec<u16> = Vec::new();
    for row in rows
        .iter()
        .filter(|r| r.start == block && r.table.is_none())
    {
        units.resize(units.len().max(row.unit), 32);
        units.truncate(row.unit);
        for run in &row.runs {
            units.extend_from_slice(&run.text);
        }
    }
    let text = String::from_utf16_lossy(&units);
    let mut byte = text.len();
    let mut used = 0;
    for (i, c) in text.char_indices() {
        if used >= unit {
            byte = i;
            break;
        }
        used += c.len_utf16();
    }
    let (a, b) = feathermd::text::word_at(&text, byte);
    let units_to = |end: usize| text[..end].chars().map(char::len_utf16).sum::<usize>();
    (
        Spot {
            block,
            unit: units_to(a),
        },
        Spot {
            block,
            unit: units_to(b),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Inline, TableLine};
    use std::rc::Rc;

    fn run(text: &str, x: i32) -> Run {
        let text: Vec<u16> = text.encode_utf16().collect();
        Run {
            width: text.len() as i32 * 10,
            text,
            x,
            ..Run::default()
        }
    }
    fn row(start: u64, unit: usize, runs: Vec<Run>) -> Row {
        Row {
            start,
            end: start + 100,
            runs,
            height: 20,
            kind: feathermd::markdown::Kind::Paragraph,
            unit,
            table: None,
            baseline: 15,
            first: unit == 0,
            last: true,
            indent: 0,
            bar: None,
            boxed: false,
        }
    }
    fn spot(block: u64, unit: usize) -> Spot {
        Spot { block, unit }
    }
    fn pens(run: &Run) -> Vec<i32> {
        (1..=run.text.len() as i32).map(|i| i * 10).collect()
    }

    /// "alpha beta gamma" wrapped after "beta" (the space is removed), then
    /// "second" as its own block, a blank line and a table row.
    fn page() -> Vec<Row> {
        let mut table = row(30, 0, vec![run("a", 0), run("1", 50)]);
        table.table = Some(TableLine {
            edges: Rc::from([0, 40, 80]),
            header: false,
            first: true,
            close: true,
            stripe: false,
            copy: Rc::from("a\t1"),
        });
        vec![
            row(0, 0, vec![run("alpha ", 0), run("beta", 60)]),
            row(0, 11, vec![run("gamma", 0)]),
            row(17, 0, vec![run("second", 0)]),
            row(24, 0, vec![]),
            table,
        ]
    }

    #[test]
    fn copies_across_wrapped_rows_blocks_blank_lines_and_tables() {
        let rows = page();
        // Inside one row.
        assert_eq!(text(&rows, spot(0, 2), spot(0, 5)), "pha");
        // Across the wrap: the removed space comes back.
        assert_eq!(text(&rows, spot(0, 6), spot(0, 14)), "beta gam");
        // Across blocks, a blank line and a whole table row.
        assert_eq!(
            text(&rows, spot(0, 12), spot(30, 0)),
            "amma\r\nsecond\r\n\r\na\t1"
        );
        // Callers order the ends; unordered or empty selections copy nothing.
        assert_eq!(text(&rows, spot(0, 14), spot(0, 6)), "");
        assert_eq!(text(&rows, spot(17, 3), spot(17, 3)), "");
    }

    #[test]
    fn covered_units_and_painted_extents() {
        let rows = page();
        assert_eq!(covered(&rows[0], spot(0, 2), spot(0, 14)), Some((2, 10)));
        assert_eq!(covered(&rows[1], spot(0, 2), spot(0, 14)), Some((11, 14)));
        assert_eq!(covered(&rows[2], spot(0, 2), spot(0, 14)), None);
        assert_eq!(covered(&rows[3], spot(0, 0), spot(30, 0)), Some((0, 0)));
        // "alpha " is units 0..6; selecting 2..10 paints from "p" to its end, and
        // all of "beta".
        assert_eq!(extent(&rows[0].runs[0], 0, (2, 10), pens), Some((20, 60)));
        assert_eq!(extent(&rows[0].runs[1], 6, (2, 10), pens), Some((60, 100)));
        assert_eq!(extent(&rows[0].runs[1], 6, (0, 6), pens), None);
    }

    #[test]
    fn hit_testing_finds_the_nearest_boundary_and_images_are_one_unit() {
        let rows = page();
        assert_eq!(nearest(&rows[0], 23, pens), 2);
        assert_eq!(nearest(&rows[0], 27, pens), 3);
        assert_eq!(nearest(&rows[0], -40, pens), 0);
        assert_eq!(nearest(&rows[0], 500, pens), 10);
        assert_eq!(nearest(&rows[1], 0, pens), 11);
        for unit in 0..=10 {
            assert_eq!(nearest(&rows[0], x_of(&rows[0], unit, pens), pens), unit);
        }
        // An image between two words is one unit, copied as its alternative text.
        let mut picture = run("\u{fffc}", 30);
        picture.width = 100;
        picture.picture = Some(Inline {
            key: Rc::from(r"C:\a.png"),
            top: 0,
            height: 50,
            alt: "Logo".into(),
        });
        let line = row(40, 0, vec![run("ab ", 0), picture, run(" cd", 130)]);
        assert_eq!(nearest(&line, 60, pens), 3);
        assert_eq!(nearest(&line, 100, pens), 4);
        assert_eq!(
            text(std::slice::from_ref(&line), spot(40, 0), spot(40, 7)),
            "ab Logo cd"
        );
    }

    #[test]
    fn double_click_selects_a_word_across_a_wrap() {
        let rows = page();
        assert_eq!(word(&rows, 0, 7), (spot(0, 6), spot(0, 10)));
        assert_eq!(word(&rows, 0, 12), (spot(0, 11), spot(0, 16)));
        // The space removed at the wrap is a word of its own.
        assert_eq!(word(&rows, 0, 10), (spot(0, 10), spot(0, 11)));
        assert_eq!(word(&rows, 17, 0), (spot(17, 0), spot(17, 6)));
    }
}
