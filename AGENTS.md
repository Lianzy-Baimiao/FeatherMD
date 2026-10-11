# FeatherMD contributor instructions

- Keep the Windows GUI native and dependency-light. No browser engine, network fetching, or full-file allocation.
- File reading and searching must remain off the UI thread. Preserve the 128 KiB read window, 4096-byte long-line segments, 2048-line layout limit, and cancellable 256 KiB search chunks.
- Do not use mmap: externally edited/truncated files must stay safe.
- Never silently take default associations or write UserChoice. Registration is opt-in, per-user, and must preserve other applications.
- Do not execute Markdown HTML, scripts, local links, or remote images. External HTTP(S) links need a user confirmation.
- Tests before release: cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test; cargo build --release; scripts/ui-smoke.ps1.
- Read docs/PROGRESS.md before continuing. Record precise evidence and limitations; distinguish warm-cache parsing, GUI first layout, real files, and sparse files.
- Do not publish ignored fixtures, local settings, local build environment scripts, or other workspace data. Never force-push.
- Use MSVC for distributable Windows binaries with static CRT. The local host has a GNU default toolchain; use cargo +stable-x86_64-pc-windows-msvc. If needed, source ignored target/dev-env.ps1 on this development machine. Do not commit machine-specific paths.
