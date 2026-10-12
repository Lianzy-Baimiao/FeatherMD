# Progress / continuation notes

## v0.1.5 publication record — 2026-10-13

- 作者/最后修改人：lianzy。用户明确要求“提交推送并发布”。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.5
- Tag/source: ffbdaac331b4725548450a92a1c881c8ead4d636 (annotated tag v0.1.5). GitHub Actions run 38309195090 (main) passed on Windows and Ubuntu, including the new settings/editor smokes; the Windows runner showed the real context menu (native_context_menu_label_and_keyboard_launch=true). Tag run 38309635003 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38309195090
- Assets: FeatherMD.exe 783360 bytes, FeatherMD-0.1.5-windows-x64.zip 874161 bytes, SHA256SUMS.txt 180 bytes (SHA-256 e5383fae1f2a5a9dc1c2fa2367e943f60d3c4b52bc250209db6f3490248ed33b). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.5 candidate entry below.
- Release-note preview URLs under v0.1.5/docs return HTTP 200. v0.1.4 tag (9c4868a) and its asset digests are unchanged.

## 2026-10-13 — v0.1.5 release candidate: right-click edit, skins and opacity, 34 px header

- 作者/最后修改人：lianzy。用户需求：①正文右键增加编辑，设置界面可选默认编辑应用；②标题栏再矮一点，图标同步缩小、圆润无锯齿；③设置界面增加皮肤（书本、古籍、常用颜色）并支持调整透明度。
- 右键菜单仅在正文区域出现（右键 / Shift+F10 / 菜单键）：用默认编辑应用打开（Ctrl+E）、选择编辑应用、复制、全选当前分页、查找、重新载入。文档以单个带引号参数传给编辑器，工作目录为文档所在文件夹；FeatherMD 仍只读。F5 重新载入保留源字节与换行行号。
- 新设置窗口（更多 → 设置…，Ctrl+,）取代独立字体对话框：外观（12 款皮肤卡片、窗口不透明度 30–100%）/ 阅读（原字体字号，控件 id 不变）/ 编辑（系统默认、记事本、检测到的编辑器、浏览 EXE）。草稿保存才生效；皮肤与透明度实时预览，取消还原。设置窗口原生标题栏随草稿皮肤着色。
- 皮肤：跟随系统、浅色、深色、护眼、书本、古籍、豆沙绿、天青、樱粉、雾灰、夜蓝、纯黑。书本/古籍在桌面底色上绘制纸页，古籍带朱砂双线栏。对比度脚本计算：正文 ≥ 9.8:1，次要文字 ≥ 4.3:1，链接 ≥ 4.7:1；护眼次要文字由 4.24 调到 5.06。
- 标题栏 40 → 34 px，按钮 30 → 26 px，图标 0.86 倍。描边宽度取半像素、中心对齐像素网格，窗口按钮等矩形图标坐标取整；96/120/144 DPI 截图放大检查：直线清晰、曲线抗锯齿。
- 修复：INI 以 UTF-8 BOM 保存时首个键被忽略（PowerShell 5 的 Set-Content 会写 BOM，ui-smoke 的“浅色”截图因此曾是深色）。含中文的冒烟脚本加 UTF-8 BOM，Windows PowerShell 5.1 才能正确解析。
- 证据（120 DPI，MSVC release 783,360 B，FileVersion 0.1.5，--locked 构建）：fmt --check、clippy -D warnings、cargo test --locked（44 项，含 12 款皮肤对比度下限测试）全部通过；以下原生回归在交付 EXE 上运行：ui（GDI 35→35）、navigation headings 454/535、wrapped 449/504、scrollbar、window（51 项命中）、typography 浅/深（GDI 35→35）、about 浅/深（GDI 22→22）、settings 浅/深（GDI 35→35）、editor 冒烟全部 PASS。
- wrapped 页数与 v0.1.4 记录的 473/523 不同；用 dist 中的 v0.1.4 EXE 在本机重跑同样得到 449/504，差异来自本机环境而非本次改动，原因未查明。
- 实机可见窗口检查（短暂显示后关闭）：80% 不透明度下标题栏像素与桌面混合（F4F6F3 → C8CAC9），圆角裁切保持；设置窗口标题栏随书本皮肤着色。主窗口 DWMWA_BORDER_COLOR 改为边线色在本机像素逐字节无差别，已恢复为 DWMWA_COLOR_NONE。
- Delivery: dist/FeatherMD.exe 783,360 字节（765 KiB），SHA-256 cc576f353784206352389aaa934ad3452d4b5425af7b466ec05562a332e73578；dist/FeatherMD-0.1.5-windows-x64.zip 874,161 字节，SHA-256 37029a56e5837ccdd7f2f75bf6f22764422f641f122039c86b14db61acc68140（新增 5 张截图）。构建、交付与 ZIP 内 EXE 哈希一致；早期 ZIP 均保留，此前的本地候选包改名为 0.1.5-candidate。全部 12 项原生回归针对交付 EXE 通过；演示文档首屏 80.99 ms 为单次本地小文件样本。
- 限制：隐藏窗口 + 原生消息测试，非物理指针；Ctrl+E 需要真实键盘状态，未自动化；编辑器用 examples/editor_probe 替身，未启动真实编辑器或 Windows edit 动词；CI 上用 -AllowNoPopup 容许服务桌面不显示弹出菜单。未做多显示器 DPI 与 Windows 10 实机验证。用户确认后推送 GitHub，CI 通过后发布；发布记录见上一条。

