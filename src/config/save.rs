use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};

use super::paths;
use super::schema::{SavedSets, Session, Settings, SCHEMA_VERSION};

/// Rename tries and the pause between them; a scanner or indexer can hold the target briefly.
const RENAME_TRIES: u32 = 4;
const RENAME_PAUSE_MS: u64 = 15;

pub(crate) fn app_value(settings: &Settings) -> Value {
    serde_json::to_value(settings).unwrap_or_else(|_| json!({ "schema_version": SCHEMA_VERSION }))
}

pub fn save(settings: &Settings) -> io::Result<()> {
    if !can_write() {
        return Ok(());
    }
    std::fs::create_dir_all(paths::data_dir())?;
    let value = app_value(settings);
    let mut text = serde_json::to_string_pretty(&value).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(&paths::settings_path(), text.as_bytes())
}

pub fn save_sets(sets: &SavedSets) -> io::Result<()> {
    if !can_write() {
        return Ok(());
    }
    std::fs::create_dir_all(paths::data_dir())?;
    let mut text = serde_json::to_string_pretty(sets).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(&paths::sets_path(), text.as_bytes())
}

pub fn save_session(session: &Session) -> io::Result<()> {
    std::fs::create_dir_all(paths::data_dir())?;
    let mut text = serde_json::to_string_pretty(session).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(&paths::session_path(), text.as_bytes())
}

/// Refuses to write over a file a newer Slate owns.
fn can_write() -> bool {
    if super::load::schema_is_newer() {
        tracing::warn!("not saving: a config file was written by a newer Slate");
        return false;
    }
    true
}

fn temp_path_for(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = temp_path_for(path);
    let written = (|| -> io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(err) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    match rename_with_retry(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}

/// A rename can lose a race with a scanner or indexer holding the destination.
fn rename_with_retry(tmp: &Path, path: &Path) -> io::Result<()> {
    let mut last = None;
    for attempt in 1..=RENAME_TRIES {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(err) => last = Some(err),
        }
        if attempt < RENAME_TRIES {
            std::thread::sleep(Duration::from_millis(RENAME_PAUSE_MS * u64::from(attempt)));
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("could not replace the file")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("slate-save-test-{}-{name}-{n}", std::process::id()))
    }

    /// The temp files `atomic_write` stages beside its destination.
    fn temp_leftovers(dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect()
    }

    #[test]
    fn atomic_write_replaces_in_place() {
        let dir = scratch("replace");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");

        atomic_write(&path, b"one").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert!(
            temp_leftovers(&dir).is_empty(),
            "a write leaves no temp file"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_cleans_up_when_it_cannot_write() {
        let dir = scratch("unwritable");
        let path = dir.join("nested").join("settings.json");

        assert!(atomic_write(&path, b"x").is_err());
        assert!(!path.exists());
        assert!(temp_leftovers(&dir).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_locked_destination_surfaces_an_error() {
        let dir = scratch("locked");
        std::fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join("write.tmp");
        std::fs::write(&tmp, b"x").unwrap();
        // A directory in the destination's place fails every rename attempt.
        let dest = dir.join("settings.json");
        std::fs::create_dir_all(&dest).unwrap();

        assert!(rename_with_retry(&tmp, &dest).is_err());
        assert!(tmp.exists(), "the caller still owns the temp file");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
