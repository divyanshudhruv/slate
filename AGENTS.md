Windows-only tab host: hosts real Win32 windows as tabs in one gpui window.
Built with Rust, `gpui`, and `ely_gpui_component`.

## Setup
- Toolchain is pinned in `rust-toolchain.toml` (rustup installs it). `aarch64-pc-windows-msvc` is only used for release builds; use `x86_64` day-to-day.
- Build: `cargo build`
- If you hit `os error 1455` (paging/commit limit), try `cargo build -j 1`.

## Checks before finishing
```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```
- `cargo test` covers `src/core`, `src/config`, the pure helpers in `src/ui` and `src/win`, and the comment-style lint in `tests/`. The interactive UI and Win32 paths still need a run of `target\debug\slate.exe`. Kill any running `slate.exe` first (it locks the exe).

## Layout
- `src/core/` - pure state (groups, tabs, saved sets, install/update planning) + tests
- `src/config/` - settings/load/save/schema for `%LOCALAPPDATA%\Burntfern\Slate\`, seeded from `configs/`
- `src/controller/` - `Cmd` dispatch and UI snapshot; owns the win layer
- `src/win/` - Win32 (enumeration, hotkeys, host lifecycle, crash recovery, tray, icons, capture, release install)
- `src/ui/` - gpui views (host window, tab bar, overlays, panels)

## Rules
- One-way flow: UI -> `Cmd` -> controller -> Win32 -> core state. UI never calls Win32 directly.
- Don't add deps without asking. Prefer `ely_gpui_component` primitives.
- Don't vendor/patch `ely_gpui_component` (git dep pinned by rev).
- Use `-` not em dashes, `->` not arrows in docs/comments.
- Keep one sentence per line in docs and comments. Never hard-wrap prose, and use `//`/`///` with no `/* */` blocks.
- Conventional Commits. Only `feat`, `fix`, or a breaking change cut a release (via release-please on `main`). Version lives in `Cargo.toml`.
- Don't commit unless asked.

## Notes
- Windows only. Uses Win32 window ownership, DWM frame control, GDI icon extraction.
- Elevated windows only hosted if Slate is elevated. Slate never launches apps, only hosts running ones.
- Hosted windows draw above the host, so Slate menus can be covered by the active member.
- Crash recovery is deferred: `Janitor::persist` writes `recovery-<pid>.json` (plain JSON) while members are hosted, and `sweep_stale` replays it at the next start. There is no child process.
- The window picker hook (`src/win/pick.rs`) is the only keyboard hook left; it is installed only while a pick is in flight and only acts when Slate has focus.
- System hotkeys are registered only while one of Slate's windows has focus (`Controller::sync_worker` plus the foreground hook in `src/win/shortcut.rs`), and they are unregistered the moment focus leaves.
- The update check is a read-only GitHub API call; Slate never downloads a release. Running a downloaded release installs it (`bootstrap`) and relaunches, and that refuses to run while another copy owns the data folder.
