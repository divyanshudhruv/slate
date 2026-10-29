//! One Slate per data folder: portable and installed builds share settings, so a named mutex and a pid file keep one copy in charge.

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, WAIT_EVENT};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};

use crate::core::install;

fn pid_path() -> PathBuf {
    crate::config::paths::data_dir().join("instance.pid")
}

/// A stable name for this data folder; the prefix keeps it inside the user's session namespace.
/// Hashed with sha2 rather than `DefaultHasher`, which may change between Rust releases.
fn mutex_name() -> Vec<u16> {
    let dir = crate::config::paths::data_dir();
    let digest = Sha256::digest(dir.to_string_lossy().to_lowercase().as_bytes());
    let name = format!(
        "Local\\Slate-{:016x}",
        u64::from_be_bytes(digest[..8].try_into().unwrap_or_default())
    );
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Claims the data folder. False means another copy owns it and this process should exit.
pub fn claim() -> bool {
    // Dev builds run beside an installed copy on purpose, so they never take the claim.
    let dev = std::env::current_exe()
        .map(|exe| install::dev_build(&exe.to_string_lossy()))
        .unwrap_or(false);
    if dev {
        return true;
    }
    let wide = mutex_name();
    let Ok(_handle) = (unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }) else {
        // Without a mutex, starting is still better than refusing to run.
        return true;
    };
    let already = unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS;
    if already {
        focus_running();
        return false;
    }
    // The mutex lives until the process exits; nothing closes the handle.
    write_pid();
    true
}

fn write_pid() {
    let path = pid_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, std::process::id().to_string());
}

fn focus_running() {
    let pid = std::fs::read_to_string(pid_path())
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .filter(|pid| *pid != std::process::id())
        .filter(|pid| process_alive(*pid));
    if let Some(pid) = pid {
        crate::win::activate_process(pid);
    }
    crate::win::shell::info_message(
        "Slate is already running",
        "Only one copy can use this settings folder. Switch to the open window, or set SLATE_PORTABLE to run a separate one.",
    );
}

/// The pid in `instance.pid` while that copy is alive and is not this process.
pub fn running_pid() -> Option<u32> {
    let pid = std::fs::read_to_string(pid_path())
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()?;
    (pid != std::process::id() && process_alive(pid)).then_some(pid)
}

/// True while a process with this id still exists; used to tell a crashed session from a running one.
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
