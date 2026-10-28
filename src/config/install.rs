use std::path::{Path, PathBuf};

use super::paths;
use super::save::atomic_write;
use super::schema::InstallRecord;

pub fn slate_dir() -> PathBuf {
    paths::install_dir()
}

pub fn canonical_exe_path() -> PathBuf {
    slate_dir().join(crate::core::install::CANONICAL_NAME)
}

pub fn install_path() -> PathBuf {
    paths::install_path()
}

pub fn load_install() -> Option<InstallRecord> {
    read_install(&install_path())
}

pub fn save_install(record: &InstallRecord) -> bool {
    write_install(&install_path(), record)
}

pub fn staging_updates_dir() -> PathBuf {
    paths::updates_dir()
}

fn read_install(path: &Path) -> Option<InstallRecord> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_install(path: &Path, record: &InstallRecord) -> bool {
    let Ok(mut text) = serde_json::to_string_pretty(record) else {
        return false;
    };
    text.push('\n');
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    atomic_write(path, text.as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("slate-install-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn canonical_exe_sits_inside_the_install_dir() {
        assert_eq!(canonical_exe_path().parent(), Some(slate_dir().as_path()));
        assert_eq!(
            canonical_exe_path().file_name(),
            Some(std::ffi::OsStr::new("slate.exe"))
        );
        assert_eq!(slate_dir(), paths::install_dir());
    }

    #[test]
    fn the_install_dir_is_under_programs() {
        if paths::is_portable() {
            return;
        }
        let parent = slate_dir().parent().map(|p| p.to_path_buf());
        assert_eq!(
            parent.as_deref().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("Burntfern"))
        );
        assert_eq!(
            parent
                .as_deref()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("Programs"))
        );
    }

    #[test]
    fn install_record_and_updates_live_in_the_data_dir() {
        assert_eq!(install_path().parent(), Some(paths::data_dir().as_path()));
        assert_eq!(
            paths::settings_path().parent(),
            Some(paths::data_dir().as_path())
        );
        assert_eq!(staging_updates_dir(), paths::updates_dir());
        assert_eq!(
            staging_updates_dir().parent(),
            Some(paths::data_dir().as_path())
        );
    }

    #[test]
    fn missing_file_reads_as_no_record() {
        assert_eq!(read_install(&scratch("missing.json")), None);
    }

    #[test]
    fn record_round_trips_through_disk() {
        let path = scratch("roundtrip.json");
        let record = InstallRecord {
            path: "C:\\apps\\slate.exe".into(),
            version: "0.1.1".into(),
            pid: 99,
        };
        assert!(write_install(&path, &record));
        assert_eq!(read_install(&path), Some(record));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_reads_as_no_record() {
        let path = scratch("corrupt.json");
        std::fs::write(&path, "not json at all").unwrap();
        assert_eq!(read_install(&path), None);
        let _ = std::fs::remove_file(path);
    }
}