## 2026-10-12 — v0.1.4 compact header and outline click highlight release candidate

- 作者/最后修改人：lianzy。标题栏由 48 px 收窄为 40 px（逻辑像素）；搜索行、主题弹层、提示与目录面板随 HEADER_HEIGHT 联动。
- 修复：短章节同屏或文末无法把被点标题滚到顶行时，目录高亮停在最上方标题。点击后记录（标题索引，跳转后的顶行），顶行不变时高亮所点标题；滚动、跳转、重排、打开文件后回到按位置判断。跨页加载在页回复后再记录。
- 证据（120 DPI，MSVC release 738,304 B）：fmt --check、clippy -D warnings、cargo test 全通过；ui、navigation（headings/wrapped）、scrollbar、window、typography（light/dark）、about（light/dark）冒烟均 PASS。window-smoke 新增 8 个短章节夹具，依次点击第 3/6/1 个标题并用 WM_APP+42/46 断言高亮索引，Home 后回到 0。已目视检查截图并更新 docs 预览图。
- 限制：仍为隐藏窗口合成消息测试，未覆盖真实指针输入。
- Version bump: Cargo/Cargo.lock (feathermd only), app.rc FileVersion/ProductVersion 0.1.4, package.ps1 default 0.1.4. Rebuilt and reran fmt, strict Clippy, cargo test --locked (40 tests) and all 9 native regressions against the 0.1.4 build: PASS. About light/dark show v0.1.4 (GDI 22→22); previews regenerated.
- Delivery: dist/FeatherMD.exe 738304 bytes (721 KiB), SHA-256 33882b654634390d9b649aa8b66f2af167ff7ff9d1f06238ea3764a7ea2a3c87. dist/FeatherMD-0.1.4-windows-x64.zip 659473 bytes, SHA-256 081132a4ff717fd217bb7053e01356af165066fb2e3bfc7815ac4723763ddd6e. Build, delivery and ZIP EXE hashes match; earlier ZIPs preserved.

## v0.1.4 publication record — 2026-10-12

- 作者/最后修改人：lianzy。Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.4
- Tag/source: 9c4868a2ac0ee712cc5f6944b12730b4072bc0ce (annotated tag v0.1.4). GitHub Actions run 38269151582 (main) passed on Windows and Ubuntu, including the native Windows smoke steps; tag run 38269160330 also passed: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38269151582
- Assets: FeatherMD.exe 738304 bytes, FeatherMD-0.1.4-windows-x64.zip 659473 bytes, SHA256SUMS.txt 180 bytes (SHA-256 1971e812bb68c9e961fa1070efae231d918951689804d2a4fee6a487077f632e). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.4 entry above.
- Versioned release-note preview URLs under v0.1.4/docs return HTTP 200. Earlier tags and release assets are unchanged.
## 2026-10-12 — v0.1.3 anti-aliased icons, reading font settings and release candidate

