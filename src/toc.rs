//! The outline as a tree: entry depths from heading levels and section numbers
//! (`1`, `1.2`, `1.2.3`), the sidebar's folds and filter, and links inside a
//! document: `#fragment` links and contents lists whose items name headings.
//! Pure functions over the bounded outline; no I/O.
use crate::{
    markdown::{Block, Kind},
    outline::Heading,
};
use std::collections::{HashMap, HashSet};

/// A leading section number of one to eight parts of at most three digits (`1`,
/// `1.2`, `1.2.3`) followed by a space, `.`, `、`, or, after two or more parts,
/// directly by a title in another script (`1.1权限对象`). Years (`2024 年`),
/// sizes (`3D`), times (`1:30`) and bare numbers are not section numbers.
pub fn section(text: &str) -> Option<Vec<u16>> {
    let mut parts: Vec<u16> = Vec::new();
    let mut rest = text.trim_start();
    loop {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 3 || parts.len() == 8 {
            return None;
        }
        parts.push(rest[..digits].parse().ok()?);
        rest = &rest[digits..];
        match rest.strip_prefix(['.', '．']) {
            Some(next) if next.starts_with(|c: char| c.is_ascii_digit()) => rest = next,
            _ => break,
        }
    }
    let separated = rest.starts_with(['.', '．', '、', ' ', '\t', '\u{3000}']);
    let titled = rest
        .chars()
        .next()
        .is_some_and(|c| !c.is_ascii() && !c.is_whitespace());
    (separated || (parts.len() > 1 && titled)).then_some(parts)
}

/// `parent` numbers a section that holds `child` (`1.2` holds `1.2.3`).
fn holds_number(parent: &[u16], child: &[u16]) -> bool {
    parent.len() < child.len() && child.starts_with(parent)
}

/// A heading that only names the table of contents: `目录`, `目 录`, `目次`,
/// `Contents`, `Table of Contents`, `TOC`.
pub fn contents_title(text: &str) -> bool {
    let name: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ':' | '：'))
        .flat_map(char::to_lowercase)
        .collect();
    matches!(name.as_str(), "contents" | "tableofcontents" | "toc")
        || [['目', '录'], ['目', '錄'], ['目', '次']]
            .iter()
            .any(|word| name.chars().eq(word.iter().copied()))
}

/// Sets every entry's `depth`, from 1. A deeper heading level nests, and skipped
/// levels add nothing. At one level a longer section number nests under a shorter
/// one (`#### 1.1` under `#### 1`) when the document numbers that level, and an
/// unnumbered heading there belongs to the numbered heading before it. A contents
/// title takes the depth of the entry after it, so it never holds the document.
/// True when some level numbers its own hierarchy.
pub fn assign_depths(entries: &mut [Heading]) -> bool {
    let numbers: Vec<Option<Vec<u16>>> = entries.iter().map(|e| section(&e.text)).collect();
    let numbered = numbered_levels(entries, &numbers);
    // Open ancestors of the next entry: (heading level, section number).
    let mut open: Vec<(u8, Option<&[u16]>)> = Vec::new();
    // Contents titles waiting for the entry after them, with their own depth.
    let mut titles: Vec<(usize, usize)> = Vec::new();
    for i in 0..entries.len() {
        let (level, number) = (entries[i].level, numbers[i].as_deref());
        let holds = |&(l, n): &(u8, Option<&[u16]>)| {
            l < level
                || (l == level
                    && numbered[usize::from(level.min(6))]
                    && n.is_some_and(|n| number.is_none_or(|m| holds_number(n, m))))
        };
        if contents_title(&entries[i].text) {
            let depth = open.iter().take_while(|&o| holds(o)).count() + 1;
            titles.push((i, depth));
            continue;
        }
        while open.last().is_some_and(|o| !holds(o)) {
            open.pop();
        }
        let depth = open.len() + 1;
        for (title, own) in titles.drain(..) {
            entries[title].depth = depth.max(own).min(255) as u8;
        }
        entries[i].depth = depth.min(255) as u8;
        open.push((level, number));
    }
    for (title, own) in titles {
        entries[title].depth = own.min(255) as u8;
    }
    numbered.contains(&true)
}

