# Architecture

## Boundaries

- document.rs: positional File reads (not memory maps), UTF-8-aligned 4096-byte segments, bounded 128 KiB windows, up to 2048 retained physical lines around a byte anchor. Can tolerate a file being truncated externally. BOM validation does not modify input.
- markdown.rs: line-local Markdown styles using pulldown-cmark; deterministic, no I/O, no HTML execution. Code fence recognition supports marker type and minimum closing length within a page.
- worker.rs: two persistent background workers, one for open/page requests and one for cancellable literal search. Pending requests coalesce; generation IDs discard stale results. Search checks cancellation every 256 KiB and retains only query-length overlap.
- settings.rs: validated settings and a sibling-temp + atomic replacement save. Portable settings are opt-in. No recent-path history.
- layout.rs: UTF-16 GDI measurement, surrogate-safe wrapping, reusable font cache. Only one bounded page is laid out.
- ui.rs: native menus/file picker/search input, double-buffered custom reader, visible-row painting, byte-based scrollbar. RefCell prevents aliasing across reentrant Win32 callbacks; destruction is handled outside the mutable borrow so WM_CLOSE posts quit reliably.
- association.rs: explicit, per-user registry registration and removal. Never writes UserChoice or extension default values.

## Performance contract

File size is a u64 and navigation is byte-based. Opening never scans the whole document or constructs a whole-file DOM/index. No line-number or global-heading index is promised. Memory cost depends on the fixed window, layout, query and OS/GDI resources, not total file length. Dense blank-line and long-line tests defend the worst shapes, not just typical documents.

Only metadata and page reads happen in I/O workers. Native text measurement is synchronous but bounded. Disk/network calls can still stall a worker; Win32 filesystem cancellation of a blocked device request is not yet implemented.

## Trade-off

Accurate arbitrary-offset Markdown state can require a complete preceding scan. v0.1 chooses latency and bounded work instead, marking noninitial preview windows as context-limited. Raw mode is the exact-source fallback (apart from Unicode replacement, tabs and long-line display segmentation). A future context index must remain optional/bounded and cannot block first paint.

## Test seam

The native window exposes a read-only WM_APP+42 diagnostic query used by scripts/ui-smoke.ps1. A snapshot command is enabled only when the launching test supplies FEATHERMD_TEST_SNAPSHOT; it renders into an in-process GDI bitmap. This avoids unreliable cross-process hidden-window HDC capture. It does not capture the desktop. Tests use a temporary adjacent INI and never touch the real user's AppData settings or default-app registry.

The test snapshot path is an explicitly supplied test output, not data read from a Markdown document. Keep this seam inert during ordinary launches.
