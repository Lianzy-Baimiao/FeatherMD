# Changelog

## 0.1.1 — 2026-10-12

### Fixed
- End positions the final viewport instead of leaving only the last row visible; long wrapped final lines reach their true tail.
- PageUp/PageDown and scrollbar page commands use measured row heights with overlap, avoiding skipped headings.
- Backward cache refills retain the current source position and apply row movement instead of a fixed 4096-byte jump.
- Forward refills retain the wrapped-row ordinal instead of restarting the current paragraph.
- Copy-visible range uses actual intersecting rows rather than a font-size estimate and an extra off-screen row.

### Verification
- Four pure viewport tests added, covering mixed heights, short/empty documents, oversized rows, all 25 valid font sizes and four simulated DPI scales.
- Real-window regression added for 5,000 headings and 300 wrapped CJK/emoji paragraphs, bidirectional cache crossings and all 13 +/- font settings.
- These tests do not validate physical monitor switching, clipboard integration or default-association registry changes.

## 0.1.0 — 2026-10-12

First native Windows release. Rust + Win32/GDI, static CRT, single executable. Bounded background reads, 64-bit byte navigation, cancellable wrap-around search, UTF-8 preview/source modes, three themes, zoom/wrap/width settings, row selection/copy, drag-and-drop, and opt-in default-app registration.

Validation includes 17 core regression tests, native hidden-window interaction tests, 1 GiB real-file benchmarking, and a 100 GiB sparse-file seek test. See README for Markdown/encoding/accessibility limitations.
