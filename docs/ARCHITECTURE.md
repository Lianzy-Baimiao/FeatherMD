# Architecture

## Boundaries

- document.rs: positional File reads (not memory maps), UTF-8-aligned 4096-byte segments, bounded 128 KiB windows, up to 2048 retained physical lines around a byte anchor. Can tolerate a file being truncated externally. BOM validation does not modify input.
- markdown.rs: line-local Markdown styles using pulldown-cmark; deterministic, no I/O, no HTML execution. Code fence recognition supports marker type and minimum closing length within a page. GitHub pipe tables are the one multi-line construct (see Tables).
- worker.rs: three independent background workers for open/page requests, cancellable literal search and bounded outline extraction. Pending requests coalesce; generation IDs discard stale results. Search and outline check cancellation every 256 KiB.
- outline.rs: up to 16 MiB of positional reads, at most 4096 ATX headings and 160 Unicode characters per label. Long physical lines retain only a 4096-byte prefix; code fences and chunk boundaries are tracked without retaining the full file. Hiding the outline cancels pending work.
- settings.rs: validated settings and a sibling-temp + atomic replacement save. Portable settings are opt-in. No recent-path history.
- layout.rs: UTF-16 GDI measurement, surrogate-safe wrapping, reusable font cache. Only one bounded page is laid out.
- viewport.rs: pure pixel-height navigation over bounded layout rows; visible ranges, one-row page overlap, and EOF viewport positioning. No I/O or Win32 dependency.
- ui.rs and ui/chrome.rs: single-row custom window frame, collapsible outline, native popup menus/file picker/search input, double-buffered reader and captured custom scrollbar input. RefCell prevents aliasing; release its borrow before DefWindowProc enters nested move/resize/tracking loops.
- Window size: the process is per-monitor DPI aware (v2), so CreateWindowEx sizes are physical pixels. Before the first show, `initial_frame` scales the 1080×820 logical default to the window's DPI, caps it to the monitor work area (never below the 640×480 logical minimum) and moves the system-chosen position back inside the work area. Later DPI changes use the rectangle suggested by WM_DPICHANGED.
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

Queries 18–31 expose horizontal position and custom scrollbar geometry. Queries 32–37 expose outline visibility, pending status, heading count, sidebar width, list position and heading offsets. Query 39 is the saved skin index, 47 the previewed skin (-1 when none), 48 the applied window alpha, 49 the header height in device pixels and 50 whether a client point (LPARAM) is inside the text-menu area. Queries 51–53 report automatic update checks started, whether a newer release is known, and whether a check is in flight. Query 54 is the widest table's right edge and 55 the reading width; 56/57/58/64/66 give table LPARAM's right edge, visual lines, columns, first layout row and rows, and 65 the table count. Queries 59–61 give layout row LPARAM's painted text left, right and middle y. Query 62 writes what Copy would put on the clipboard to `copy.txt` beside the snapshot file and returns its UTF-8 length (only with FEATHERMD_TEST_SNAPSHOT), so tests never touch the user's clipboard; 63 says whether a source selection is non-empty. `examples/editor_probe.rs` is a windowless stand-in editor that records its arguments only when FEATHERMD_EDITOR_PROBE is set; it is not shipped. Native tests drive down/move/up, track clicks, wheel input, capture loss, cached paragraph scrolling, outline toggling/clicking and client-frame hit tests. Internal GDI captures verify the complete custom frame, both themes and narrow layouts.

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

## Update checks (the only network access)

