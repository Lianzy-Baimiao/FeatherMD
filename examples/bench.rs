//! Reproducible warm-cache, local-file read + Markdown parse benchmark. Not first-paint timing.
use feathermd::{document::Document, markdown};
use std::{path::PathBuf, time::Instant};
fn percentile(samples: &mut [f64], p: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * p).round() as usize]
}
fn main() {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("usage: bench <file> [iterations]"),
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
    for _ in 0..count {
        let start = Instant::now();
        let d = Document::open(&path).unwrap();
        open.push(start.elapsed().as_secs_f64() * 1000.0);
        for (anchor, out) in [
            (0, &mut first),
            (size / 2, &mut middle),
            (size.saturating_sub(1), &mut end),
        ] {
            let start = Instant::now();
            let page = d.page(anchor).unwrap();
            std::hint::black_box(markdown::blocks(&page.lines, false));
            out.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    println!(
        "{{\"bytes\":{size},\"iterations\":{count},\"open_p50_ms\":{:.3},\"first_p50_ms\":{:.3},\"first_p95_ms\":{:.3},\"middle_p50_ms\":{:.3},\"end_p50_ms\":{:.3}}}",
        percentile(&mut open, 0.5),
        percentile(&mut first, 0.5),
        percentile(&mut first, 0.95),
        percentile(&mut middle, 0.5),
        percentile(&mut end, 0.5)
    );
}
