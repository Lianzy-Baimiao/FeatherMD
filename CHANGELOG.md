# Changelog

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
