# Changelog

## 0.1.2 — 2026-10-12

### Added
- Collapsible, persistent heading outline with click-to-jump and independent wheel scrolling. A separate worker scans up to 16 MiB, keeps at most 4096 ATX headings, and reports limits explicitly; fenced code is excluded.
- Custom 48-pixel title bar, window controls, border and themed scrollbars. Open, find and more actions share the title bar; the second toolbar has been removed.
- Native scrollbar and window regression scripts covering real window message paths, outline navigation/persistence, narrow layouts and resize hit testing.

### Fixed
- Keep outline controls in the document area: collapse from the outline heading and expand from the left content edge, without a duplicate control in the application title bar.
- Release the App borrow before entering the default window procedure: nested native tracking messages now reach application navigation.
- Track custom thumbs through capture, use 64-bit seek geometry, and discard stale page results when returning to an already cached location.
- A quick click followed by a drag also starts tracking when Windows emits WM_LBUTTONDBLCLK; cancellation releases capture.
- Handle icon-only labels without passing an empty UTF-16 allocation to DrawTextW.
- Preserve the source anchor and wrapped row when resizing, and remove caption bits implicitly added during window creation.

### Verification boundaries
- Directory scans and their memory use are bounded; this is not a complete full-file heading index for arbitrarily large documents.
- Automated UI tests use hidden native windows and messages. Physical pointer checks are recorded separately in docs/PROGRESS.md; multi-monitor transitions and opt-in association changes remain unverified.

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