- Header, welcome and About icons now use GDI+ anti-aliased vector paths with round caps, drawn at the window DPI. Before/after icon captures show 6 vs 216 edge colors. No icon font, downloaded assets or third-party runtime were added.
- More → 字体与默认字号: installed-font selector (212 faces on this host), 12–36 px default size, live preview, draft-only reset, Save/Cancel/Escape and restart persistence. Body text and headings use the chosen face; UI chrome, source view and code stay on their own fonts. The dialog follows the reader theme and handles WM_DPICHANGED.
- Release candidate build: fmt --check, strict all-targets Clippy, all 40 tests (2+4+19+8+3+4) and the static-CRT MSVC release build pass.
- Native regressions against the 738304-byte candidate (second full batch, 9/9 PASS): UI smoke GDI 35→35; navigation headings 454/535 and wrapped 473/523 pages, 13 font sizes each; scrollbar both axes, vertical middle byte 1834737 of 3675600, wrapped row 25 preserved; window 51 hit tests, resize anchor 941400 preserved; typography light/dark (4 invalid sizes rejected, GDI 31→31); About light/dark (GDI 22→22, browser not launched).
- Intermittent failure: in the first full batch, scrollbar smoke failed once with "Cached paragraph thumb did not move through wrapped rows" (scripts/scrollbar-smoke.ps1). It passed three isolated reruns and the complete second batch. The cause was not identified; treat it as an unresolved possible flake, not a fix.
- README previews regenerated from this build: reading light/dark/outline-hidden, welcome light/dark, About light/dark and new typography light/dark. File-association menu capture unchanged.
- Delivery: dist/FeatherMD.exe 738304 bytes (721 KiB), SHA-256 d199bcde3309e786e7217ee6f8fb2b6a7345a665e576dce0d9776319869be16f. Build, delivery and ZIP EXE hashes match. dist/FeatherMD-0.1.3-windows-x64.zip 659871 bytes, SHA-256 1189ad4b2abbae5586dd57476a4cd41555100b454eb131576aa0c2ad6cc04a01. The earlier local 0.1.3 archive was preserved under an -early suffix. FileVersion 0.1.3, CompanyName lianzy. Delivery EXE UI smoke PASS, GDI 35→35; demo first layout 90.43 ms is a single local sample.
- Boundaries: hidden windows and native messages, not physical pointer input; no multi-monitor DPI, Windows 10, Snap or full accessibility validation. Missing glyphs rely on Windows font fallback. No production association changes were performed.


## v0.1.3 publication record — 2026-10-12

- Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.3
- Tag/source: 4daaaab282e60df04732bff3a9f9d470e2bd22a1 (annotated tag v0.1.3). GitHub Actions run 38255735339 passed on Windows and Ubuntu, including the native Windows smoke steps: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38255735339
- Assets: FeatherMD.exe 738304 bytes, FeatherMD-0.1.3-windows-x64.zip 659871 bytes, SHA256SUMS.txt 180 bytes (SHA-256 6ef32e3da1bc36a02817fd508c60630209def768d07919fa4cfc51f9bbb84359). All three were downloaded again; hashes match the local distribution and GitHub asset digests. EXE/ZIP hashes are recorded in the v0.1.3 entry above.
- Versioned README preview URLs under v0.1.3/docs return HTTP 200. Earlier tags and release assets are unchanged.
- Follow-up: investigate the single unreproduced cached-paragraph scrollbar smoke failure; update deprecated Node 20 Actions (checkout/upload-artifact v4) and note the ubuntu-latest migration to Ubuntu 26 starting 2026-10-19.

## 2026-10-12 — main header and welcome redesign (local 0.1.3 revision)

