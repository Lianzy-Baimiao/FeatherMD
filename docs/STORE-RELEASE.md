# Microsoft Store build and submission

Status (2026-10-26): Submission 1 (`1152921505702158375`, 1.0.10.0) was published
2026-10-20. **Submission 2** (`1152921505702218732`, 1.0.12.0) passed
certification and is the version in the Store. **Submission 3**
(`1152921505702229604`) with the frozen **1.0.13.0** MSIX (Cargo 0.1.13: plain text,
logs and legacy encodings; `.txt` / `.text` / `.log` added to Open With) was
submitted for certification on 2026-10-26 and publishes automatically when it passes
(the owner asked for an unattended release including the Store); it was accepted,
not yet certified. A 1.0.11.0 candidate passed WACK (run 40044912773) and lifecycle
(run 40044915701) but was never uploaded. Never reuse an uploaded version. Do not
reuse LiteView's identity or ratings. See PROGRESS.md for exact evidence and
limitations.

## Build

Use the MSVC static-CRT toolchain, Python 3.11+ with Pillow, and Windows SDK
MakeAppx. Keep machine-specific SDK paths outside the repository.

```powershell
cargo +stable-x86_64-pc-windows-msvc build --locked --release --features store --target-dir target/store
python -m unittest discover -s packaging -p test_msix.py
python packaging/msix.py --binary target/store/release/feathermd.exe --version 1.0.10.0 --test-identity --output dist/store-test-1 --makeappx makeappx.exe
```

The packer requires a new output directory, checks x64 PE and the Store-channel
marker, rejects dynamically linked VC/GNU runtimes, generates icons from the
existing ICO, and invokes MakeAppx with schema validation enabled. It verifies
every archive file byte-for-byte against a strict layout. No settings, fixtures,
certificates, dependencies or developer environment files are included.

Output is **unsigned**. `-TEST.msix` uses `FeatherMD.StoreTest` and is never a
submission candidate. No script here installs a certificate on the developer's
computer. Local trust, installation, upgrade and uninstall must be tested in an
explicitly disposable environment. MakeAppx success is not Store certification.

## Runtime differences

- `--features store` disables GitHub checks even when run outside a package.
  Installed package identity also activates these safeguards in a normal build.
  The packer nevertheless requires the Store build so an extracted EXE does not
  accidentally switch channels.
- AUMID is `<actual family>!FeatherMD`. Associations are declared for `.md`,
  `.markdown`, `.mdown`, `.mkd` and, from 1.0.13.0, a second group for `.txt`,
  `.text`, `.log`; Windows chooses defaults after user confirmation.
  No UserChoice writes, administrative escalation or portable registry changes.
- Packaged settings use unredirected LocalAppData + `Packages/<family>/LocalState`.
  Failure to determine the identity/folder stops startup instead of writing to a
  shared fallback. An unpackaged Store preview uses `FeatherMD/StorePreview` under
  LocalAppData and ignores adjacent portable INIs.
- The update page opens Microsoft Store's Downloads/Library URI. It cannot report
  whether the Store installed an update. No invented product ID or GitHub upgrade.
- Privacy notice is available offline under Settings > Updates. The full policy
  is included in both ZIP and MSIX. Images retain the existing individually
  confirmed HTTP(S) behavior; HTTP's cleartext risk is explicitly disclosed.
- Native renderer, bounded file access, external-editor and local-image safety
  remain unchanged. No new broad-file-system or internet capabilities.

## Production identity and version

After the user confirms reserving FeatherMD in Partner Center, copy this product's
Identity Name, Publisher and display names to ignored
`packaging/store-identity.local.json`, based on the example. Set
`partner_center_verified` to true only after comparing them with Partner Center.

Replace `--test-identity` with `--identity packaging/store-identity.local.json`.
The packer verifies syntax and the explicit attestation, **not the account itself**.
The Store signs accepted packages for distribution. GitHub portable EXEs remain
unsigned unless separately signed with a trusted code-signing service/certificate.

