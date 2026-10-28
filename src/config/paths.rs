use std::path::{Path, PathBuf};

fn home_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("USERPROFILE") {
        let dir = PathBuf::from(dir);
        if !dir.as_os_str().is_empty() {
            return dir;
        }
    }
    let drive = std::env::var_os("HOMEDRIVE");
    let path = std::env::var_os("HOMEPATH");
    if let (Some(drive), Some(path)) = (drive, path) {
        let mut combined = PathBuf::from(drive);
        combined.push(path);
        if !combined.as_os_str().is_empty() {
            return combined;
        }
    }
    std::env::temp_dir()
}

fn local_app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(dir);
        if !dir.as_os_str().is_empty() {
            return dir;
        }
    }
    home_dir().join("AppData").join("Local")
}

fn program_files_local_dir() -> PathBuf {
    local_app_data_dir().join("Programs")
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

pub fn is_portable() -> bool {
    if std::env::var_os("SLATE_PORTABLE").is_some() {
        return true;
    }
    if let Some(dir) = exe_dir() {
        if dir.join("slate.portable").exists() {
            return true;
        }
    }
    false
}

/// Settings, sets, session, install record, backups, updates and logs.
pub fn data_dir() -> PathBuf {
    if is_portable() {
        if let Some(dir) = exe_dir() {
            return dir.join("data");
        }
    }
    local_app_data_dir().join("Burntfern").join("Slate")
}

/// The installed slate.exe. Portable builds run from wherever they unpacked.
pub fn install_dir() -> PathBuf {
    if is_portable() {
        if let Some(dir) = exe_dir() {
            return dir;
        }
    }
    program_files_local_dir().join("Burntfern").join("Slate")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn sets_path() -> PathBuf {
    data_dir().join("sets.json")
}

pub fn session_path() -> PathBuf {
    data_dir().join("session.json")
}

pub fn install_path() -> PathBuf {
    data_dir().join("install.json")
}

pub fn updates_dir() -> PathBuf {
    data_dir().join("updates")
}

pub fn backups_dir() -> PathBuf {
    data_dir().join("backups")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Pre-0.5 layout: `%USERPROFILE%\.burntfern\`.
pub fn legacy_root() -> PathBuf {
    home_dir().join(".burntfern")
}

pub fn legacy_app_dir() -> PathBuf {
    legacy_root().join("slate")
}

/// True when exe is the copy the old layout installed.
pub fn is_legacy_install(exe: &str) -> bool {
    if exe.is_empty() {
        return false;
    }
    let Some(parent) = Path::new(exe).parent() else {
        return false;
    };
    let legacy = legacy_app_dir();
    parent
        .as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&legacy.to_string_lossy())
}

pub fn ensure_dirs() {
    for dir in [
        data_dir(),
        install_dir(),
        updates_dir(),
        backups_dir(),
        logs_dir(),
    ] {
        let _ = std::fs::create_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_and_install_are_separate_outside_portable_mode() {
        if is_portable() {
            return;
        }
        assert_ne!(data_dir(), install_dir());
        assert_eq!(
            data_dir().file_name().and_then(|n| n.to_str()),
            Some("Slate")
        );
        assert_eq!(
            install_dir().parent().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("Burntfern"))
        );
    }

    #[test]
    fn state_files_live_under_the_data_dir() {
        for path in [settings_path(), sets_path(), session_path(), install_path()] {
            assert_eq!(path.parent(), Some(data_dir().as_path()));
        }
        assert_eq!(updates_dir().parent(), Some(data_dir().as_path()));
        assert_eq!(backups_dir().parent(), Some(data_dir().as_path()));
        assert_eq!(logs_dir().parent(), Some(data_dir().as_path()));
    }

    #[test]
    fn legacy_layout_is_detected() {
        let exe = legacy_app_dir().join("slate.exe");
        assert!(is_legacy_install(&exe.to_string_lossy()));
        assert!(!is_legacy_install(""));
        assert!(!is_legacy_install(r"C:\apps\slate.exe"));
    }
}