- Corrected the scope after the user's clarification: rebuilt the main reader/home header, not About. Preserved the existing uncommitted work and all previous packages. No repository reset, commit, push, tag or GitHub publication.
- Replaced the wide text controls with locally drawn feather/line icons, a compact preview/source capsule, folder/search/ellipsis actions and one appearance button showing sun/moon/system state. Controls retain a single 48-logical-pixel header. The application-action area shrank from 400 to 260 logical pixels; window buttons remain separate. Header blank space still drags the native window.
- Added a theme-aware appearance picker (light/dark/system), delayed shortcut tooltips and Ctrl+T. Arrow/Tab navigation, Enter/Space selection, Escape, repeat-button toggle, outside click, focus loss and resize dismissal are implemented. The picker sits below an open native search EDIT row rather than painting over a child HWND. Theme changes still save/repaint without relayout; source/preview semantics and More/About/association actions remain intact.
- Redesigned welcome with a centered feather mark, typographic hierarchy and a single open/drop entry. Suppressed the empty sidebar without changing the saved outline preference; removed redundant welcome footer text. Updated README instructions, light/dark reading/welcome screenshots and package contents.
- Validation: MSVC static-CRT build, fmt --check, strict all-targets Clippy and all 39 tests pass. Final window smoke passed three consecutive runs plus the delivery EXE: 51 frame/control hit tests, 640x480 logical minimum (800x600 physical at 120 DPI), actual picker click/keyboard/dismissal paths, tooltip delay, search-row avoidance and restart persistence. Source byte 941400 is retained on resize. Scrollbar regression preserves wrapped row 25 through theme changes. Navigation: headings 454/535 pages, wrapped 473/523, each exercising 13 native font sizes. About light/dark tests pass with GDI 19→19.
- Test-harness corrections: hidden synthetic mouse moves must not register real desktop leave tracking, and hidden windows need explicit painting before overlay hit-testing. A transient GDI sample saw 30→43 while another run was 30→30; the leak test now samples the minimum across a short quiescence window instead of counting in-flight paint resources, without relaxing its retained-growth threshold. It also exercises ten actual picker paints. Final UI and delivery UI: GDI 31→31. One early synthetic source-click attempt failed once; no cause was proven, and it did not recur in the final three repeated window runs or delivery run. Do not claim real-pointer validation from these results.
- Delivery: dist/FeatherMD.exe, 718848 bytes (702 KiB), SHA-256 8192f9c347dbddebf8bb9213bc73faeb550405de82894b2ef6a3ad4fa99e6789. dist/FeatherMD-0.1.3-header-ui-windows-x64.zip, 579683 bytes, SHA-256 c70d0736274b49826e64b9eadebf7d7530f95b0b5827ee67f9251a7b608a2b6e. Build/delivery/ZIP EXE hashes match. All previous versioned ZIPs are preserved. EXE version remains 0.1.3; this is a local UI revision.
- Boundaries: native messages and in-process snapshots, not physical mouse automation, full accessibility validation, real system-wide theme changes, physical multi-monitor DPI tests or Windows 10 testing. The diagnostic child-EDIT print can show standard white in dark search snapshots because its parent color callback re-enters an active App borrow; do not treat that snapshot artifact as proof of the visible EDIT's theme. No association/registry changes, actual browser launch or new large-file benchmark were performed. Delivery demo first layout 106.71ms is only one local small-file sample.

## 2026-10-12 — About redesign and project/support badges (local)

- Preserved the user's existing uncommitted v0.1.3 work. More now has an explicit “关于 FeatherMD…” entry (F1). Fixed destinations: https://github.com/Lianzy-Baimiao/FeatherMD and https://ifdian.net/a/lianzy; the support URL and flat-square MIT/purple 爱发电 badges were checked against WowAltBoard's README.
- After the user rejected the initial stock Task Dialog appearance, replaced it with a native modal card layout: feather mark, clear title/version hierarchy, GitHub/support cards, optional shortcuts, safe Return to reader action and custom frame. It uses the reader's light/dark/sepia/system preference, with Windows theme notifications and DPI/work-area fitting; no web renderer, remote assets or additional Cargo dependencies.
- Standard STATIC/BUTTON children retain names, focus and keyboard behavior. A native regression exposed Enter on a focused BS_OWNERDRAW link closing About instead of activating the link: the default dialog manager was choosing the safe default action. The button subclass now explicitly requests/handles Enter; Tab → GitHub → Enter reaches the confirmation. Escape, F1, help expansion/collapse and repeated open/close also pass.
- The deferred About launch and nested link confirmations release state borrows before entering modal loops. Both fixed external URLs require confirmation, default Cancel; tests cancel rather than launching the browser. No production association registration/unregistration, UserChoice write or default-app change was performed.
- Hidden cross-process PrintWindow sometimes returned success with a completely black image. Extracted the existing reader's opt-in, in-process GDI snapshot helper for reuse by About, including real child-control painting; added background-pixel checks. Final light/dark/expanded/keyboard-focus captures were inspected, and README/package include the final light/dark previews. The capture seam remains inert without FEATHERMD_TEST_SNAPSHOT.
- Final validation: fmt check, strict all-targets Clippy, 39 tests and MSVC static-CRT release build pass. About passes in light/dark/sepia/system; GDI 19→19 over eight additional open/close cycles per run. Reader UI, window (49 hit tests), scrollbar and navigation shapes pass: headings 454/535 pages, wrapped 473/523 pages, each with 13 native font sizes. Delivery EXE also passes light/dark About and UI smoke (GDI 31→31). Demo first layout 72.30ms is one local small-file run, not a cold-start or large-file benchmark.
- Delivery: dist/FeatherMD.exe, 709120 bytes (~693 KiB), SHA-256 2141282b47398c96383e4e004f8b3ec419fffb3c76ebd1e361f7c1db4fc45619. dist/FeatherMD-0.1.3-about-ui-windows-x64.zip, 568242 bytes, SHA-256 9855dc1876db833245208970cc5f9d9c321576a5e813c5b7477650b636340ac8. Build/delivery/ZIP EXE hashes match; package includes README and its About screenshots. All pre-existing ZIPs are preserved. EXE version remains 0.1.3; this is a local UI revision, not a GitHub release. No commits/pushes/tags/publication in this turn.
- Boundaries: native message tests, not desktop pointer injection or full assistive-technology validation. No actual browser handoff, system-wide theme change, physical multi-monitor DPI transition or Windows 10 run was exercised. Confirmation/error dialogs remain standard Windows dialogs. Large real-file benchmarks were not repeated.