MSIX versions: 1.0.10.0 was the first upload (Cargo 0.1.10); 1.0.12.0 carries
Cargo 0.1.12 (1.0.11.0 was built but never uploaded); 1.0.13.0 carries Cargo 0.1.13
(plain text and logs). Increment the third component for each new candidate/upload,
independent of Cargo's version: a replacement candidate for the same Cargo version
still takes the next number. Components are <=65535, the first is nonzero and the
fourth stays 0. Never overwrite/reuse an uploaded version.

## Listing draft

Name: **FeatherMD**, reserved with user confirmation on 2026-10-17.
Store ID: `9MW2CTPBSGX7`; package name: `ice.FeatherMD`.
The product is published (1.0.10.0 since 2026-10-20).
Selected category: Productivity. Supported languages: Simplified/Traditional
Chinese, English, Japanese.
Architecture: x64. Package minimum: Windows 10 build 19041. This is a manifest
target, not a claim of completed Windows 10 testing.

中文简介：轻巧的 Markdown 阅读器。原生窗口、按需读取，打开长文档也不必先载入全文。

中文描述：FeatherMD 羽读专注于阅读本机 Markdown 文件。支持目录导航、搜索、预览与源码切换、代码高亮、表格、20 款配色和阅读字体调整。文档只读；需要修改时，可交给你选择的编辑器。图片默认不自动显示，网络图片每张单独确认。商店版的安装和更新由 Windows 与 Microsoft Store 管理。部分复杂 Markdown、HTML 和图片格式不支持；正文与目录尚未提供完整屏幕阅读器语义。

English: A lightweight native Markdown reader with bounded, on-demand file access.
Read local files with outline navigation, search, preview/source modes, code
highlighting, tables, 20 skins and adjustable fonts. Documents stay read-only;
hand them to your chosen editor to make changes. Local images are off by default;
remote images require individual consent. Microsoft Store manages this edition's
installation and updates. Not every Markdown/HTML/image format is supported.
Full screen-reader semantics for the document and outline are not yet provided.

Verified public privacy URL:
`https://github.com/Lianzy-Baimiao/FeatherMD/blob/main/docs/PRIVACY.md`
HTTP 200 and content equality were verified on 2026-10-17; HTTP 200 was rechecked
on 2026-10-18. Check availability again before submitting. Support URL: repository
Issues.
Do not claim all accessibility requirements are met. Do not reuse LiteView's IARC
answers or screenshots. Complete the questionnaire for FeatherMD's actual behavior.

## Submitted materials and screenshots

- Uploaded `dist/store-1.0.10.0-candidate/FeatherMD-1.0.10.0-x64.msix`, 551,084
  bytes, SHA-256 `ba17aec5ba13880c794e767640104f8ae41771da0a60b751f9f013c7538cb80f`.
  Desktop only; future device-family expansion is disabled. Do not replace these
  verified bytes casually. Later uploads: 1.0.12.0 (Submission 2, SHA-256
  `9c4eb44c…`) and `dist/store-1.0.13.0-candidate/FeatherMD-1.0.13.0-x64.msix`
  (Submission 3), 574,736 bytes, SHA-256
  `34a7d6332b64cdee2ac89db1abecc7a9bf724e1dbd30c93e605b5c5e57a348e2`.
- Four localized listings (zh-CN, zh-TW, en-US, ja-JP) each have four desktop
  screenshots and five feature bullets. Capture output is
  `test-artifacts/store-listing-39047863992/`: light, dark, source and tables for
  each language, with hashes in `provenance.json`. All are 1600x1000 real native
  captures of the frozen WACK executable, not mockups or a newer build.
- `scripts/store-listing-capture.ps1` and `.github/workflows/store-listing.yml`
  capture on a disposable Windows runner only. Run 39047863992 passed; the prior
  run 39047719721 failed on the runner's limited display resolution. The fix
  changes only the disposable display before capture. No local GUI is launched.
