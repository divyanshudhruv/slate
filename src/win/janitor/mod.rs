mod wire;

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, IsWindow, GWL_HWNDPARENT};

use super::host::{unregister_member, StylingState};
use super::instance::process_alive;
use wire::{decode, encode};

#[derive(Default)]
pub struct Janitor {
    path: Option<PathBuf>,
    last: Option<Vec<u8>>,
}

impl Janitor {
    pub fn new() -> Self {
        cleanup_legacy();
        // Restores whatever a previous crash left behind; the windows stay where the session put them.
        sweep_stale();
        Self {
            path: Some(state_path()),
            last: None,
        }
    }

    /// Drops the recovery file after a clean restore, so the next launch has nothing left to replay.
    pub fn disarm(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
        self.last = None;
    }

    pub fn persist(&mut self, slate_hwnd: isize, members: &[(isize, StylingState)]) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let Some(bytes) = encode(slate_hwnd, members) else {
            return;
        };
        if self.last.as_deref() == Some(bytes.as_slice()) {
            return;
        }
        if write(&path, &bytes) {
            self.last = Some(bytes);
        }
    }
}

pub fn state_path() -> PathBuf {
    let dir = crate::config::paths::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("recovery-{}.json", std::process::id()))
}

/// Older builds kept a signed binary payload and a DPAPI key; neither is read any more.
fn cleanup_legacy() {
    let dir = crate::config::paths::data_dir();
    let _ = std::fs::remove_file(dir.join("janitor.key"));
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("janitor-") else {
            continue;
        };
        let pid: u32 = rest
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        if pid != 0 && pid != std::process::id() && process_alive(pid) {
            continue;
        }
        let _ = std::fs::remove_file(entry.path());
    }
}

pub fn sweep_stale() {
    let dir = crate::config::paths::data_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let me = std::process::id();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = parse_state_name(&name.to_string_lossy()) else {
            continue;
        };
        if pid == me {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        if process_alive(pid) {
            continue;
        }
        let path = entry.path();
        let outcome = match std::fs::read(&path) {
            Ok(bytes) => restore(&bytes),
            Err(err) => {
                tracing::warn!("could not read {}: {err}", path.display());
                continue;
            }
        };
        match outcome {
            Some(result) if result.complete => {
                let _ = std::fs::remove_file(&path);
            }
            Some(_) => {
                // A replay that could not finish is parked, so the next start does not restore the same windows twice.
                quarantine(&path);
            }
            None => {
                // Nothing here can be replayed, and leaving it would retry on every start.
                tracing::warn!(
                    "{} is not readable recovery state; dropping it",
                    path.display()
                );
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Renames a payload that could not be fully restored, keeping it for support without retrying it.
fn quarantine(path: &Path) {
    let failed = path.with_extension("json.failed");
    tracing::warn!(
        "some windows could not be restored from {}; moving it to {}",
        path.display(),
        failed.display()
    );
    let _ = std::fs::rename(path, failed);
}

fn parse_state_name(name: &str) -> Option<u32> {
    name.strip_prefix("recovery-")?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

pub fn write(path: &Path, bytes: &[u8]) -> bool {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, bytes).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

struct RestoreOutcome {
    /// False when a window was still owned and a write did not take, so replaying the payload could double-restore.
    complete: bool,
}

fn restore(bytes: &[u8]) -> Option<RestoreOutcome> {
    let (slate_hwnd, members) = decode(bytes)?;
    let mut complete = true;
    for (hwnd, st) in members {
        unsafe {
            if !IsWindow(Some(HWND(hwnd as *mut _))).as_bool() {
                continue;
            }
            let owner = GetWindowLongPtrW(HWND(hwnd as *mut _), GWL_HWNDPARENT);
            if owner != 0 && owner != slate_hwnd {
                continue;
            }
        }
        let ok = unregister_member(hwnd, &st);
        complete &= ok;
    }
    Some(RestoreOutcome { complete })
}
