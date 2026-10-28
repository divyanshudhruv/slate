use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::paths;
use super::schema::{SavedSets, Session, Settings, SCHEMA_VERSION};

pub(crate) fn app_value(settings: &Settings) -> Value {
    serde_json::to_value(settings).unwrap_or_else(|_| json!({ "schema_version": SCHEMA_VERSION }))
}

pub fn save(settings: &Settings) -> io::Result<()> {
    std::fs::create_dir_all(paths::data_dir())?;
    let value = app_value(settings);
    let mut text = serde_json::to_string_pretty(&value).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(&paths::settings_path(), text.as_bytes())
}

pub fn save_sets(sets: &SavedSets) -> io::Result<()> {
    save_sets_at(&paths::sets_path(), sets)
}

/// Writes a set list anywhere, so export uses the same atomic path as the live file.
pub fn save_sets_at(path: &Path, sets: &SavedSets) -> io::Result<()> {
    std::fs::create_dir_all(paths::data_dir())?;
    let mut text = serde_json::to_string_pretty(sets).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(path, text.as_bytes())
}

pub fn save_session(session: &Session) -> io::Result<()> {
    std::fs::create_dir_all(paths::data_dir())?;
    let mut text = serde_json::to_string_pretty(session).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write(&paths::session_path(), text.as_bytes())
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
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}
