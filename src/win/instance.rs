//! One Slate per data folder.
//!
//! Portable and installed builds read the same settings unless `SLATE_PORTABLE` is set, so a
//! second copy would fight the first over the same files. A named mutex derived from the data
//! directory makes the claim atomic, and a pid file lets the losing copy focus the winner.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows::Win32::System::Threading::CreateMutexW;

use crate::core::install;

fn pid_path() -> PathBuf {
    crate::config::paths::data_dir().join("instance.pid")
}

/// A stable name for this data folder; the prefix keeps it inside the user's session namespace.
fn mutex_name() -> Vec<u16> {
    let dir = crate::config::paths::data_dir();
    let mut hasher = DefaultHasher::new();
    dir.to_string_lossy().to_lowercase().hash(&mut hasher);
    let name = format!("Local\\Slate-{:016x}", hasher.finish());
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
        .filter(|pid| crate::win::process_alive(*pid));
    if let Some(pid) = pid {
        crate::win::activate_process(pid);
    }
    crate::win::shell::info_message(
        "Slate is already running",
        "Only one copy can use this settings folder. Switch to the open window, or set SLATE_PORTABLE to run a separate one.",
    );
}
