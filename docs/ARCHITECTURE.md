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
- The 34-logical-pixel header uses GDI+ line icons at 0.86 scale, a preview/source capsule and a single theme picker (light/dark/system plus a "More skins" row that opens Settings). The picker is painted last with its own hit targets; Escape/outside click/focus loss/resize dismiss it, and its keyboard state is separate from document navigation. It opens below the native EDIT row when searching, avoiding child-HWND occlusion. Tooltip delay uses the existing UI timer; theme-only persistence still does not relayout. Empty-state sidebar suppression does not change the saved outline preference.
- editor.rs: external editor detection and launch (see External editing). Never writes documents or associations.
- ui/about.rs: modal native dialog with themed GDI decorations and real STATIC/BUTTON children for keyboard/accessibility semantics. Enter is explicitly handled for BS_OWNERDRAW cards. A deferred message releases App state before opening About; the About state is also released before external-link confirmations. Fixed GitHub/support destinations only, no automatic network activity.
- ui/snapshot.rs: shared opt-in in-process GDI snapshot helper, used by the reader and About tests. It remains inert unless FEATHERMD_TEST_SNAPSHOT is explicitly set by the launcher.
- scrollbar.rs: overflow-safe geometry over pixel offsets or u64 source bytes. Entirely cached documents map to row heights so a single wrapped paragraph remains scrollable; larger documents use bounded byte navigation.
- association.rs: explicit, per-user registry registration and removal, with owned key handles and contextual errors. Tests use a disposable registry root through the same implementation, preserving other applications and simulated defaults. Never writes real UserChoice or extension default values. UI registration and default selection are separate commands; the latter launches the supported Default apps URI and leaves the final choice to Windows. See FILE_ASSOCIATIONS.md for API support boundaries.

## Performance contract

File size is a u64 and document navigation is byte-based. First paint never waits for a document scan or whole-file DOM. The optional outline starts after first layout and scans only its bounded prefix on its own worker. No unrestricted global-heading index or line-number index is promised. Memory cost depends on fixed read buffers, bounded headings, layout, query and OS/GDI resources, not total file length.

Only metadata and page reads happen in I/O workers. Native text measurement is synchronous but bounded. Disk/network calls can still stall a worker; Win32 filesystem cancellation of a blocked device request is not yet implemented.

## Trade-off

Accurate arbitrary-offset Markdown state can require a complete preceding scan. v0.1 chooses latency and bounded work instead, marking noninitial preview windows as context-limited. Raw mode is the exact-source fallback (apart from Unicode replacement, tabs and long-line display segmentation). A future context index must remain optional/bounded and cannot block first paint.

## Test seam

The native window exposes a read-only WM_APP+42 diagnostic query used by scripts/ui-smoke.ps1. A snapshot command is enabled only when the launching test supplies FEATHERMD_TEST_SNAPSHOT; it renders into an in-process GDI bitmap. This avoids unreliable cross-process hidden-window HDC capture. It does not capture the desktop. Tests use a temporary adjacent INI and never touch the real user's AppData settings or default-app registry.

The test snapshot path is an explicitly supplied test output, not data read from a Markdown document. Keep this seam inert during ordinary launches.

Queries 18–31 expose horizontal position and custom scrollbar geometry. Queries 32–37 expose outline visibility, pending status, heading count, sidebar width, list position and heading offsets. Query 39 is the saved skin index, 47 the previewed skin (-1 when none), 48 the applied window alpha, 49 the header height in device pixels and 50 whether a client point (LPARAM) is inside the text-menu area. `examples/editor_probe.rs` is a windowless stand-in editor that records its arguments only when FEATHERMD_EDITOR_PROBE is set; it is not shipped. Native tests drive down/move/up, track clicks, wheel input, capture loss, cached paragraph scrolling, outline toggling/clicking and client-frame hit tests. Internal GDI captures verify the complete custom frame, both themes and narrow layouts.

## Navigation regression boundary

PageUp/PageDown use actual measured row heights, not a font-size estimate. EOF clamps the top to a full trailing viewport; an End request accounts for multiple wrapped rows sharing the final source range. When paging requires a refill, the source byte, wrapped-row ordinal and remaining row movement are restored, rather than subtracting an arbitrary byte count. This does not recover missing Markdown fence context at random offsets.

