//! Preview character selection. A position is a block (its first source byte) and a
//! displayed UTF-16 unit inside it, so a selection survives relayout, resizing and
//! images arriving. A table row counts its displayed cells joined by tabs, so a
//! selection runs through the cells in reading order and copies tab-separated text;
//! the pointer only ever lands in the cell under it. An image is one unit and copies
//! its alternative text. Hit testing measures text with the run's font through `pens`.
use crate::layout::{Row, Run};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Spot {
    pub(super) block: u64,
    pub(super) unit: usize,
}

/// First displayed unit after `row`; a table row ends after its last cell.
pub(super) fn row_end(row: &Row) -> usize {
    match &row.table {
        Some(table) => table.text.len(),
        None => row.unit + row.runs.iter().map(|r| r.text.len()).sum::<usize>(),
    }
}

/// The runs of `row` with the block unit each starts at. Text rows count on from
/// `row.unit`; a table line's runs record where they sit in the row's cells.
pub(super) fn runs(row: &Row) -> impl Iterator<Item = (usize, &Run)> {
    let mut unit = row.unit;
    row.runs.iter().enumerate().map(move |(i, run)| {
        let first = match &row.table {
            Some(table) => table.starts.get(i).copied().unwrap_or(unit),
            None => unit,
        };
        unit = first + run.text.len();
        (first, run)
    })
}

/// Units `[from, to)` of cell `column` of a table row, without the tabs between cells.
fn cell(row: &Row, column: usize) -> (usize, usize) {
    let Some(table) = &row.table else {
        return (row.unit, row_end(row));
    };
    let from = table.cells.get(column).copied().unwrap_or(table.text.len());
    let to = table
        .cells
        .get(column + 1)
        .map_or(table.text.len(), |&next| next.saturating_sub(1));
    (
        from.min(table.text.len()),
        to.max(from).min(table.text.len()),
    )
}