/// Headings of a page sized by their depth in the outline instead of their `#`
/// count, for a document whose headings number their hierarchy at one level (all
/// `####` with `1`, `1.1`, `1.1.1`): depth 1 reads as `#`, depth 2 as `##`, and so
/// on to `######`. Headings outside the outline keep theirs.
pub fn levels(blocks: &mut [Block], entries: &[Heading]) {
    for block in blocks.iter_mut() {
        if let Kind::Heading(level) = &mut block.kind {
            let i = entries.partition_point(|e| e.offset < block.start);
            if let Some(entry) = entries.get(i).filter(|e| e.offset == block.start) {
                *level = entry.depth.clamp(1, 6);
            }
        }
    }
}

/// Heading levels whose section numbers nest: a number there extends one seen
/// before at the same level (`1.1` after `1`).
fn numbered_levels(entries: &[Heading], numbers: &[Option<Vec<u16>>]) -> [bool; 7] {
    let mut seen: [HashSet<&[u16]>; 7] = Default::default();
    let mut out = [false; 7];
    for (entry, number) in entries.iter().zip(numbers) {
        let Some(number) = number else {
            continue;
        };
        let level = usize::from(entry.level.min(6));
        if number.len() > 1 && seen[level].contains(&number[..number.len() - 1]) {
            out[level] = true;
        }
        seen[level].insert(number.as_slice());
    }
    out
}

/// Entry `index` has entries under it.
pub fn has_children(entries: &[Heading], index: usize) -> bool {
    entries
        .get(index + 1)
        .is_some_and(|next| entries.get(index).is_some_and(|e| next.depth > e.depth))
}

/// The deepest depth an outline opens unfolded to: as deep as keeps at most
/// `budget` entries listed, at least the top level.
pub fn fold_depth(entries: &[Heading], budget: usize) -> u8 {
    let mut count = [0usize; 256];
    for entry in entries {
        count[usize::from(entry.depth)] += 1;
    }
    let mut total = 0;
    let mut deepest = 1;
    for (depth, &n) in count.iter().enumerate().skip(1) {
        total += n;
        if total > budget {
            break;
        }
        if n > 0 {
            deepest = depth;
        }
    }
    deepest as u8
}

/// Folds of an outline that just opened: entries at the `fold_depth` or deeper
/// that hold others, so opening one shows its children folded in turn.
pub fn initial_folds(entries: &[Heading], budget: usize) -> Vec<bool> {
    let depth = fold_depth(entries, budget);
    (0..entries.len())
        .map(|i| entries[i].depth >= depth && has_children(entries, i))
        .collect()
}

/// One row of the sidebar: an outline entry, and whether a filter lists it only as
/// the parent of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub entry: usize,
    pub context: bool,
}

/// The rows the sidebar lists. Without filter words: every entry not inside a folded
/// one. With words: the entries whose lower-case label (`lower`) contains all of
/// them, each after the parents it needs, folds aside.
pub fn rows(entries: &[Heading], folded: &[bool], lower: &[String], words: &[String]) -> Vec<Line> {
    let mut out = Vec::new();
    if words.is_empty() {
        // Entries deeper than this are inside a folded one.
        let mut hidden_below = u8::MAX;
        for (entry, heading) in entries.iter().enumerate() {
            if heading.depth > hidden_below {
                continue;
            }
            hidden_below = if folded.get(entry).copied().unwrap_or(false) {
                heading.depth
            } else {
                u8::MAX
            };
            out.push(Line {
                entry,
                context: false,
            });
        }
        return out;
    }
    // Parents of the current entry, and whether each is listed already.
    let mut path: Vec<(usize, bool)> = Vec::new();
    for (entry, heading) in entries.iter().enumerate() {
        while path
            .last()
            .is_some_and(|&(i, _)| entries[i].depth >= heading.depth)
        {
            path.pop();
        }
        let label = lower.get(entry).map_or("", String::as_str);
        let matched = words.iter().all(|w| label.contains(w.as_str()));
        if matched {
            for (parent, listed) in path.iter_mut() {
                if !*listed {
                    *listed = true;
                    out.push(Line {
                        entry: *parent,
                        context: true,
                    });
                }
            }
            out.push(Line {
                entry,
                context: false,
            });
        }
        path.push((entry, matched));
    }
    out
}

