//! Pre-0.5 layout migration: copies the old tree forward and retires it.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::load::backup_into;
use super::paths;
use super::schema::SCHEMA_VERSION;

/// Copies the old layout over, keeping whichever copy of a file is newer.
pub(super) fn migrate_legacy(old_root: &Path, new_root: &Path) -> bool {
    if !old_root.is_dir() {
        return true;
    }
    let old_app = old_root.join("slate");
    let backups = new_root.join("backups");
    let mut complete = true;
    for name in ["settings.json", "sets.json", "session.json", "install.json"] {
        let src = old_app.join(name);
        let dest = new_root.join(name);
        if !src.is_file() {
            continue;
        }
        if dest.exists() {
            if !source_is_newer(&src, &dest) {
                continue;
            }
            if !backup_into(&backups, &dest) {
                tracing::warn!("could not back up {}; leaving it in place", dest.display());
                complete = false;
                continue;
            }
        }
        if let Err(err) = copy_json(&src, &dest) {
            tracing::warn!("could not migrate {}: {err}", src.display());
            complete = false;
        }
    }
    if !merge_theme_sources(old_root, new_root) {
        complete = false;
    }
    complete
}

pub(super) fn source_is_newer(src: &Path, dest: &Path) -> bool {
    let modified = |path: &Path| path.metadata().and_then(|meta| meta.modified()).ok();
    match (modified(src), modified(dest)) {
        (Some(src), Some(dest)) => src > dest,
        (Some(_), None) => true,
        _ => false,
    }
}

fn copy_json(src: &Path, dest: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(src)?;
    serde_json::from_slice::<Value>(&bytes).map_err(std::io::Error::other)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    super::save::atomic_write(dest, &bytes)
}

/// Folds theme and accent into settings.json, from the newest config.json.
fn merge_theme_sources(old_root: &Path, new_root: &Path) -> bool {
    let settings = new_root.join("settings.json");
    let new_config = new_root.join("config.json");
    let mut merged = std::fs::read_to_string(&settings)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({ "schema_version": SCHEMA_VERSION }));
    let has_theme = merged.get("theme").and_then(Value::as_str).is_some();
    let has_accent = merged.get("accent").and_then(Value::as_str).is_some();
    let mut changed = false;
    if !(has_theme && has_accent) {
        if let Some((theme, accent)) = newest_theme(&old_root.join("config.json"), &new_config) {
            if let Some(object) = merged.as_object_mut() {
                object.insert("schema_version".into(), SCHEMA_VERSION.into());
                if !has_theme {
                    if let Some(theme) = theme {
                        object.insert("theme".into(), theme.into());
                        changed = true;
                    }
                }
                if !has_accent {
                    if let Some(accent) = accent {
                        object.insert("accent".into(), accent.into());
                        changed = true;
                    }
                }
            }
        }
    }
    if changed {
        let Ok(mut text) = serde_json::to_string_pretty(&merged) else {
            return false;
        };
        text.push('\n');
        if let Some(dir) = settings.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if super::save::atomic_write(&settings, text.as_bytes()).is_err() {
            return false;
        }
    }
    if new_config.is_file() && !backup_into(&new_root.join("backups"), &new_config) {
        tracing::warn!("could not move {} aside", new_config.display());
        return false;
    }
    true
}

fn newest_theme(old_config: &Path, new_config: &Path) -> Option<(Option<String>, Option<String>)> {
    let mut best: Option<(std::time::SystemTime, Option<String>, Option<String>)> = None;
    for path in [old_config, new_config] {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let theme = value
            .get("theme")
            .and_then(Value::as_str)
            .map(str::to_string);
        let accent = value
            .get("accent")
            .and_then(Value::as_str)
            .map(str::to_string);
        if theme.is_none() && accent.is_none() {
            continue;
        }
        let modified = path
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(stamp, ..)| modified >= *stamp) {
            best = Some((modified, theme, accent));
        }
    }
    best.map(|(_, theme, accent)| (theme, accent))
}

/// Removes the old app folder only, never the shared ~/.burntfern root.
pub(super) fn cleanup_legacy() {
    cleanup_legacy_at(
        &paths::legacy_root(),
        &paths::install_dir(),
        current_exe_dir().as_deref(),
    );
}

/// Removes `old_root/slate` only once the new install exists and this exe is not running from it.
pub(super) fn cleanup_legacy_at(old_root: &Path, install_dir: &Path, exe_dir: Option<&Path>) {
    let old_app = old_root.join("slate");
    if !old_app.is_dir() {
        return;
    }
    if exe_dir.is_some_and(|dir| same_dir(dir, &old_app)) {
        return;
    }
    let new_exe = install_dir.join(crate::core::install::CANONICAL_NAME);
    // Only once the new install exists; until then the old tree holds the only copy of Slate.
    if !new_exe.exists() {
        return;
    }
    if std::fs::remove_dir_all(&old_app).is_err() {
        return;
    }
    let _ = std::fs::remove_dir(old_root);
}

fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}
