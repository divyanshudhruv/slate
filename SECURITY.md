# Security Policy

## Supported versions

Slate ships as a portable binary from the
[Releases](https://github.com/divyanshudhruv/slate/releases) page. Only the latest
release is supported. If you build from source, use the tip of `main`.

## Reporting a vulnerability

Please report security issues privately. Do not open a public issue.

Use GitHub's private vulnerability reporting: open the **Security** tab of the
repository and click **Report a vulnerability**.

Include as much of this as you can:

- What the issue is and what an attacker could do with it.
- Steps to reproduce, or a proof of concept.
- The Slate version (the release tag, or the `version` in `Cargo.toml`).
- Your Windows version.

## Scope

Slate is a Win32 desktop app that hosts other windows. Areas of interest:

- Window ownership and message handling (`src/win/`).
- The single-instance janitor and its shared state (`src/win/janitor/`).
- Config and saved-set parsing (`src/config/`).
- The self-update flow and its registered-exe record (`src/win/update.rs`,
  `install.json` in the data directory).
- The tray icon, global shortcuts, and IPC.

Out of scope: anything that needs an already-compromised machine, and the
limitations already listed in the README.

## Threat model

What Slate trusts, what it does not, and what a bug costs.

### Assets

- The hosted windows. Slate takes ownership, changes styles, and restores them
  on exit or crash. A wrong restore leaves someone else's app unusable.
- The self-updater. It replaces `%LOCALAPPDATA%\Programs\Burntfern\Slate\slate.exe`
  and relaunches it.
- The data directory, `%LOCALAPPDATA%\Burntfern\Slate`. Settings, saved sets,
  session state, logs, and the install record live there.
- The global keyboard hook, which sees every keystroke while Slate runs.

### Trust boundaries

- Same-user processes are semi-trusted. They can read and write the data
  directory, so Slate treats state files as input from a peer, not as gospel:
  janitor payloads are HMAC-signed with a per-install key and rejected when the
  signature, the Slate pid, or the window owner does not match.
- Elevated windows are out of reach unless Slate is elevated. Slate never
  launches apps and never asks for elevation on its own.
- The network is untrusted. Update checks read GitHub's JSON; downloads are
  verified against the release checksum before the swap helper runs.
- Config files are user-editable and may be hand-mangled or written by an older
  or newer build. Unknown fields survive a save, mistyped fields fall back to
  defaults with a log warning, and files newer than the schema are backed up
  instead of being overwritten.

### Realistic attackers

- A malicious local process running as the same user. It can already read the
  user's files, so Slate only tries to stop it from steering window handles or
  update payloads out from under the app.
- A hostile network on the update path.
- Not in scope: a compromised Windows session, malware with admin rights, or
  anything that can already inject into other processes.

### Known limits

- Saved sets and settings are plain JSON on purpose; they are not secrets.
- The keyboard hook forwards unmatched keys untouched but sees them first.
- Crash recovery needs the janitor payload to be readable by the next Slate
  start; it is dropped when the pid, signature, or owner check fails.
- Update checks are rate-limited by GitHub, not by Slate.

## What to expect

This is a small project maintained in spare time. I aim to acknowledge reports
within a few days, and I will credit reporters in the release notes unless asked
otherwise.
