# Progress / continuation notes

## User intent

Build and keep polishing an ultra-light Rust Windows Markdown viewer, fast first display independent of whole-file loading, settings, opt-in default association, ideally a single EXE; upload working results to Lianzy-Baimiao on GitHub. User requested continued unattended work. Do not disturb existing repositories or default apps.

## 2026-10-12 — initial implementation

- Repository/project: FeatherMD. Native Win32 + GDI, no web runtime, 3 normal direct dependencies.
- Implemented bounded background I/O, UTF-8 segment alignment, source/preview, cancellable original-text search, byte-percent navigation, native menus/open dialog/drop, row selection/copy, 3 themes/zoom/wrap/width, atomic settings, per-user opt-in registration.
- Single EXE uses static MSVC CRT and embeds icon/manifest/version/licenses. Windows 11 x64 local tests passed.
- Tests: 17 core tests including 100 GiB sparse seek; fmt + warning-free Clippy; native hidden-window smoke against 807 B demo and real 1 GiB Markdown; first-layout around 75 ms for the 1 GiB fixture on this machine. See PERFORMANCE.md for exact boundaries.
- Fixed during testing: UTF-8 scalar split at end of read window, wrap-around search missing overlapping origin, UTF-16 BOM handling, Win32 destruction reentrancy, cross-process test input/capture assumptions.
- Local machine defaults to GNU Rust but has MSVC/SDK in a custom location. Use cargo +stable-x86_64-pc-windows-msvc and source ignored target/dev-env.ps1; do not publish that machine-specific file. Do not change the global default toolchain.
- Source published: https://github.com/Lianzy-Baimiao/FeatherMD (main, initial commit 658ea5c).
- Initial GitHub Actions run 38168176475 passed on Windows and Ubuntu, including the native Windows smoke test.
- Final 640 KiB EXE revalidated locally: fmt, Clippy with warnings denied, all 17 core tests, release build, small-document and real 1 GiB native smoke all passed. v0.1.0 packaging/publication completed; see the release record below.
- Hourly continuation remains active after the initial release, using the bounded high-value backlog below; no cosmetic churn.

## v0.1.0 publication record — 2026-10-12

- Public release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.0
- Tag/verified source: c63a265ab03d0c84b42c26325bc83029697188d1. GitHub Actions run 38168408623 passed on Windows and Ubuntu, including native Windows smoke.
- Published assets: FeatherMD.exe (655,360 bytes), FeatherMD-0.1.0-windows-x64.zip (320,434 bytes), SHA256SUMS.txt. GitHub asset SHA-256 digests match local package hashes.
- EXE SHA-256: b8bea5f9d41aa1cf4feeffd54786187a18256a2d42fc99ea6d6ed5d4a09efb8e
- ZIP SHA-256: e91b6a293c3024ccd69fa81e87de4d5c9781b0a2aedb1be9b08b431b0fdc18a0
- Default-app registry mutation was deliberately not exercised on the development user's system. Document this as an outstanding opt-in integration test, not as a verified end-to-end association change.
- Initial delivery is complete. Continue meaningful polish via the hourly heartbeat; preserve published assets and tag, and use a new version for future binaries.

## v0.1.1 navigation work — 2026-10-12

- Reproduced the original End defect with a native keyboard/capture test: only one row was painted despite a long document. The wrapped fixture separately exposed End targeting the first visual row of the final physical line.
- Added pure pixel-height viewport logic and native integration. Fixed EOF placement, heading page skips, fixed-byte backward jumps, wrapped-row refill resets and off-screen copy range.
- Source I/O limits, static CRT, no-network behavior and user-owned default associations are unchanged.
- Added 4 pure tests (21 total), real-window navigation tests and CI coverage. Heading run: 555 forward / 655 backward pages; wrapped run: 393 forward / 447 backward pages. Both exercise 13 actual font settings. Simulated DPI tests are not multi-monitor validation.
- Release candidate v0.1.1 passed local fmt/strict Clippy/21 tests/release build, standard UI smoke, both navigation shapes and real 1 GiB smoke (59.60 ms first layout, single warm-cache sample). EXE: 656,896 bytes. Published after CI and asset verification; see below. v0.1.0 assets and tag remain unchanged.

## v0.1.1 publication record — 2026-10-12

- Public release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.1
- Tag/source: 2f2c002bf0f83f3fcd1e6fd5f0a8b75fbae0a105. GitHub Actions run 38175299658 passed on Windows and Ubuntu; Windows includes both new native navigation shapes.
- Assets verified uploaded: EXE 656,896 bytes, ZIP 330,197 bytes, SHA256SUMS.txt. GitHub SHA-256 digests match local packages.
- EXE SHA-256: 4f2355d7b740821e4627e2971bc22622163303abe3c8f84535f2f905cd5f3eca
- ZIP SHA-256: 27f7afed4fb2cef41d613233c8e95c543531867e1ce69a8cdd74a4a3d0e628f0
- dumpbin confirms only Windows-provided DLL dependencies. Local distribution path remains dist/FeatherMD.exe; older versioned ZIP is preserved.
- Next continuation should pick an unfinished item below rather than repeat the completed EOF/paging work.

## Next high-value polish work (not all required for the initial release)

1. v0.1.1 covers bidirectional heading/wrapped-line cache crossings, EOF placement and 13 native zoom settings. Remaining: selection/copy integration, preserving reading position across resize/zoom, and physical multiple-monitor DPI transitions.
2. Improve Markdown context continuity on sequential page navigation using bounded cached checkpoints; do not scan entire huge files or make arbitrary jumps wait. Add tests that distinguish sequential correctness from explicitly context-limited random jumps.
3. Native accessible reading/text selection and keyboard focus ergonomics. Keep any accessibility projection bounded to the viewport.
4. Assess bounded table layout and local image support only if they do not inflate the binary or block the UI; maintain explicit no-network defaults.
5. Test registration/unregistration in an isolated Windows user/VM; preserve other applications and never touch UserChoice. Update deprecated GitHub Actions runtime versions with verified upstream releases.

## Guardrails

- Do not rewrite version tags or existing release assets silently. New code merits tests and a new version when published.
- Do not claim complete CommonMark rendering, instant arbitrary-size display, or cold-start performance from warm-cache measurements.
- Keep heartbeat notifications quiet unless there is a meaningful verified improvement, publication, failure or required user action. Pick a real bounded improvement per continuation; no cosmetic churn just to appear busy.
