mod wire;

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, HWND, WAIT_EVENT};
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE, PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, IsWindow, ShowWindow, GWL_HWNDPARENT, SW_HIDE,
};

use super::host::{unregister_member, StylingState};
pub use wire::{decode, encode};

#[derive(Default)]
pub struct Janitor {
    path: Option<PathBuf>,
    started: bool,
    last: Option<Vec<u8>>,
}

impl Janitor {
    pub fn new() -> Self {
        // Restores whatever a previous crash left behind; the windows stay where the session put them.
        sweep_stale();
        Self {
            path: Some(state_path()),
            started: false,
            last: None,
        }
    }

    /// Drops the payload after a clean restore, so the child janitor has nothing left to replay.
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
        let Some(key) = hmac_key() else {
            return;
        };
        let bytes = encode(&key, slate_hwnd, members);
        if self.last.as_deref() == Some(bytes.as_slice()) {
            return;
        }
        if !self.started {
            spawn(&path);
            self.started = true;
        }
        if write(&path, &bytes) {
            self.last = Some(bytes);
        }
    }
}

pub fn state_path() -> PathBuf {
    let dir = crate::config::paths::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("janitor-{}.bin", std::process::id()))
}

fn hmac_key() -> Option<[u8; 32]> {
    static KEY: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();
    *KEY.get_or_init(load_or_create_key)
}

fn key_path() -> PathBuf {
    crate::config::paths::data_dir().join("janitor.key")
}

/// DPAPI-protected so another account cannot forge it; older raw 32-byte files get rewritten.
/// A key that exists but cannot be read is kept: replacing it would orphan every payload.
fn load_or_create_key() -> Option<[u8; 32]> {
    let path = key_path();
    if path.exists() {
        let Ok(bytes) = std::fs::read(&path) else {
            tracing::error!("could not read the janitor key; crash recovery is off");
            return None;
        };
        if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
            save_key(&path, &key);
            return Some(key);
        }
        if let Some(raw) = unprotect(&bytes) {
            if let Ok(key) = <[u8; 32]>::try_from(raw.as_slice()) {
                return Some(key);
            }
        }
        tracing::error!("the janitor key is unreadable; leaving it and skipping recovery");
        return None;
    }
    let mut key = [0u8; 32];
    let generated = unsafe {
        use windows::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        BCryptGenRandom(None, &mut key, BCRYPT_USE_SYSTEM_PREFERRED_RNG)
    };
    if generated.is_err() {
        return None;
    }
    save_key(&path, &key);
    Some(key)
}

fn save_key(path: &Path, key: &[u8; 32]) {
    let Some(sealed) = protect(key) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, sealed);
}

fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .ok()?;
        let sealed = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        free_blob(&output);
        Some(sealed)
    }
}

fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: sealed.len() as u32,
            pbData: sealed.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .ok()?;
        let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        free_blob(&output);
        Some(plain)
    }
}

fn free_blob(blob: &windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB) {
    use windows::Win32::Foundation::LocalFree;
    unsafe {
        if !blob.pbData.is_null() {
            let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                blob.pbData as *mut core::ffi::c_void,
            )));
        }
    }
}

pub fn spawn(path: &Path) {
    use std::os::windows::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .arg("--janitor")
        .arg(path)
        .arg(std::process::id().to_string())
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn();
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
        if outcome.complete {
            let _ = std::fs::remove_file(&path);
        } else {
            // A replay that could not finish is parked, so the next start does not restore the same windows twice.
            quarantine(&path);
        }
    }
}

/// Renames a payload that could not be fully restored, keeping it for support without retrying it.
fn quarantine(path: &Path) {
    let failed = path.with_extension("bin.failed");
    tracing::warn!(
        "some windows could not be restored from {}; moving it to {}",
        path.display(),
        failed.display()
    );
    let _ = std::fs::rename(path, failed);
}

fn parse_state_name(name: &str) -> Option<u32> {
    name.strip_prefix("janitor-")?
        .strip_suffix(".bin")?
        .parse()
        .ok()
}

pub fn process_alive(pid: u32) -> bool {
    const ERROR_INVALID_PARAMETER_LOW: u32 = 87;
    unsafe {
        match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(handle) => {
                let alive = WaitForSingleObject(handle, 0) != WAIT_EVENT(0);
                let _ = CloseHandle(handle);
                alive
            }
            Err(e) => (e.code().0 as u32 & 0xFFFF) != ERROR_INVALID_PARAMETER_LOW,
        }
    }
}

pub fn write(path: &Path, bytes: &[u8]) -> bool {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, bytes).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

pub fn run(path: PathBuf, pid: u32) {
    if pid == 0 {
        return;
    }
    hide_console();
    let _ = watch(pid);
    let mut complete = true;
    if let Ok(bytes) = std::fs::read(&path) {
        complete = restore(&bytes).complete;
    }
    if complete {
        let _ = std::fs::remove_file(&path);
    } else {
        quarantine(&path);
    }
}

fn hide_console() {
    use windows::Win32::System::Console::GetConsoleWindow;
    unsafe {
        let console = GetConsoleWindow();
        if !console.0.is_null() {
            let _ = ShowWindow(console, SW_HIDE);
        }
    }
}

fn watch(pid: u32) -> windows::core::Result<()> {
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, false, pid)?;
        WaitForSingleObject(handle, INFINITE);
        let _ = CloseHandle(handle);
    }
    Ok(())
}

struct RestoreOutcome {
    /// False when a window was still owned and a write did not take, so replaying the payload could double-restore.
    complete: bool,
}

fn restore(bytes: &[u8]) -> RestoreOutcome {
    let mut outcome = RestoreOutcome { complete: true };
    let Some(key) = hmac_key() else {
        outcome.complete = false;
        return outcome;
    };
    let Some((slate_hwnd, members)) = decode(&key, bytes) else {
        outcome.complete = false;
        return outcome;
    };
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
        outcome.complete &= ok;
    }
    outcome
}
