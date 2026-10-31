//! Reproducible warm-cache, local-file read + parse benchmark in the file's own
//! format (Markdown, plain text or log), plus one outline scan, the outline tree and
//! contents links, and (with a query) Find's list of matches. Not first-paint timing.
use feathermd::{
    document::{Document, Query},
    outline, plain, toc,
};
use std::{path::PathBuf, sync::atomic::AtomicU64, time::Instant};
fn percentile(samples: &mut [f64], p: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * p).round() as usize]
}
fn main() {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("usage: bench <file> [iterations] [find query]"),
    );
    let count = std::env::args()
        .nth(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(30)
        .clamp(1, 1000);
    let mut open = vec![];
    let mut first = vec![];
    let mut middle = vec![];
    let mut end = vec![];
    let size = Document::open(&path).unwrap().size;
    let mut format = plain::Format::Markdown;
    for _ in 0..count {
        let start = Instant::now();
        let mut d = Document::open(&path).unwrap();
        d.format = plain::sniff(d.format, &d.page(0).unwrap().lines);
        format = d.format;
        open.push(start.elapsed().as_secs_f64() * 1000.0);
        for (anchor, out) in [
            (0, &mut first),
            (size / 2, &mut middle),
            (size.saturating_sub(1), &mut end),
        ] {
            let start = Instant::now();
            let page = d.page(anchor).unwrap();
            std::hint::black_box(plain::blocks(&page.lines, d.format, false));
            out.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let mut d = Document::open(&path).unwrap();
    d.format = format;
    let start = Instant::now();
    let scan = outline::scan(&d, &AtomicU64::new(0), 0, &AtomicU64::new(0))
        .unwrap()
        .unwrap();
    let outline_ms = start.elapsed().as_secs_f64() * 1000.0;
    // The tree part of that scan alone: depths and link anchors.
    let start = Instant::now();
    let mut tree = scan.headings.clone();
    toc::assign_depths(&mut tree);
    std::hint::black_box(toc::Contents::new(&tree));
    let tree_ms = start.elapsed().as_secs_f64() * 1000.0;
    let rows = toc::rows(
        &scan.headings,
        &toc::initial_folds(&scan.headings, 500),
        &[],
        &[],
    )
    .len();
    // Contents links on the first page.
    let mut blocks = plain::blocks(&d.page(0).unwrap().lines, d.format, false);
    let start = Instant::now();
    toc::link(&mut blocks, &scan.headings, &scan.contents);
    let link_ms = start.elapsed().as_secs_f64() * 1000.0;
    let links = blocks
        .iter()
        .filter(|b| {
            b.spans
                .iter()
                .any(|s| s.link.as_deref().is_some_and(|l| l.starts_with('#')))
        })
        .count();
    // Find's list of matches, median of five: matching case, then (as Find starts)
    // letters of either case.
    let query = std::env::args().nth(3);
    let find = |match_case: bool| {
        let Some(text) = &query else {
            return (0.0, 0);
        };
        let (mut times, mut matches) = (vec![], 0);
        for _ in 0..5 {
            let start = Instant::now();
            let found = d
                .find_all(
                    Query { text, match_case },
                    &AtomicU64::new(0),
                    0,
                    &AtomicU64::new(0),
                )
                .unwrap()
                .unwrap();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            matches = found.items.len();
        }
        (percentile(&mut times, 0.5), matches)
    };
    let (find_ms, matches) = find(true);
    let (fold_ms, folded) = find(false);
    println!(
        "{{\"bytes\":{size},\"format\":\"{format:?}\",\"encoding\":\"{}\",\"iterations\":{count},\"open_p50_ms\":{:.3},\"first_p50_ms\":{:.3},\"first_p95_ms\":{:.3},\"middle_p50_ms\":{:.3},\"end_p50_ms\":{:.3},\"outline_ms\":{outline_ms:.1},\"outline_scanned\":{},\"outline_kind\":\"{:?}\",\"outline_entries\":{},\"errors\":{},\"warnings\":{},\"tree_ms\":{tree_ms:.2},\"rows_opened\":{rows},\"contents_links\":{links},\"link_ms\":{link_ms:.2},\"find_p50_ms\":{find_ms:.2},\"find_matches\":{matches},\"find_any_case_p50_ms\":{fold_ms:.2},\"find_any_case_matches\":{folded}}}",
        d.encoding.name(),
        percentile(&mut open, 0.5),
        percentile(&mut first, 0.5),
        percentile(&mut first, 0.95),
        percentile(&mut middle, 0.5),
        percentile(&mut end, 0.5),
        scan.scanned,
        scan.kind,
        scan.headings.len(),
        scan.errors,
        scan.warnings
    );
}
