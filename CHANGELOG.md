# Changelog

## 0.1.10 — 2026-10-16

### Added
- Click an HTTP(S) image placeholder in the preview, confirm the exact address, and load that image in place. Works with automatic local images off; failures offer an explicit retry. Identical URLs share the in-memory result. No automatic remote downloads, cookies, authentication, referer, redirects, disk cache or saved permissions; F5/reopening clears the result. Existing raster, byte, pixel and memory bounds remain in force; downloads run separately from local decoding, with a bounded queue and a 15-second UI deadline.
- Four-language placeholder, confirmation, retry, Settings and About text. URL-policy, loopback HTTP and stale-result tests; image-smoke covers cancel, load, deduplication, linked images, failure/retry, source/preview switching and reload/restart without automatic downloads.

### Fixed
- The English image note in About fits small work areas, including the 96 DPI CI desktop; GDI measurement tests cover 90–144 DPI and the standard/fallback fonts.

## 0.1.9 — 2026-10-14

### Added
- Local images, **off by default** (Settings → Reading → "Show images", `images=true|false`). When on, only files on this computer are read: relative paths from the document's folder, drive paths and `file:///C:/…`. Links, junctions, network drives, shares, device paths and online-only OneDrive files are refused before any byte is read; web images are never downloaded. The format is sniffed from the content (PNG, JPEG, GIF, BMP, TIFF, ICO) so a mislabelled metafile never reaches a decoder. Files are capped at 32 MiB, 32 megapixels and 16384 px a side, decoded with GDI+ on a background thread from memory, turned upright from EXIF orientation and kept at most 2048 px a side; a page shows at most 64 images within a 160 MiB budget, and turning the option off frees them at once. Images are drawn with their transparency at CSS-pixel size (HTML `width`/`height` honoured), shrink to the text width and the reading area's height, and sit in the text flow, so logos, badge rows and inline icons all work and linked images stay clickable. Images that are not shown keep a placeholder with the reason (loading, web image, missing, unsupported, too large, link/network/device path, online-only).
- A safe HTML subset that shapes text and never runs: `p`, `h1`–`h6` (also in the outline), `br`, `hr`, `b`/`strong`, `i`/`em`, `u`, `s`/`del`, `code`, `kbd`, `mark`, `sub`/`sup`, `a`, `img`, `blockquote`, `ul`/`ol`/`li`, `pre`, `div`/`span`/`center`, `details`/`summary`, `align` and `text-align` centring, and character entities. `script`, `style`, `iframe`, `object`, `svg`, forms and the like are hidden; comments and declarations are skipped; nesting is bounded.
- Code highlighting for about 25 languages (Rust, C/C++, C#, Java/Kotlin/Scala, Swift, Go, JavaScript/TypeScript, Python, Ruby, PHP, Lua, SQL, shell, PowerShell, batch, JSON, YAML, TOML/INI, Dockerfile, CSS, HTML/XML, diff), line by line with a small state for block comments and multi-line strings; colours are derived from each skin's text colour.
- Eight skins (20 in all): Paper, Newsprint, Lilac, Dusk, Polar, Deep sea, Violet and Mocha. The gallery is a 5×4 grid with light skins first; saved skin indices of the first twelve are unchanged. Every skin passes the same contrast floors (body ≥ 9.8:1, secondary ≥ 4.3:1, links ≥ 4.5:1).
- Character selection in the preview, as shown: drag, double-click a word, Shift+click, Ctrl+A, Esc and drag auto-scroll. Copy joins the wrapped lines of a paragraph (restoring the spaces removed at the wraps), copies table rows as tab-separated cells and images as their alt text. A selection is anchored to source bytes, so zooming, resizing and images arriving keep it.

### Changed
- Reading layout: default size 16 px (was 18), body lines about 1.6× the font size, shorter blank-line gaps, more room above headings than below, rules under levels 1–2 drawn under a wrapped heading's last line only, code blocks on one rounded panel with a smaller monospace font and a language label, rounded inline-code and `kbd` chips, quotes with a grey bar and secondary text, striped table rows, checkbox task items, hanging indents for wrapped list items, baseline-aligned mixed fonts, superscript/subscript.
- The default reading font follows the interface language (Meiryo for Japanese, the interface font otherwise) and is stored as an empty `font_family`, so it changes with the language; an explicitly chosen font is kept.
- Title bar: the "FeatherMD" wordmark at the top left is gone (only the feather mark and the document name remain); Open, Find, Appearance and More share one size, stroke and hover style; Appearance is one fixed half-filled circle without a chevron; caption buttons light up as full-height columns and Close turns Windows red; the preview/source switch moved to a segmented control at the left of the status bar.
- Settings → Appearance note and Reading page layout updated; menu labels say "Copy selected / visible text".
- About is up to date: the reading notes say both views select by character and add a line on images (off by default, turned on in Settings → Reading, local files only, web images never downloaded); the expanded panel is 20 logical px taller for it.

### Tests
- Unit tests: image path rules (device paths, shares, schemes, reserved names, streams, percent-decoding), raster signatures, placeholder text, GDI+ decode of premultiplied PNG, EXIF orientation 6, size caps and non-raster input, path checks refusing a junction, preview selection (covered units, extents, nearest boundary, copy across wraps/blocks/tables/images, word selection), HTML (`center`, `text-align`), line breaking around images and a property test that wrapped paragraphs never start a line with closing punctuation at any width; all skins and the gallery order.
- New image-smoke (light/dark): off reads nothing; on draws five images with exact size, centring, transparency, EXIF rotation, badges on one line, a linked hit target, a column-wide image refitting on resize, reasons for web/junction/missing/broken/SVG images, flat GDI handles, and Settings turning images off frees their memory and bitmaps.
- table-smoke covers preview character selection (drag across lines, double-click, Shift+click over CJK and emoji, select all, Esc, the selection surviving a zoom); settings-smoke covers "Show images" default, save and reset; window/i18n/update smokes follow the new header and footer.

## 0.1.8 — 2026-10-13

### Added
- One-click copy for code blocks, quotes and tables. Hovering one in the preview shows a Copy button in its top-right corner: code is copied as its exact source without the fence lines (indented code loses one level of indentation), quotes as shown, tables as tab-separated rows. The button keeps to the top of a long block's visible part, sits beside a table narrower than the text instead of over its header, and shows a check mark for 1.5 s. A block running past the loaded page copies its loaded part and says so.
- Interface languages: 简体中文, 繁體中文, English and 日本語. By default the interface follows the Windows display language (Taiwan, Hong Kong and Macao use Traditional Chinese; languages without a translation use English). Settings has a new "Language" page, also reachable from More → "界面语言 / Language…"; Save switches menus, buttons, messages and dialogs at once, without a restart. Each language uses an interface font with its regional glyph forms (Microsoft YaHei UI, Microsoft JhengHei UI, Yu Gothic UI). Stored as `language=system|zh-CN|zh-TW|en|ja`.
- README in Traditional Chinese, English and Japanese (README.zh-TW.md, README.en.md, README.ja.md) with a language switcher, each with its own reading view, About window and language page, and translated demo documents (docs/demo.*.md). All four are in the release ZIP.
- Native i18n regression (i18n-smoke): every view — welcome, header tooltips, appearance picker, menus, all Settings pages, About collapsed and expanded, a document with its copy button, Find and Go to — in each language, with no text cut off; "System" resolves to this machine's display language; eight live switches keep GDI and USER handle counts flat.

### Changed
- About is up to date: the expanded panel now lists shortcuts in two columns (adding Ctrl+E, Ctrl+, , Ctrl+T, Esc) plus notes on one-click copy, table layout, character selection and the paged preview; the "Back to reading" button is wider with centred text, and the version sits centred in its pill.
- The welcome "Open document" button widens when a translation needs it (it stays 208 logical px in Chinese).
- docs/demo.md mentions the 12 skins, the interface languages and the copy button, and says "offline by default" instead of "fully offline".

### Fixed
- window-smoke: the INI reset before the welcome checks was on the same line as `StopReader`, so PowerShell passed it as an ignored argument and the welcome captures inherited the previous run's settings.

### Tests
- Unit tests for box grouping (fences, mismatched fence markers, empty fences, indented code, quotes, tables, boxes cut off by the page edge), exact copy with dedent and split long lines, translation-table integrity (placeholders, mnemonics, shortcut suffixes, no simplified characters in Traditional Chinese or Japanese), a source scan that every Chinese interface literal is translated, Windows LANGID mapping and the language setting round trip.
- table-smoke covers the copy buttons: placement, sticky top, the narrow-table position, every box kind's copied text, the check mark clearing, source mode without buttons and GDI stability. Every smoke now pins `language=zh-CN`, since CI runners use an English system locale.

## 0.1.7 — 2026-10-13

### Added
- Tables. GitHub-style pipe tables (header row plus a `| --- | :-: | --: |` delimiter row) render as a grid with a shaded header and rules; cells keep bold, inline code and links, and the delimiter colons set left/centre/right alignment. Previously every `|` line was shown as raw monospace text.
- Tables fit the window: columns keep their content width when the table fits; otherwise the widest columns narrow first and cells wrap by word or CJK character, never starting a line with closing punctuation. A table whose columns cannot fit even at about six characters each scrolls horizontally on its own while the text stays put. Without wrapping, tables keep their content width and scroll with the page.
- Character selection in source mode: drag with an I-beam cursor, double-click a word, Shift+click to extend, and drag past the top or bottom edge to scroll. Ctrl+A selects the loaded page, Esc clears, Ctrl+C or "复制选中文本" copies the exact source (tabs kept, CJK and emoji intact, CRLF between lines, no extra break inside a split long line). Without a selection, Copy takes the visible source.
- Preview copy writes table rows as tab-separated cells, once per row however many lines it wraps to.
- Native table/selection regression (table-smoke, light/dark): fitting, wrapping in a narrow window, the wide table scrolling alone, unwrapped widths, TSV copy, drag/double-click/Shift+click/select-all/Esc and drag auto-scroll, read through a test seam instead of the clipboard.

### Changed
- `|` lines without a delimiter row are ordinary paragraphs (as in GFM) instead of monospace text.
- New windows follow display scaling. The default size is now 1080×820 logical pixels at the monitor's scale (1620×1230 physical at 150%) instead of a fixed 1080×820 physical pixels, which left only 720×547 logical pixels and the narrow outline at 150%. The size is capped to the monitor work area, never below the 640×480 minimum, and a cascade position that would overflow moves back inside the work area. Moving between monitors already followed DPI through the size Windows suggests.

### Tests
- Unit tests for table splitting, delimiter rows, column fitting, cell line breaking (CJK, punctuation, surrogate pairs), table detection and its limits, pages that start inside a table, caret/display mapping, exact source copy and word selection; every skin keeps the source highlight at least 1.4:1 against code rows.
- window-smoke checks the default frame against the monitor's scale and work area instead of widening the window first; a unit test covers 100/125/150/200%, overflowing positions, small work areas and monitors with negative coordinates.

## 0.1.6 — 2026-10-13

### Added
- Update checks against GitHub Releases. More → Check for updates opens the new Settings "Updates" page and checks at once; results show up to date, a newer release (name and date) or the reason a check failed.
- Optional check on start, **off by default**: when enabled, a background check about 3 s after first paint, at most once per 24 h; failures are silent and retried on a later start.
- A newer release is marked by a small dot on the More button and a "发现新版本 vX.Y.Z…" menu entry, kept across restarts until you upgrade. "Go to download page" opens the release page after a confirmation; nothing is downloaded or installed.
- Native update regression (update-smoke, light/dark) against a loopback stand-in for the GitHub API: default-off, once-daily start check, request headers, badge, menu label, manual results, HTTP/HTML failures, link confirmation, checkbox persistence and closing during a check.

### Network boundary
- The only request is an HTTPS GET of `api.github.com/repos/Lianzy-Baimiao/FeatherMD/releases/latest` through Windows' WinHTTP, using the system proxy and certificate validation. Responses are capped at 1 MiB and parsed by a bounded reader; release pages are built from the fixed repository, not from response URLs. A check always ends within 15 s.
- About now says "默认不联网" (offline by default) instead of "无自动联网".

### Fixed
- Native About and update tests wait for confirmation dialogs to be filled in before reading them (a rare race in about-smoke).
- Native window and navigation tests no longer assume display scaling of 125% or less. At 150% the default window is under 800 logical px wide, so window-smoke widens it before its wide-outline checks, and the paging loop limit is now only a hang guard (the headings fixture needs 1111 pages there). The 0.1.5 reader failed both tests the same way; the reader itself is unchanged.

## 0.1.5 — 2026-10-13

### Added
- Text context menu (right click, Shift+F10 or the Menu key, reading area only): edit with the default editor (Ctrl+E), choose editor, copy, select current page, find and reload.
- Settings window (More → Settings…, Ctrl+,) with Appearance, Reading and Editor pages, replacing the separate font dialog. Drafts apply only on Save; skin and opacity preview live on the reader and are reverted by Cancel/Escape.
- Twelve skins: follow system, light, dark, eye care, Book (paper sheet on a linen desk), Ancient (rice paper, ink, cinnabar and double ruled margins), bean green, sky, pink, gray, midnight blue and pure black. Every skin keeps body text at ≥ 9.8:1 and secondary text at ≥ 4.3:1 contrast.
- Window opacity 30–100% in 5% steps (native trackbar, keyboard accessible); fully opaque windows are not layered.
- Default editor: Windows edit verb with Notepad fallback, Notepad, detected VS Code / VSCodium / Cursor / Typora / MarkText / Notepad++ / Sublime Text / Zed / EmEditor, or any local .exe. The document is passed as one quoted argument with its folder as working directory; a missing editor offers to choose another.
- The header picker gains a fourth "More skins…" row; gallery skins show a palette icon.
- Native regressions: settings-smoke (skins, preview/cancel, opacity alpha, grid keys, persistence, GDI) and editor-smoke (stand-in editor, Unicode path quoting, real context menu label and keyboard launch, reload position, missing-editor prompt).

### Changed
- Title bar 40 → 34 px with icons scaled to match. Stroke widths snap to half pixels and centres to the pixel grid, so straight edges stay crisp while curves remain anti-aliased.
- F5 keeps the source byte and wrapped row after reloading.
- The eye-care skin's secondary text is slightly darker (5.1:1 instead of 4.2:1).

### Fixed
- Settings files saved as UTF-8 with a BOM (Notepad, Windows PowerShell 5) no longer drop their first key.
- Smoke scripts containing Chinese text carry a UTF-8 BOM, so Windows PowerShell 5.1 parses them correctly.

## 0.1.4 — 2026-10-12

### Changed
- Compact 40 px custom title bar (was 48 px), with header icons, search row, theme picker, tooltips and outline panel aligned to the new height.

### Fixed
- Clicking an outline heading always highlights that heading, even when short sections share one screen or the document end prevents it from becoming the top row. Scrolling or other navigation returns to position-based highlighting.

## 0.1.3 — 2026-10-12

### Added
- More → Font and default size: native installed-font selector, 12–36 px input, live preview, draft reset, Save/Cancel and persistent preferences. Body/headings use the chosen face; UI and monospace source/code stay independent.
- GDI+ anti-aliased vector chrome with rounded strokes and curved feather/eye/moon icons, without downloadable assets or third-party runtime dependencies.
- Font-dialog native light/dark tests for keyboard selection, validation, cancellation, restart, rendering isolation and GDI resource cleanup.
- Redesigned the main header, not just About: feather identity, compact preview/source icon capsule, one theme button with a themed picker, and folder/search/ellipsis actions with delayed shortcut tooltips. Ctrl+T opens appearance; arrows/Tab, Enter and Escape work in the picker.
- Centered welcome page with a feather mark and one open/drop affordance; no empty outline pane before opening a document.
- Native regressions for popup pointer/keyboard interactions, outside/focus/resize dismissal, search-field avoidance, tooltip delay and narrow light/dark welcome layouts.
- More → About FeatherMD (F1), with a custom native, theme-aware card layout, version badge, GitHub/project and voluntary support links, and collapsible keyboard help. Links always require confirmation with Cancel as the default.
- MIT and 爱发电 README badges matching WowAltBoard’s flat-square style, plus light/dark About previews.
- About native regression for link destinations/cancellation, Tab/Enter/Escape, expandable help and GDI resource cleanup.
- Direct header access to preview/source and light/dark/follow-system, now compacted into an icon capsule and one appearance button. System preference is the default for new settings, refreshes on Windows theme notifications and persists independently of its resolved color.
- A dedicated File associations submenu with separate actions for adding FeatherMD to Markdown's Open with list, choosing it as the .md default through Windows, and removing registration.
- Registration completes without opening Settings or asking an extra confirmation. Default selection automatically registers the app and opens its per-user Default apps page where supported, with a Settings-home fallback and accurate failure reporting.
- Friendly application metadata and isolated Windows registry integration coverage for repeated registration, quoted Unicode paths, value types and cleanup that preserves other applications and default choices.

### Fixed
- Theme-only changes no longer rebuild layout or reset wrapped-paragraph reading position. Clicking the already-selected reading mode is a no-op.
- Scrollbar smoke tests use physical coordinates on scaled displays and verify reading-position stability across all theme commands.

### Platform boundary
- Windows 10/11 require user confirmation through the system's Default apps interface. No unsupported silent default setter or UserChoice hash manipulation is included.

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