## 2026-10-12 — local header switches and association verification

- Continued the existing uncommitted v0.1.3 work without resetting user edits. Built the always-visible preview/source and light/dark/system segmented controls; removed their dependency on the More menu. New settings default to system, explicit choices persist, and WM_SETTINGCHANGE / WM_THEMECHANGED refresh the resolved palette.
- Added a regression before fixing a related reading-position bug: `scripts/scrollbar-smoke.ps1` reported `Header command 209 moved the reading position: wrapped row 25 -> 0`. Cause: theme commands called the general persistence path, which rebuilt layout from the physical source byte. Theme-only changes now save preferences and repaint without relayout; repeated selection of the current preview/source mode does nothing. All six tested theme/current-mode commands preserve the wrapped row.
- On this host at 120 DPI, the scrollbar test initially missed the thumb because its caller used virtualized coordinates. The script now opts into per-monitor-aware physical coordinates and restores its prior DPI context; it then reproduced the theme regression and passes after the fix.
- Validation: fmt, strict all-targets Clippy, 38 tests, MSVC static-CRT release build, UI smoke, window smoke, scrollbar smoke and both navigation shapes pass. Window smoke: 49 frame hit tests, 800x600 physical minimum at 120 DPI (640x480 logical), top switches including narrow layout and restart persistence, source byte 941400 retained on resize. Headings navigation 454/535 pages; wrapped navigation 473/523 pages; both exercise 13 actual font sizes.
- Delivery EXE also passed UI/window smoke. GDI 27→27. Demo first layout 87.53ms is a single local run, not a cold-start or large-file measurement. Final light/dark/narrow/welcome captures were inspected; README previews now match the local build. No large real-file benchmark was repeated in this turn.
- Association symptom did not reproduce on the current host: read-only SHAssocEnumHandlers includes FeatherMD for .md, and UI Automation inspected FeatherMD in the Windows Default apps application list and its dedicated page. Registration still points to an older executable outside the project. The user clarified that they personally clicked to choose FeatherMD as default during inspection; do not attribute that choice to the agent. No production registry registration/unregistration was performed in this turn; the existing registry tests use isolated subtrees.
- Added `scripts/association-inspect.ps1`, a read-only Shell candidate probe, and documented fixed-location registration, re-registration after moving, Settings refresh and browse-to-EXE fallback. Registration is still opt-in, per-user and never writes UserChoice or extension defaults. Do not claim a reproduced registration root cause or universal Windows compatibility from this host.
- Local delivery: `dist/FeatherMD.exe`, 696832 bytes (680 KiB), SHA-256 `1def124a0a90f8edc9a66d07a4ade24ed7cdb5b589369fa16f1d84259da30fd2`; identical to target/release. `dist/FeatherMD-0.1.3-top-switches-windows-x64.zip`, 524522 bytes, SHA-256 `aa6e597648db6030863fed6b9eea0c4704e5c7ba08dff872cce1a98004591874`. Preserved all pre-existing versioned ZIPs including the earlier 0.1.3 archive. This is a local revision, not a new GitHub publication; no commits, pushes, tags or releases were made.
- Boundaries: native smoke uses hidden windows/messages, not real pointer input or a real system-wide theme change. Theme mapping is covered by pure tests and native refresh/persistence checks; physical monitor transitions and Windows 10 Settings UI remain unverified. dumpbin reports Windows-provided DLLs only.


## 2026-10-12 — v0.1.2 local UI, scroll and outline delivery

Author / company / last editor: lianzy.