- `src/update.rs` (cross-platform, pure): SemVer comparison, a bounded non-recursive JSON reader that takes `tag_name`, `name`, `published_at`, `draft` and `prerelease` from the top level only, tag validation, release-page URLs built from the fixed repository, and the once-per-day schedule. Fuzzed with truncated and mutated payloads.
- `src/updater.rs` (Windows): one synchronous WinHTTP GET on a background thread, HTTPS to `api.github.com` with `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY` (fallback: default proxy) and default certificate checks; headers are the user agent, `Accept: application/vnd.github+json` and the API version. 1 MiB response cap. WinHTTP's own response timeout proved unreliable in loopback tests, so `Pending::poll` enforces a 15 s deadline and abandons a stalled thread, whose handles close when it returns.
- Automatic checks: `update_check` (default false), `update_checked`, `update_latest` in settings. A one-shot timer fires 3 s after the window is shown when `update::due` says so; the timer re-checks the preference. Only successful checks update the bookkeeping; failures stay silent. Manual checks run from the Settings "Updates" page (its own poll timer) and report back to the reader with a same-thread `UPDATE_RESULT` (WM_APP+46) so the badge and settings stay in one place. Neither path holds an App or Prefs borrow across a modal loop.
- Test seam: `FEATHERMD_TEST_UPDATE_URL` is honoured only for `http://127.0.0.1:<port>/…`, and plain HTTP is refused for any other host. `examples/update_server.rs` is the loopback stand-in used by `scripts/update-smoke.ps1`; WinHTTP unit tests use an in-process loopback listener. CI never contacts GitHub's API.

## Tables

- `src/table.rs` (pure): GFM cell splitting (optional outer pipes, `\|`), delimiter rows and alignment, column fitting and cell line breaking (after spaces, around CJK, never before closing or after opening punctuation, never inside a surrogate pair). At most 64 columns per row.
- `markdown::blocks` recognises a header row followed by a delimiter row with the same cell count; the delimiter belongs to the header's byte range. Following non-blank lines that start no other block are body rows (missing cells empty, extra cells dropped); `|` lines without a delimiter stay paragraphs, as in GFM. Cell text is escaped so it cannot become a heading, list, quote, rule or fence. A page that starts inside a table keeps its leading `|` rows as one table without a header row. Source mode never builds tables.
- `layout.rs` measures each cell once (per-unit pen positions from `GetTextExtentExPointW`). A column's natural width is its widest unwrapped cell; its minimum is about six ems, its content if narrower, or its widest word up to twelve ems. `fit_columns` keeps natural widths when they fit, otherwise narrows the widest columns first to a common width; when even the minimums do not fit, the table keeps them and is wider than the reading column. Each visual line of a table row is a layout row sharing the table's rule positions, so scrolling and paging stay row-based.
- With wrapping on, only tables wider than the reading column follow the horizontal scroll; text and tables that fit stay in place. With wrapping off, tables keep natural widths and everything scrolls together. Preview copy writes a table row once, as tab-separated cells.

## Source selection

- Source mode selects characters. A `text::Caret` is a loaded line's first file byte plus a byte index into its decoded text, so a selection survives relayout, zoom and page refills. Hit testing maps the pointer to the nearest character boundary using the displayed text (tabs shown as four spaces, controls as U+FFFD) and never splits a surrogate pair; double-click takes the word/space/symbol run under the pointer; Shift extends; dragging outside the text scrolls on the UI timer.
- `text::selected` copies the exact decoded source between two carets with CRLF between physical lines and no break between the segments of a split long line; parts outside the loaded page are left out and reported. The highlight uses a dedicated colour (accent blended into the code background), at least 1.4:1 against code rows in every skin.
- Queries 54–66 (tables, rows on screen and the copy seam) are listed under Test seam.

## External editing

- `src/editor.rs` launches the configured editor with `ShellExecuteExW` (no shell error UI, synchronous), passing the absolute document path as one quoted argument and its folder as working directory. `System` uses the registered `edit` verb and falls back to `%SystemRoot%\System32\notepad.exe` only for "no association" errors. The UI thread initialises COM as STA for shell extensions.
- `Editor::validate_program` accepts only local drive-letter `.exe` paths without quotes, control characters, wildcards, forward slashes or alternate streams; anything else parses back to `System`. Detection reads fixed install locations and per-application App Paths keys; it never scans the disk and never launches anything.
- The text context menu is built per invocation (labels reflect the editor and selection) and released before `TrackPopupMenu`'s modal loop. F5 records `(source byte, wrapped row)` and restores both once the reopened document is ready.