/// The row that shows entry `active`: its own, or that of the folded parent it is
/// inside. None when no listed row stands for it.
pub fn active_row(entries: &[Heading], rows: &[Line], active: usize) -> Option<usize> {
    let row = rows.partition_point(|r| r.entry <= active).checked_sub(1)?;
    let shown = rows[row].entry;
    if shown == active {
        return Some(row);
    }
    // A parent holds `active` when nothing between them is as shallow as it.
    let depth = entries.get(shown)?.depth;
    entries
        .get(shown + 1..=active)?
        .iter()
        .all(|e| e.depth > depth)
        .then_some(row)
}

/// GitHub's heading anchor: lower case, spaces to hyphens, punctuation dropped
/// (letters and digits of every script kept).
pub fn slug(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// `text` with runs of white space as one space and none at the ends: how a
/// heading and a contents item naming it are compared.
fn label(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What links inside a Markdown document resolve to: each entry's anchor (repeated
/// headings numbered `-1`, `-2` as on GitHub) and the entries by label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    anchors: Vec<String>,
    by_label: HashMap<String, Vec<usize>>,
}
impl Contents {
    pub fn new(entries: &[Heading]) -> Self {
        let mut used: HashMap<String, usize> = HashMap::new();
        let mut anchors = Vec::with_capacity(entries.len());
        let mut by_label: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            let base = slug(&entry.text);
            let anchor = match used.get_mut(&base) {
                Some(n) => {
                    *n += 1;
                    format!("{base}-{n}")
                }
                None => {
                    used.insert(base.clone(), 0);
                    base
                }
            };
            anchors.push(anchor);
            let key = label(&entry.text);
            if !key.is_empty() {
                by_label.entry(key).or_default().push(index);
            }
        }
        Self { anchors, by_label }
    }
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }
    /// The anchor `#…` of entry `index`.
    pub fn anchor(&self, index: usize) -> Option<&str> {
        self.anchors.get(index).map(String::as_str)
    }
    /// The entry a `#fragment` link points to: by anchor (as written, lower case or
    /// made into one), then by the heading's own text.
    pub fn resolve(&self, fragment: &str) -> Option<usize> {
        let wanted = decode(fragment.strip_prefix('#').unwrap_or(fragment));
        if wanted.is_empty() {
            return None;
        }
        let lower = wanted.to_lowercase();
        let made = slug(&wanted);
        for candidate in [wanted.as_str(), lower.as_str(), made.as_str()] {
            if let Some(i) = self.anchors.iter().position(|a| a == candidate) {
                return Some(i);
            }
        }
        self.by_label
            .get(&label(&wanted))
            .and_then(|found| found.first().copied())
    }
    /// The entry a contents item titled `text` names, preferring the first after
    /// `offset` (a repeated title names the next heading of that name).
    fn target(&self, text: &str, entries: &[Heading], offset: u64) -> Option<usize> {
        let key = label(text);
        if key.is_empty() {
            return None;
        }
        // Converted contents often end with leader dots and a page number.
        let found = self
            .by_label
            .get(&key)
            .or_else(|| self.by_label.get(strip_page(&key)))?;
        found
            .iter()
            .copied()
            .find(|&i| entries.get(i).is_some_and(|e| e.offset > offset))
            .or_else(|| found.first().copied())
    }
}