scripts/navigation-smoke.ps1 drives native keyboard input over 5,000 heading lines and a separate wrapped CJK/emoji fixture. Diagnostic queries 14–17 expose current/last-painted wrapped-row positions and row counts; assertions compare actual painted boundaries to the next viewport, not the navigation helper against itself. Tests also exercise all 13 +/- font settings. Simulated DPI scales in pure tests do not substitute for physical multi-monitor DPI validation. No clipboard or default-app registration is changed by these tests.

## About dialog regression boundary

scripts/about-smoke.ps1 checks both link destinations through their real native buttons, cancels the confirmations, verifies the safe default, exercises Tab/Enter/Escape and F1, expands/collapses help and repeats opening/closing to detect GDI leaks. Light/dark/sepia captures check actual background pixels, preventing a successful-but-blank PrintWindow result from passing. About shares the read-only WM_APP+42 snapshot query (4) and renders child controls in-process after releasing its state borrow. These tests do not launch the browser, inject desktop pointer input or replace full assistive-technology/multi-monitor testing.

## Typography and vector chrome

- `src/ui/smooth.rs` owns a process-lifetime GDI+ startup session and short-lived RAII graphics/pen/brush/path resources. AntiAlias8x8, high-quality pixel offsets and round caps/joins replace aliased GDI polygon icon paths. Stroke widths snap to half device pixels (1 px floor); icon centres snap so odd widths sit on pixel centres; rectangular glyphs (window buttons, outline, monitor, folder, zoom) also round every offset to whole pixels with symmetric rounding. Curves stay anti-aliased. GDI+ scopes end before GDI text drawing. Only the Windows-provided gdiplus.dll is added.
- `src/ui/preferences.rs` (formerly typography.rs) is one native modal Settings dialog with Appearance, Reading and Editor pages, installed-font enumeration and owner-drawn combo/button/card surfaces. All values are draft state until Save. Reading ids 1101–1104 are unchanged; range validation is 12–36 logical pixels. No external fonts are loaded. Native Tab/arrows/Enter/Escape retain their control semantics; skin cards behave as a radio group (only the selected card is a Tab stop, arrows move through the 4×3 grid, Enter saves).
- The App releases its RefCell borrow before entering the modal loop. UI and reading font caches are separate; both measurement and rendering use the reading cache. Source/code override family with Consolas. DPI changes rebuild caches; child WM_SETFONT replaces old handles before their cache is freed.
- INI font_family values reject control characters, vertical @ faces and names over 31 UTF-16 units; legacy files keep Microsoft YaHei UI. Windows handles missing glyph/font fallback. Font changes reflow at the source-byte anchor; theme-only changes still do not relayout. A leading UTF-8 BOM is ignored.

## Skins, opacity and live preview

- `Theme` has twelve values in a fixed `Theme::ALL` order (the diagnostic index). `Palette` adds `page` (reading sheet; equal to `bg` for flat skins) and an optional `ornament` (Ancient's ruled margins). Book/Ancient draw the sheet between the outline and the vertical scrollbar; flat skins draw nothing extra, so light/dark/eye-care pixels are unchanged.
- While Settings is open it sends `PREVIEW_APPEARANCE` (WM_APP+45) to the reader with the draft skin index and opacity. The reader stores this in `App::preview`, paints with it and applies opacity, but never saves it. Save copies only dialog-owned fields into the live settings; Cancel/Escape clears the preview. The modal loop holds no App borrow.
- Opacity uses `SetLayeredWindowAttributes(LWA_ALPHA)` on the reader only. 100% removes `WS_EX_LAYERED`. The Settings dialog stays opaque and colours its native caption to the draft skin (DWMWA_CAPTION_COLOR/TEXT_COLOR/BORDER_COLOR; ignored where unsupported).

## External editing

- `src/editor.rs` launches the configured editor with `ShellExecuteExW` (no shell error UI, synchronous), passing the absolute document path as one quoted argument and its folder as working directory. `System` uses the registered `edit` verb and falls back to `%SystemRoot%\System32\notepad.exe` only for "no association" errors. The UI thread initialises COM as STA for shell extensions.
- `Editor::validate_program` accepts only local drive-letter `.exe` paths without quotes, control characters, wildcards, forward slashes or alternate streams; anything else parses back to `System`. Detection reads fixed install locations and per-application App Paths keys; it never scans the disk and never launches anything.
- The text context menu is built per invocation (labels reflect the editor and selection) and released before `TrackPopupMenu`'s modal loop. F5 records `(source byte, wrapped row)` and restores both once the reopened document is ready.
