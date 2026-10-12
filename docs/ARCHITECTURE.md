# Architecture

## Boundaries

- document.rs: positional File reads (not memory maps), UTF-8-aligned 4096-byte segments, bounded 128 KiB windows, up to 2048 retained physical lines around a byte anchor. Can tolerate a file being truncated externally. BOM validation does not modify input.
- markdown.rs: line-local Markdown styles using pulldown-cmark; deterministic, no I/O, no HTML execution. Code fence recognition supports marker type and minimum closing length within a page.
- worker.rs: three independent background workers for open/page requests, cancellable literal search and bounded outline extraction. Pending requests coalesce; generation IDs discard stale results. Search and outline check cancellation every 256 KiB.
- outline.rs: up to 16 MiB of positional reads, at most 4096 ATX headings and 160 Unicode characters per label. Long physical lines retain only a 4096-byte prefix; code fences and chunk boundaries are tracked without retaining the full file. Hiding the outline cancels pending work.
- settings.rs: validated settings and a sibling-temp + atomic replacement save. Portable settings are opt-in. No recent-path history.
- layout.rs: UTF-16 GDI measurement, surrogate-safe wrapping, reusable font cache. Only one bounded page is laid out.
- viewport.rs: pure pixel-height navigation over bounded layout rows; visible ranges, one-row page overlap, and EOF viewport positioning. No I/O or Win32 dependency.
- ui.rs and ui/chrome.rs: single-row custom window frame, collapsible outline, native popup menus/file picker/search input, double-buffered reader and captured custom scrollbar input. RefCell prevents aliasing; release its borrow before DefWindowProc enters nested move/resize/tracking loops.
- scrollbar.rs: overflow-safe geometry over pixel offsets or u64 source bytes. Entirely cached documents map to row heights so a single wrapped paragraph remains scrollable; larger documents use bounded byte navigation.
- association.rs: explicit, per-user registry registration and removal. Never writes UserChoice or extension default values.

## Performance contract

File size is a u64 and document navigation is byte-based. First paint never waits for a document scan or whole-file DOM. The optional outline starts after first layout and scans only its bounded prefix on its own worker. No unrestricted global-heading index or line-number index is promised. Memory cost depends on fixed read buffers, bounded headings, layout, query and OS/GDI resources, not total file length.

Only metadata and page reads happen in I/O workers. Native text measurement is synchronous but bounded. Disk/network calls can still stall a worker; Win32 filesystem cancellation of a blocked device request is not yet implemented.

## Trade-off

Accurate arbitrary-offset Markdown state can require a complete preceding scan. v0.1 chooses latency and bounded work instead, marking noninitial preview windows as context-limited. Raw mode is the exact-source fallback (apart from Unicode replacement, tabs and long-line display segmentation). A future context index must remain optional/bounded and cannot block first paint.

## Test seam

The native window exposes a read-only WM_APP+42 diagnostic query used by scripts/ui-smoke.ps1. A snapshot command is enabled only when the launching test supplies FEATHERMD_TEST_SNAPSHOT; it renders into an in-process GDI bitmap. This avoids unreliable cross-process hidden-window HDC capture. It does not capture the desktop. Tests use a temporary adjacent INI and never touch the real user's AppData settings or default-app registry.

The test snapshot path is an explicitly supplied test output, not data read from a Markdown document. Keep this seam inert during ordinary launches.

Queries 18–31 expose horizontal position and custom scrollbar geometry. Queries 32–37 expose outline visibility, pending status, heading count, sidebar width, list position and heading offsets. Native tests drive down/move/up, track clicks, wheel input, capture loss, cached paragraph scrolling, outline toggling/clicking and client-frame hit tests. Internal GDI captures verify the complete custom frame, both themes and narrow layouts.

## Navigation regression boundary

PageUp/PageDown use actual measured row heights, not a font-size estimate. EOF clamps the top to a full trailing viewport; an End request accounts for multiple wrapped rows sharing the final source range. When paging requires a refill, the source byte, wrapped-row ordinal and remaining row movement are restored, rather than subtracting an arbitrary byte count. This does not recover missing Markdown fence context at random offsets.

scripts/navigation-smoke.ps1 drives native keyboard input over 5,000 heading lines and a separate wrapped CJK/emoji fixture. Diagnostic queries 14–17 expose current/last-painted wrapped-row positions and row counts; assertions compare actual painted boundaries to the next viewport, not the navigation helper against itself. Tests also exercise all 13 +/- font settings. Simulated DPI scales in pure tests do not substitute for physical multi-monitor DPI validation. No clipboard or default-app registration is changed by these tests.
