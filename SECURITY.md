# Security Policy

## Supported versions

Slate ships as a portable binary from the [Releases](https://github.com/divyanshudhruv/slate/releases) page. Only the latest release is supported. If you build from source, use the tip of `main`.

## Reporting a vulnerability

Please report security issues privately. Do not open a public issue.

Use GitHub's private vulnerability reporting: open the **Security** tab of the repository and click **Report a vulnerability**.

Include as much of this as you can:

- What the issue is and what an attacker could do with it.
- Steps to reproduce, or a proof of concept.
- The Slate version (the release tag, or the `version` in `Cargo.toml`).
- Your Windows version.

## Scope

Slate is a Win32 desktop app that hosts other windows. Areas of interest:

- Window ownership and message handling (`src/win/`).
- The single-instance janitor and its crash-recovery state (`src/win/janitor/`).
- Config and saved-set parsing (`src/config/`).
- The self-install path and its registered-exe record (`src/win/update.rs`, `install.json` in the data directory).
- The tray icon, system hotkeys, and IPC.

Out of scope: anything that needs an already-compromised machine, and the limitations already listed in the README.

## Threat model

What Slate trusts, what it does not, and what a bug costs.

### Assets

- The hosted windows. Slate takes ownership, changes styles, and restores them on exit or crash. A wrong restore leaves someone else's app unusable.
- The install directory, `%LOCALAPPDATA%\Programs\Burntfern\Slate`, which holds `slate.exe`.
- The data directory, `%LOCALAPPDATA%\Burntfern\Slate`. Settings, saved sets, session state, logs, and the install record live there.
- The registered hotkeys, which only see the chords Slate binds and only while one of its windows has focus.

### Trust boundaries

- Same-user processes are semi-trusted. They can read and write the data directory, so Slate treats state files as input from a peer, not as gospel: recovery files are rejected when the recorded Slate pid or the window owner does not match.
- Elevated windows are out of reach unless Slate is elevated. Slate never launches apps and never asks for elevation on its own.
- The network is untrusted. The only request Slate makes is a rate-limited read of GitHub's release JSON; nothing is downloaded, verified, or executed.
- Config files are user-editable and may be hand-mangled or written by an older or newer build. Unknown fields survive a save, mistyped fields fall back to defaults with a log warning, and a file newer than the schema is left in place with saving disabled.

### Realistic attackers

- A malicious local process running as the same user. It can already read the user's files, so Slate only tries to stop it from steering window handles or install files out from under the app.
- A hostile network answering the release check. It can only change the version string and the link you open yourself.
- Not in scope: a compromised Windows session, malware with admin rights, or anything that can already inject into other processes.

### Known limits

- Saved sets and settings are plain JSON on purpose; they are not secrets.
- The window picker uses a short-lived keyboard hook; it passes every key it is not handling straight on and only acts while Slate has focus.
- Crash recovery runs at the next start, not at crash time: after a hard crash the hosted windows stay as they are until Slate is started again.
- A newer release replaces `slate.exe` only when you run the download yourself; Slate does not download, verify, or swap builds on its own.
- Update checks are rate-limited by GitHub, not by Slate.

## What to expect

This is a small project maintained in spare time. I aim to acknowledge reports within a few days, and I will credit reporters in the release notes unless asked otherwise.