- User's final market instruction is worldwide, including China, not China-only.
  All 240 current markets are selected and all 240 price-table rows were verified
  as zero. Base currency is CNY; public/discoverable, no trial or promotion.
  Automatic expansion to future markets is off. Values persist after reopening,
  but the draft overview did not show pricing completion text after IARC was
  saved. Submission was subsequently accepted; the missing text did not block
  submission. Do not invent a Chinese ISBN or other approval
  from the market selector's generic label.
- Submission 1's release hold was manual and was confirmed again by its submitted
  status page: certification did not automatically publish it. The
  500-character restricted-capability field contains a saved
  454-character explanation. Full testing notes are saved separately and were
  verified after a page reload.
- IARC answers: Other apps; no packaged rating-sensitive
  content, native user communication, curated online content, age-restricted
  commerce, location sharing, in-app digital purchases, redeemable rewards,
  browser/search-engine function, primary news/education function or physical
  distribution. The user authorized publisher-name/email sharing and rating
  generation; Preview ratings returned IARC 10.3 with IARC/Microsoft/PEGI 3+,
  ESRB E, USK Everyone, Brazil L, Chile TE and Russia 0. The mandatory checkbox
  combines IARC terms acceptance with a declaration of legal adulthood. The user
  confirmed it; the checkbox was selected and the rating saved. The current
  rating ID is still shown as pending, not an issued certificate number.

## Reviewer notes

These core notes, test steps and supplemental-test limitations are saved in
Partner Center's Additional Testing Info. They are not evidence of approval.

FeatherMD is a native Rust Win32 desktop reader, `packagedClassicApp` / `mediumIL`.
`runFullTrust` is required for its existing Win32 renderer, reading user-selected
local files and launching a user-selected external editor. It does not request
administrator elevation. No login or payment is required. Optional support links
lead to third-party payments and do not unlock features.

Test: launch from Start; open a local Markdown file containing Unicode/spaces in
its path; navigate/search; switch language/theme; open the `.txt` novel and `.log`
in `docs/store-samples` (chapters; time index with error/warning counts); use the
Windows Open With picker for each of the seven declared extensions; save settings;
restart; update/uninstall the package. Remote images require clicking and
confirming a URL; F5 revokes that consent. Open Settings > Updates to find the
Store update entry and offline privacy notice.

Where another app is already the default (Notepad for `.txt`), Windows may answer
Open With → FeatherMD with its own "Select an app" dialog listing FeatherMD; the
notes tell reviewers to choose FeatherMD and "Just once". This was observed for
`.txt` and `.text` on the Windows Server 2025 runner (run 40242268129 captures);
`.log` and the Markdown types opened FeatherMD directly.

The saved notes also preserve the production WACK overall PASS with one optional
Blocked executables FAIL, the ShellExecuteW/ExW and REG explanation, and links to
the current WACK and lifecycle runs (40240292473 and 40244480948 for 1.0.13.0).
They explicitly do not claim completed Windows 10 validation, full accessibility or
prior Microsoft certification.

## Remaining release gates

- 1.0.12.0 passed certification and is live. 1.0.13.0 is in certification as
  Submission 3 with "What's new" in all four listings and automatic publication;
  pricing, markets and age rating are unchanged. Check the actual result. If
  certification rejects it, read the actual report before changing the frozen
  candidate; a fix needs a new version number. Do not repeat completed
  IARC/age/data-sharing consent.
- The listing description, feature bullets and screenshots still describe a
  Markdown reader only; updating them for text and logs is a separate change.
- Partner Center from this machine: headless browsers get Akamai "Access Denied".
  A separate Edge instance with its own user-data folder signs in through the
  Windows account; drive it over a local DevTools port. Its sync pulls the owner's
  extensions, so change nothing in that profile and delete it afterwards.