- User requested working scrollbars, a non-native border, a cleaner UI, a collapsible heading outline, and removal of the separate toolbar. Implemented a single 48px custom title bar with file title, find, open, more and window controls; remaining actions live in the popup menu. The outline collapses from its own heading and expands from the left content edge; no duplicate toggle remains in the application title bar. Ctrl+B toggles the outline and its preference persists across restarts.
- Reproduced old v0.1.1 twice with actual pointer input: dragging the native thumb to the middle left checkpoint 00000 / 0%. A one-change probe releasing the App borrow before DefWindowProc reached checkpoint 02457 / 49.1%, confirming native tracking reentrancy as the original cause.
- Custom scrollbars share actual viewport state, support both axes and page clicks, use 64-bit geometry, preserve wrapped-paragraph progress and invalidate stale page requests. A regression caught WM_LBUTTONDBLCLK (click followed quickly by drag): old code stayed at byte 0; the fixed branch reaches byte 1,834,737 in the 3,675,600-byte fixture. WM_CANCELMODE releases capture.
- scrollbar-smoke PASS: both axes, drag before release, double-click drag, wheel, track paging, capture loss, three rapid out-and-back seeks, short-document hiding and a 4,060-byte single paragraph with 28 wrapped-row positions.
- window-smoke PASS: 39 frame hit tests, no WS_CAPTION/WS_VSCROLL/WS_HSCROLL or attached native menu, 640x480 client layout at 96 DPI, byte 941,400 preserved across resizing, 1,200 headings, outline click at byte 7,845, independent outline wheel, hide/reopen through actual buttons and preference persistence. Search input remains clear of the collapsed outline button at narrow width. Captured light/dark/search/narrow/empty states.
- Core checks: formatting, warning-free Clippy, 34 tests and static-CRT MSVC release build passed. Navigation regression: heading shape 357 forward / 419 backward pages; wrapped shape 251 / 280 pages, both with 13 actual font settings.
- UI smoke: 29→29 GDI handles. Real 1 GiB Markdown smoke: 107.86ms first layout (one warm-cache sample), 5.42MiB private memory, 19.54MiB working set, GDI 28→29. Not a cold-start guarantee; first-layout timing is measured before waiting for the outline worker.
- Actual pointer checks separately exercised a fresh Release instance reaching 50.7% / checkpoint 02536 by thumb drag. A diagnostic build also verified custom maximize/restore (1080x820 ↔ 2240x1368). Earlier reused-window pointer attempts did not always move content; this led to the explicit double-click regression above. Only successful observations and native-message assertions count as verified evidence. All temporary input tracing was removed.
- Outline limits: separate cancellable worker, 256KiB reads, up to 16MiB scanned, 4096 ATX headings, 160 characters per label, 4096-byte long-line prefix. Code-fence content is excluded. Limits are shown in the sidebar; Setext headings are not indexed. F5 rebuilds the outline. Reading and full-text search continue beyond its range.
- Local delivery: dist/FeatherMD.exe (684,544 bytes) and a new v0.1.2 ZIP (468,999 bytes), including README previews. Build/standalone/ZIP executable hashes match: 23c1b618040df097d043f819eb7aad5e38f31df3cd0689b1607e2f6becff54d5. ZIP hash: a8a4b47657319b73068bb90f088601e7e56eca73ffea53b421ab44a6b012f924. Existing v0.1.0/v0.1.1 packages/tags are preserved. Executable metadata explicitly names lianzy; dumpbin confirms only Windows-provided dependencies. Final packaged EXE passes UI smoke (GDI 29→29). GitHub publication was completed at the user's explicit request; see the record below.
- Remaining platform checks: real multi-monitor DPI transitions, Windows 10 visual verification, snap layouts and isolated default-association integration. Popup menus and file dialogs remain OS-provided; the application frame and scrollbars are custom. Full document/outline accessibility semantics remain future work.

## v0.1.2 publication record — 2026-10-12

- Public latest release: https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v0.1.2
- Tag/source: 8df8bbb5e22a75eb0a2344d223a74bb6e6c38a7f. GitHub Actions run 38198642207 passed on Windows and Ubuntu, including all native smoke tests: https://github.com/Lianzy-Baimiao/FeatherMD/actions/runs/38198642207
- README now contains final light, dark, collapsed-outline and welcome previews. Versioned release preview URLs resolve against v0.1.2.
- Published EXE, ZIP and SHA256SUMS.txt were downloaded again; all SHA-256 hashes match the local distribution and GitHub asset digests. Package sizes and hashes are recorded above. The tag points to the tested source commit; existing releases and tags remain untouched.
- Follow-up work must start from remaining bounded improvements, not repeat the completed scrollbar, frame or outline-control placement work. The current Actions run also warns about deprecated Node 20 actions; update these separately against current upstream releases.

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