/// Units `[a, b)` of `row` (relative to its block) inside the selection `from..to`,
/// or None when the row is outside it. Every visual line of a table row covers the
/// same units of the row; each run paints its own part. An empty row inside the
/// selection is selected with nothing to paint (a blank line to copy).
pub(super) fn covered(row: &Row, from: Spot, to: Spot) -> Option<(usize, usize)> {
    if from >= to || row.start < from.block || row.start > to.block {
        return None;
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
/// `Run::x`). `pens(run)` gives the pen position after each unit of a text run. In a
/// table only the cell under `x` counts, so a drag never jumps into the next column;
/// left of the table is the row's start, right of it the row's end, and a cell with
/// nothing on this visual line gives its end.
pub(super) fn nearest(row: &Row, x: i32, mut pens: impl FnMut(&Run) -> Vec<i32>) -> usize {
    let (only, fallback) = match &row.table {
        Some(table) => {
            let (Some(&left), Some(&right)) = (table.edges.first(), table.edges.last()) else {
                return 0;
            };
            if x < left {
                return 0;
            }
            if x >= right {
                return table.text.len();
            }
            let column = table
                .edges
                .partition_point(|&edge| edge <= x)
                .saturating_sub(1);
            let (from, to) = cell(row, column);
            (Some((from, to)), to)
        }
        None => (None, row.unit),
    };
    let mut best = (i32::MAX, fallback);
    let mut consider = |unit: usize, at: i32| {
        let distance = (at - x).abs();
        if distance < best.0 {
            best = (distance, unit);
        }
    };
    for (unit, run) in runs(row) {
        if only.is_some_and(|(from, to)| unit < from || unit + run.text.len() > to) {
            continue;
        }
        consider(unit, run.x);
        if atomic(run) || run.text.is_empty() {
            consider(unit + run.text.len(), run.x + run.width);
        } else {
            for (i, pen) in pens(run).into_iter().enumerate() {
                consider(unit + i + 1, run.x + pen);
            }
        }
    }
    best.1
}

/// Pen position (like `Run::x`) of the boundary before `unit` of `row`; the inverse
/// of `nearest` for the units this visual line shows.
pub(super) fn x_of(row: &Row, unit: usize, mut pens: impl FnMut(&Run) -> Vec<i32>) -> i32 {
    let count = row.runs.len();
    let table = row.table.is_some();
    for (i, (first, run)) in runs(row).enumerate() {
        let last = first + run.text.len();
        // A table run holds its own end; text runs hand a shared boundary to the next.
        let here = if table {
            (first..=last).contains(&unit)
        } else {
            unit < last
        };
        if here || (!table && i + 1 == count) {
            let k = unit.clamp(first, last) - first;
            return match k {
                0 => run.x,
                _ if atomic(run) || k == run.text.len() => run.x + run.width,
                _ => run.x + pens(run).get(k - 1).copied().unwrap_or(run.width),
            };
        }
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
/// selected cells once, tab-separated (a whole row its source cells), an image its
/// alternative text.
pub(super) fn text(rows: &[Row], from: Spot, to: Spot) -> String {
    let mut out = String::new();
    // Block and end unit of the last row copied.
    let mut last: Option<(u64, usize)> = None;
    for row in rows {
        let Some((a, b)) = covered(row, from, to) else {
            continue;
        };
        if let Some(line) = &row.table {
            // Every visual line of a table row covers the same units: copy them once.
            if last.is_some_and(|(block, _)| block == row.start) {
                continue;
            }
            if last.is_some() {
                out.push_str("\r\n");
            }
            if a == 0 && b == line.text.len() {
                out.push_str(&line.copy);
            } else {
                out.push_str(&String::from_utf16_lossy(&line.text[a..b]));
            }
            last = Some((row.start, b));
            continue;
        }
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

/// The word (or space run, or symbol run) of `block` around `unit`, as displayed. In
/// a table it stays inside the cell holding `unit`.
pub(super) fn word(rows: &[Row], block: u64, unit: usize) -> (Spot, Spot) {
    let spot = |unit: usize| Spot { block, unit };
    if let Some((row, line)) = rows
        .iter()
        .filter(|r| r.start == block)
        .find_map(|r| r.table.as_ref().map(|t| (r, t)))
    {
        // A tab between cells belongs to the cell before it.
        let column = line
            .cells
            .partition_point(|&start| start <= unit)
            .saturating_sub(1);
        let (from, to) = cell(row, column);
        let (a, b) = word_in(&line.text[from..to], unit.clamp(from, to) - from);
        return (spot(from + a), spot(from + b));
    }
    // The block's displayed units; spaces removed at line breaks come back.
    let mut units: Vec<u16> = Vec::new();
    for row in rows.iter().filter(|r| r.start == block) {
        units.resize(units.len().max(row.unit), 32);
        units.truncate(row.unit);
        for run in &row.runs {
            units.extend_from_slice(&run.text);
        }
    }
    let (a, b) = word_in(&units, unit);
    (spot(a), spot(b))
}

/// Units `[a, b)` of the word, space run or symbol run of `units` around `unit`. The
/// end of the text belongs to its last character, so a double-click on the right
/// half of a cell's or paragraph's last character still selects its word.
fn word_in(units: &[u16], unit: usize) -> (usize, usize) {
    let text = String::from_utf16_lossy(units);
    let mut byte = text.len();
    let mut used = 0;
    for (i, c) in text.char_indices() {
        if used >= unit {
            byte = i;
            break;
        }
        used += c.len_utf16();
    }
    if byte == text.len() {
        byte = text.char_indices().next_back().map_or(0, |(i, _)| i);
    }
    let (a, b) = feathermd::text::word_at(&text, byte);
    let units_to = |end: usize| text[..end].chars().map(char::len_utf16).sum::<usize>();
    (units_to(a), units_to(b))
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

    /// One visual line of a table row of `cells`; `runs` are (unit in the row, text, x).
    fn table_line(start: u64, cells: &[&str], edges: &[i32], runs: &[(usize, &str, i32)]) -> Row {
        let mut text: Vec<u16> = Vec::new();
        let mut starts_of = Vec::new();
        for cell in cells {
            if !starts_of.is_empty() {
                text.push(u16::from(b'\t'));
            }
            starts_of.push(text.len());
            text.extend(cell.encode_utf16());
        }
        let mut line = row(start, 0, runs.iter().map(|&(_, t, x)| run(t, x)).collect());
        line.kind = feathermd::markdown::Kind::Table;
        line.table = Some(TableLine {
            edges: Rc::from(edges),
            header: false,
            first: true,
            close: true,
            stripe: false,
            copy: Rc::from(cells.join("\t")),
            text: text.into(),
            cells: starts_of.into(),
            starts: runs.iter().map(|&(unit, _, _)| unit).collect(),
        });
        line
    }

    /// "alpha beta gamma" wrapped after "beta" (the space is removed), then
    /// "second" as its own block, a blank line and a table row.
    fn page() -> Vec<Row> {
        vec![
            row(0, 0, vec![run("alpha ", 0), run("beta", 60)]),
            row(0, 11, vec![run("gamma", 0)]),
            row(17, 0, vec![run("second", 0)]),
            row(24, 0, vec![]),
            table_line(30, &["a", "1"], &[0, 40, 80], &[(0, "a", 0), (2, "1", 50)]),
        ]
    }

    /// Two table rows in columns 0..60 and 60..120 (text inset by 5). The first
    /// row's "ab cd" wraps after "ab" (the space is removed); "xy" fits.
    fn table() -> Vec<Row> {
        let (cells, edges) = (["ab cd", "xy"], [0, 60, 120]);
        vec![
            table_line(50, &cells, &edges, &[(0, "ab", 5), (6, "xy", 65)]),
            table_line(50, &cells, &edges, &[(3, "cd", 5)]),
            table_line(70, &["e", "f"], &edges, &[(0, "e", 5), (2, "f", 65)]),
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
            text(&rows, spot(0, 12), spot(30, 3)),
            "amma\r\nsecond\r\n\r\na\t1"
        );
        // Ending where the table row starts leaves the row out.
        assert_eq!(text(&rows, spot(0, 12), spot(30, 0)), "amma\r\nsecond\r\n");
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
        // The end of a block belongs to its last word.
        assert_eq!(word(&rows, 17, 6), (spot(17, 0), spot(17, 6)));
        assert_eq!(word(&rows, 0, 16), (spot(0, 11), spot(0, 16)));
    }

    #[test]
    fn table_cells_select_by_character_in_reading_order() {
        let rows = table();
        // Inside one cell, across its wrap: the removed space comes back.
        assert_eq!(text(&rows, spot(50, 1), spot(50, 4)), "b c");
        // From one cell into the next: a tab between them.
        assert_eq!(text(&rows, spot(50, 3), spot(50, 7)), "cd\tx");
        // Into the next row: CRLF; whole rows copy their source cells.
        assert_eq!(text(&rows, spot(50, 6), spot(70, 1)), "xy\r\ne");
        assert_eq!(text(&rows, spot(50, 0), spot(70, 3)), "ab cd\txy\r\ne\tf");
        // Each visual line paints only its own selected characters.
        let (a, b) = covered(&rows[0], spot(50, 1), spot(50, 7)).expect("covered");
        assert_eq!((a, b), (1, 7));
        assert_eq!(covered(&rows[1], spot(50, 1), spot(50, 7)), Some((1, 7)));
        let painted = |row: &Row| -> Vec<(i32, i32)> {
            runs(row)
                .filter_map(|(first, run)| extent(run, first, (a, b), pens))
                .collect()
        };
        assert_eq!(painted(&rows[0]), vec![(15, 25), (65, 75)]);
        assert_eq!(painted(&rows[1]), vec![(5, 25)]);
        // A selection ending where a row starts leaves that row out.
        assert_eq!(covered(&rows[2], spot(50, 0), spot(70, 0)), None);
        assert_eq!(row_end(&rows[1]), 8);
    }

    #[test]
    fn table_hit_testing_stays_in_the_cell_under_the_pointer() {
        let rows = table();
        // The first line: "ab" in the first column, "xy" in the second.
        assert_eq!(nearest(&rows[0], 16, pens), 1);
        assert_eq!(nearest(&rows[0], 50, pens), 2);
        assert_eq!(nearest(&rows[0], 62, pens), 6);
        assert_eq!(nearest(&rows[0], 100, pens), 8);
        // Left of the table is the row's start, right of it the row's end.
        assert_eq!(nearest(&rows[0], -5, pens), 0);
        assert_eq!(nearest(&rows[0], 120, pens), 8);
        // The wrapped line holds "cd"; a cell with nothing on a line gives its end.
        assert_eq!(nearest(&rows[1], 0, pens), 3);
        assert_eq!(nearest(&rows[1], 90, pens), 8);
        for (row, units) in [
            (&rows[0], vec![0, 1, 2, 6, 7, 8]),
            (&rows[1], vec![3, 4, 5]),
        ] {
            for unit in units {
                assert_eq!(nearest(row, x_of(row, unit, pens), pens), unit);
            }
        }
    }

    /// The real layout of a wrapped table, measured by GDI in a memory DC (no window):
    /// each run is its slice of the row's selection text inside one cell, every unit
    /// but tabs and spaces removed at wraps is drawn once, and hit testing returns
    /// each boundary it is given.
    #[test]
    fn laid_out_table_lines_match_their_selection_text() {
        use crate::layout::{Fonts, Layout, Picture, View};
        use feathermd::{document::SourceLine, markdown, settings::Settings};
        use windows_sys::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC};
        let source = [
            "| 快捷键 | 说明 |",
            "| --- | --- |",
            "| `Ctrl+F` | 在整份文档中后台查找，区分大小写；按 F3 跳到下一处，Esc 取消。 |",
            "| a | wrapped words keep going across several visual lines here |",
        ];
        let mut start = 0;
        let lines: Vec<SourceLine> = source
            .iter()
            .map(|line| {
                let end = start + line.len() as u64;
                let out = SourceLine {
                    start,
                    end,
                    text: (*line).to_owned(),
                    continued: false,
                };
                start = end + 2;
                out
            })
            .collect();
        let blocks = markdown::blocks(&lines, false);
        let settings = Settings::default();
        let pictures = |_: &markdown::Image| Picture::Off;
        let view = View {
            settings: &settings,
            pictures: &pictures,
            max_height: 600,
        };
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            assert!(!dc.is_null());
            let mut fonts = Fonts::with_family(96, "Microsoft YaHei UI");
            let layout = Layout::build(dc, &blocks, &view, &mut fonts, 260);
            let table: Vec<&Row> = layout.rows.iter().filter(|r| r.table.is_some()).collect();
            let mut blocks_seen: Vec<u64> = Vec::new();
            for row in &table {
                let line = row.table.as_ref().expect("table line");
                for (first, run) in runs(row) {
                    assert_eq!(line.text[first..first + run.text.len()], run.text[..]);
                    assert!(!run.text.contains(&u16::from(b'\t')));
                    for unit in first..=first + run.text.len() {
                        let x = x_of(row, unit, |r| super::super::pens(dc, &mut fonts, r));
                        let hit = nearest(row, x, |r| super::super::pens(dc, &mut fonts, r));
                        assert_eq!(hit, unit, "row {} x {x}", row.start);
                    }
                }
                if !blocks_seen.contains(&row.start) {
                    blocks_seen.push(row.start);
                }
            }
            DeleteDC(dc);
            // The header and two body rows; the narrow second column wraps both.
            assert_eq!(blocks_seen.len(), 3);
            for &block in &blocks_seen[1..] {
                let count = table.iter().filter(|r| r.start == block).count();
                assert!(count >= 2, "row {block} has {count} visual lines");
            }
            for &block in &blocks_seen {
                let lines: Vec<&&Row> = table.iter().filter(|r| r.start == block).collect();
                let line = lines[0].table.as_ref().expect("table line");
                let mut shown = vec![0; line.text.len()];
                for row in &lines {
                    for (first, run) in runs(row) {
                        for count in &mut shown[first..first + run.text.len()] {
                            *count += 1;
                        }
                    }
                }
                for (unit, &count) in shown.iter().enumerate() {
                    if matches!(line.text[unit], 9 | 32) {
                        assert!(count <= 1, "unit {unit} of row {block} drawn {count} times");
                    } else {
                        assert_eq!(count, 1, "unit {unit} of row {block}");
                    }
                }
            }
            // Part of a wrapped row copies its text with the spaces removed at wraps.
            let last = *blocks_seen.last().expect("rows");
            let end = table
                .last()
                .and_then(|r| r.table.as_ref())
                .map_or(0, |t| t.text.len());
            assert_eq!(
                text(&layout.rows, spot(last, 2), spot(last, end)),
                "wrapped words keep going across several visual lines here"
            );
        }
    }

    #[test]
    fn double_click_in_a_table_stays_in_its_cell() {
        let rows = table();
        assert_eq!(word(&rows, 50, 1), (spot(50, 0), spot(50, 2)));
        assert_eq!(word(&rows, 50, 4), (spot(50, 3), spot(50, 5)));
        // The end of a cell, and the tab after it, belong to the cell's last word.
        assert_eq!(word(&rows, 50, 5), (spot(50, 3), spot(50, 5)));
        assert_eq!(word(&rows, 50, 8), (spot(50, 6), spot(50, 8)));
        assert_eq!(word(&rows, 70, 2), (spot(70, 2), spot(70, 3)));
    }
}