- Third-party support is declared. Its public page describes support for creative
  work, with no observed FeatherMD feature unlock. Actual payment security and
  transaction terms have not been audited; do not claim verified compliance.
- Preserve the recorded lifecycle and WACK evidence and optional-warning rationale.
- Real Store signing/delivery; Windows 10 and accessibility limitations.
- Verify final Store listing, age rating, target markets and delivered package
  after approval/publication. Submission evidence is in
  `test-artifacts/feathermd-submitted-20261018.txt` and the matching `.jpg`.
- Policies were rechecked on 2026-10-24 for this update: version 7.20 (published
  2026-09-15, in effect from 2026-10-22) changes XBOX branding and game rules,
  age-rating accuracy (11.11) and game-mod UGC moderation (11.12). Nothing new
  applies to this update, and the content behind the age rating is unchanged.
  They were not rechecked for 1.0.13.0 (the policy pages could not be fetched from
  that session); 1.0.13.0 only adds reading local text and log files.
- Submissions 2 and 3 publish automatically after certification, as the owner
  asked; no monitoring task was configured.

## Testing without interrupting the desktop

All native smoke scripts refuse local execution by default. With current user
permission and exactly one secondary monitor, pass `-SecondaryMonitor`. The EXE
must contain the rebuilt test-host marker. A UI-thread-only CBT hook and window
subclasses constrain top-level windows before display, suppress activation and
record `test-host.txt`. Synthetic tests do not sample physical Ctrl/Shift state
from a game on the primary screen. No global hooks, cursor movement or keyboard
injection are used. If monitor discovery/containment fails, startup returns an
error without opening another dialog.

This mode **cannot validate real keyboard focus**. About's Tab/Enter test is
explicitly deferred, not counted as covered. Run the unmodified active-focus
path in the GitHub-hosted Windows CI job. `-AllowDesktopUi` bypasses the local
restriction and requires separate user permission; never use it just to make a
test pass.

The manually dispatched `Store candidate validation` workflow prepares two test
versions and runs GUI, Shell activation of all seven extensions, installation,
settings-preserving upgrade and uninstall on a disposable Windows 2025 runner.
Its short-lived signing certificate/private key and signed packages are not
uploaded. It is not WACK and not Store certification. Run 39045655691 passed,
including exact-AUMID Shell Open With activation rather than a UWP file contract.
When Windows answers an activation with its "Select an app" dialog (OpenWith.exe),
the dialog is not reachable by UI Automation on the runner, so the test records a
capture and opens the same file directly instead of clicking; it still requires
each declared group to start the reader through the Shell at least once and every
UserChoice default to stay unchanged. Run 40244480948 passed this way (`.txt` and
`.text` showed the dialog).

The separate manual `Supplemental Store WACK` workflow builds the same Store
channel on a fresh Windows 2025 runner. Its default is a test identity. After
verifying FeatherMD's own identity page, pass the local identity JSON as the
`identity_json` dispatch input to build the production-identity candidate. This
input is public package metadata, not signing credentials. The test script only
accepts FeatherMD's test identity or its verified production name/publisher. It verifies the
package/binary hashes, obtains WACK from a hash-pinned Microsoft-signed SDK
installer when needed, and tests a temporary signed copy. Signing keys are
nonexportable and stay off the uploaded reports; trust is removed in cleanup.
The original unsigned package must remain unchanged. Raw XML, command exit codes,
environment, hashes and a conservative summary are retained. Missing, partial,
unknown or not-run reports do not pass. Optional test failures remain visible and
require manual review, even when WACK's overall result is PASS. The report parser
and pinned SDK source reuse the previously inspected LiteView validation tooling,
not LiteView's package identity or test results.

The uploaded candidate is the original unsigned package, not the temporary
self-signed test copy. Prefer the exact production candidate from a verified run
for submission instead of rebuilding different bytes locally. This is still
supplemental Windows Server testing, not supported Windows client testing or
Microsoft's certification. Local Python/PowerShell parsing is not a WACK run.