/// `%E7%9B%AE` and other percent escapes of a link fragment as text.
fn decode(fragment: &str) -> String {
    let bytes = fragment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = fragment.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `Title ........ 12` as `Title`; text without a trailing page number unchanged.
fn strip_page(text: &str) -> &str {
    let number = text.trim_end_matches(|c: char| c.is_ascii_digit());
    if number.len() == text.len() {
        return text;
    }
    number
        .trim_end_matches([' ', '\t', '.', '…', '·', '．', '\u{3000}'])
        .trim_end()
}

/// Bullets of nested list items, as `markdown` draws them.
const BULLETS: [&str; 3] = ["• ", "◦ ", "▪ "];

/// Links the items of contents lists on a preview page to the headings they name,
/// and indents a flat list by the depth of those headings. A contents list is a run
/// of at least three list items, mostly titles of outline entries, with nothing but
/// blank lines between them. Items that hold a link already are left alone.
pub fn link(blocks: &mut [Block], entries: &[Heading], contents: &Contents) {
    if contents.is_empty() || entries.is_empty() {
        return;
    }
    let mut i = 0;
    while i < blocks.len() {
        if !item(&blocks[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < blocks.len() && (item(&blocks[i]) || blank(&blocks[i])) {
            i += 1;
        }
        let items: Vec<usize> = (start..i).filter(|&k| item(&blocks[k])).collect();
        let targets: Vec<Option<usize>> = items
            .iter()
            .map(|&k| target_of(&blocks[k], entries, contents))
            .collect();
        let found = targets.iter().flatten().count();
        if found < 3 || found * 5 < items.len() * 3 {
            continue;
        }
        let flat = items
            .iter()
            .all(|&k| blocks[k].indent == blocks[items[0]].indent);
        let base = list_depth(entries, blocks[start].start);
        for (&k, &target) in items.iter().zip(&targets) {
            let Some((entry, anchor)) = target.and_then(|e| contents.anchor(e).map(|a| (e, a)))
            else {
                continue;
            };
            let block = &mut blocks[k];
            let link = format!("#{anchor}");
            for span in block.spans.iter_mut().skip(1) {
                span.link = Some(link.clone());
            }
            if flat {
                let nest = usize::from(entries[entry].depth)
                    .saturating_sub(base)
                    .min(7);
                block.indent += nest as i32 * 24;
                if let Some(marker) = block.spans.first_mut()
                    && BULLETS.contains(&marker.text.as_str())
                {
                    marker.text = BULLETS[nest % 3].into();
                }
            }
        }
    }
}

/// A list item line (its first span is the marker).
fn item(block: &Block) -> bool {
    block.marker && block.table.is_none() && matches!(block.kind, Kind::Paragraph | Kind::Quote)
}
fn blank(block: &Block) -> bool {
    block.kind == Kind::Paragraph && block.spans.is_empty()
}

/// The entry list item `block` names, unless it holds a link or an image.
fn target_of(block: &Block, entries: &[Heading], contents: &Contents) -> Option<usize> {
    let spans = block.spans.get(1..)?;
    if spans.is_empty() || spans.iter().any(|s| s.link.is_some() || s.image.is_some()) {
        return None;
    }
    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
    contents.target(&text, entries, block.start)
}

/// Depth of the entries a contents list at `offset` starts with: that of the
/// contents title above it, or one below another heading above it.
fn list_depth(entries: &[Heading], offset: u64) -> usize {
    let above = entries.partition_point(|e| e.offset < offset);
    match above.checked_sub(1).map(|i| &entries[i]) {
        Some(title) if contents_title(&title.text) => usize::from(title.depth),
        Some(heading) => usize::from(heading.depth) + 1,
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_nest_by_prefix_and_page_numbers_strip() {
        assert!(holds_number(&[1], &[1, 2]));
        assert!(holds_number(&[1, 2], &[1, 2, 3]));
        assert!(!holds_number(&[1, 2], &[1, 2]));
        assert!(!holds_number(&[1, 2], &[1, 3, 1]));
        assert_eq!(strip_page("附录 A ........ 12"), "附录 A");
        assert_eq!(strip_page("Install…… 3"), "Install");
        assert_eq!(strip_page("Install"), "Install");
        assert_eq!(decode("%E7%9B%AE%E5%BD%95"), "目录");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(label("  1.1   采购\t管理 "), "1.1 采购 管理");
    }
}
