use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde_json::Value;

use super::paths;
use super::schema::{AppRule, SavedSet, SavedSets, SavedTab, Session, Settings, SCHEMA_VERSION};

/// Set when a config file carries a schema newer than this build understands.
static NEWER_SCHEMA: AtomicBool = AtomicBool::new(false);

fn template_dir() -> Option<PathBuf> {
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("configs")));
    let in_cwd = std::env::current_dir().ok().map(|dir| dir.join("configs"));
    [beside_exe, in_cwd]
        .into_iter()
        .flatten()
        .find(|dir| dir.is_dir())
}

/// Creates the data tree, migrates the pre-0.5 layout, seeds missing files.
pub fn ensure() {
    paths::ensure_dirs();
    if paths::is_portable() {
        seed_defaults();
        return;
    }
    let complete = migrate_legacy(&paths::legacy_root(), &paths::data_dir());
    if !complete {
        tracing::warn!("legacy config migration incomplete; retrying next launch");
        return;
    }
    seed_defaults();
    cleanup_legacy();
}

fn seed_defaults() {
    if !paths::settings_path().exists() {
        seed("slate\\settings.json", &paths::settings_path(), &AppSeed);
    }
    if !paths::sets_path().exists() {
        seed("slate\\sets.json", &paths::sets_path(), &SetsSeed);
    }
}

/// Copies the old layout over, keeping whichever copy of a file is newer.
fn migrate_legacy(old_root: &Path, new_root: &Path) -> bool {
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

fn source_is_newer(src: &Path, dest: &Path) -> bool {
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
fn cleanup_legacy() {
    let old_root = paths::legacy_root();
    let old_app = old_root.join("slate");
    if !old_app.is_dir() {
        return;
    }
    if current_exe_dir().is_some_and(|dir| same_dir(&dir, &old_app)) {
        return;
    }
    let new_exe = paths::install_dir().join(crate::core::install::CANONICAL_NAME);
    // Only once the new install exists; until then the old tree holds the only copy of Slate.
    if !new_exe.exists() {
        return;
    }
    if std::fs::remove_dir_all(&old_app).is_err() {
        return;
    }
    let _ = std::fs::remove_dir(&old_root);
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

trait Seed {
    fn content(&self) -> String;
}

struct AppSeed;
struct SetsSeed;

impl Seed for AppSeed {
    fn content(&self) -> String {
        pretty(&super::save::app_value(&Settings::default()))
    }
}

impl Seed for SetsSeed {
    fn content(&self) -> String {
        pretty(&SavedSets::default())
    }
}

/// The exact bytes a save would write, so a seeded file and a saved one agree.
fn pretty(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).map_or_else(
        |_| "{}\n".to_string(),
        |mut text| {
            text.push('\n');
            text
        },
    )
}

fn seed(template_rel: &str, dest: &Path, fallback: &dyn Seed) {
    if let Some(dir) = template_dir() {
        if let Ok(bytes) = std::fs::read(dir.join(template_rel)) {
            if write_new(dest, &bytes) {
                return;
            }
        }
    }
    let _ = write_new(dest, fallback.content().as_bytes());
}

fn write_new(dest: &Path, bytes: &[u8]) -> bool {
    if dest.exists() {
        return true;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(bytes).is_ok()
        }
        Err(_) => false,
    }
}

pub fn load() -> Settings {
    let mut settings = read_settings(&paths::settings_path()).unwrap_or_default();
    settings.preview_delay_ms = settings
        .preview_delay_ms
        .min(crate::timings::MAX_PREVIEW_DELAY_MS);
    settings
}

/// Reads settings through a type check, so one bad field does not drop the rest.
fn read_settings(path: &Path) -> Option<Settings> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            backup(path);
            return None;
        }
    };
    let version = schema_version_of(&value);
    if version > SCHEMA_VERSION as u64 {
        tracing::warn!(
            "{} has schema {version}, newer than this build's {SCHEMA_VERSION}; \
             running with defaults and leaving the file alone",
            path.display()
        );
        NEWER_SCHEMA.store(true, Ordering::Relaxed);
        return None;
    }
    Some(settings_from_value(migrate(value)))
}

/// True when a config file was written by a newer Slate, so nothing may overwrite it.
pub fn schema_is_newer() -> bool {
    NEWER_SCHEMA.load(Ordering::Relaxed)
}

fn settings_from_value(value: Value) -> Settings {
    let sanitized = sanitize(
        None,
        value,
        &serde_json::to_value(Settings::default()).unwrap_or(Value::Null),
    );
    match serde_json::from_value::<Settings>(sanitized.clone()) {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!("settings hold an unreadable value ({err}); keeping the rest");
            settings_field_by_field(sanitized)
        }
    }
}

/// Reads one field at a time, so a single bad field cannot reset every other one.
fn settings_field_by_field(value: Value) -> Settings {
    let Some(map) = value.as_object() else {
        return Settings::default();
    };
    let mut kept = serde_json::Map::new();
    for (key, field) in map {
        let mut probe = serde_json::Map::with_capacity(1);
        probe.insert(key.clone(), field.clone());
        if serde_json::from_value::<Settings>(Value::Object(probe)).is_ok() {
            kept.insert(key.clone(), field.clone());
        } else {
            tracing::warn!("dropping unreadable settings field '{key}'");
        }
    }
    serde_json::from_value(Value::Object(kept)).unwrap_or_default()
}

fn schema_version_of(value: &Value) -> u64 {
    value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}

/// Drops fields and list entries whose JSON shape no longer matches the schema.
fn sanitize(key: Option<&str>, value: Value, default: &Value) -> Value {
    match (value, default) {
        (Value::Object(mut map), Value::Object(defaults)) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let Some(field_default) = defaults.get(&key) else {
                    continue;
                };
                let Some(field) = map.remove(&key) else {
                    continue;
                };
                let fixed = sanitize(Some(&key), field, field_default);
                if !fixed.is_null() || field_default.is_null() {
                    map.insert(key, fixed);
                }
            }
            Value::Object(map)
        }
        (Value::Array(items), Value::Array(default)) => {
            let template = match default.first().cloned().or_else(|| element_template(key)) {
                Some(template) => template,
                None => return Value::Array(items),
            };
            let kept = items
                .into_iter()
                .map(|item| sanitize(None, item, &template))
                .filter(|item| !item.is_null() || template.is_null())
                .collect();
            Value::Array(kept)
        }
        (value, Value::Null) => value,
        (value, default) if value_kind(&value) == value_kind(default) => value,
        (_, _) => Value::Null,
    }
}

/// The shape of one entry for a list field whose default is empty.
fn element_template(key: Option<&str>) -> Option<Value> {
    match key {
        Some("rules") => serde_json::to_value(AppRule::default()).ok(),
        _ => None,
    }
}

fn value_kind(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

pub fn load_sets() -> SavedSets {
    let path = paths::sets_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return SavedSets::default();
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => {
            let version = schema_version_of(&value);
            if version > SCHEMA_VERSION as u64 {
                tracing::warn!(
                    "{} has schema {version}, newer than this build's {SCHEMA_VERSION}; \
                     leaving the file alone",
                    path.display()
                );
                NEWER_SCHEMA.store(true, Ordering::Relaxed);
                return SavedSets::default();
            }
            parse_sets(value)
        }
        Err(_) => {
            backup(&path);
            SavedSets::default()
        }
    }
}

pub fn load_session() -> Session {
    let path = paths::session_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Session::default();
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            backup(&path);
            return Session::default();
        }
    };
    if schema_version_of(&value) > SCHEMA_VERSION as u64 {
        tracing::warn!("{} is newer than this build; ignoring it", path.display());
        backup(&path);
        return Session::default();
    }
    parse_session(migrate(value))
}

/// Keeps the tabs that read, so one bad entry does not drop the session.
fn parse_session(value: Value) -> Session {
    let mut out = Session::default();
    if let Some(items) = value.get("tabs").and_then(Value::as_array) {
        out.tabs = items
            .iter()
            .filter_map(|item| serde_json::from_value::<SavedTab>(item.clone()).ok())
            .collect();
    }
    out
}

fn parse_sets(value: Value) -> SavedSets {
    let value = migrate(value);
    let mut out = SavedSets::default();
    if let Some(version) = value.get("schema_version").and_then(|v| v.as_u64()) {
        out.schema_version = u32::try_from(version).unwrap_or(SCHEMA_VERSION);
    }
    if let Some(items) = value.get("sets").and_then(|v| v.as_array()) {
        out.sets = items
            .iter()
            .filter_map(|item| serde_json::from_value::<SavedSet>(item.clone()).ok())
            .collect();
    }
    assign_ids(&mut out.sets);
    out
}

fn assign_ids(sets: &mut [SavedSet]) {
    let mut used: Vec<u64> = Vec::with_capacity(sets.len());
    let mut next = sets
        .iter()
        .map(|s| s.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for set in sets.iter_mut() {
        if set.id == 0 || used.contains(&set.id) {
            set.id = next;
            next = next.saturating_add(1);
        }
        used.push(set.id);
    }
}

fn migrate(mut value: Value) -> Value {
    if !value.is_object() {
        return value;
    }
    let version = schema_version_of(&value);
    if version < 1 {
        value["schema_version"] = SCHEMA_VERSION.into();
    }
    value
}

fn backup(path: &Path) {
    if !backup_into(&paths::backups_dir(), path) {
        tracing::warn!("could not move {} aside", path.display());
    }
}

/// Moves a file aside. False means it could not be preserved.
fn backup_into(dir: &Path, path: &Path) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("{}.{stamp}.{n}", name.to_string_lossy()));
    if std::fs::rename(path, &dest).is_ok() {
        return true;
    }
    // A rename can fail across a lock; a copy still keeps the bytes.
    if std::fs::copy(path, &dest).is_err() {
        return false;
    }
    let _ = std::fs::remove_file(path);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_only_override_what_the_file_sets() {
        let s = settings_from_value(serde_json::json!({
            "theme": "dark",
            "tabs": { "icons": false }
        }));
        assert_eq!(s.theme, crate::config::ThemeMode::Dark);
        assert_eq!(s.accent.name(), "none");
        assert!(!s.tabs.icons);
        assert!(s.tabs.count_badge);
    }

    #[test]
    fn keybinds_partial_only_sets_what_it_gives() {
        let value = serde_json::json!({
            "keybinds": { "next_tab": "nonsense", "prev_tab": "ctrl+tab" }
        });
        let s = settings_from_value(value);
        assert_eq!(s.keybinds.prev_tab, "ctrl+tab");
        assert_eq!(s.keybinds.close_tab, "ctrl+w");
        let bindings = s.keybinds.bindings();
        let (_, got) = bindings
            .iter()
            .find(|(action, _)| *action == crate::core::keys::KeyAction::PrevTab)
            .unwrap();
        assert_eq!(got.vk, crate::core::keys::KeyAction::PrevTab.default().vk);
    }

    #[test]
    fn settings_read_theme_and_accent() {
        let s = settings_from_value(serde_json::json!({
            "theme": "dark",
            "accent": "Rose",
            "tabs": { "icons": false }
        }));
        assert_eq!(s.theme, crate::config::ThemeMode::Dark);
        assert_eq!(s.accent.name(), "Rose");
        assert!(!s.tabs.icons);
    }

    #[test]
    fn wrong_typed_key_does_not_drop_the_rest() {
        let s = settings_from_value(serde_json::json!({
            "theme": 123,
            "accent": "Rose",
            "tabs": { "icons": "yes", "count_badge": false }
        }));
        assert_eq!(s.theme, crate::config::ThemeMode::System);
        assert_eq!(s.accent.name(), "Rose");
        assert!(s.tabs.icons);
        assert!(!s.tabs.count_badge);
    }

    #[test]
    fn unknown_keys_survive_a_save() {
        let s = settings_from_value(serde_json::json!({ "future_flag": 7 }));
        let value = crate::config::save::app_value(&s);
        assert_eq!(value["future_flag"], 7);
    }

    #[test]
    fn migrate_stamps_missing_version() {
        let out = migrate(serde_json::json!({ "theme": "dark" }));
        assert_eq!(out["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn migrate_leaves_non_object_untouched() {
        assert_eq!(migrate(serde_json::json!(123)).as_u64(), Some(123));
        assert!(migrate(serde_json::json!(null)).is_null());
    }

    #[test]
    fn parse_sets_skips_bad_entries() {
        let value = serde_json::json!({
            "schema_version": 1,
            "sets": [
                { "name": "Good", "tabs": [] },
                { "tabs": "not-an-array" },
                42,
                { "name": "AlsoGood", "tabs": [] }
            ]
        });
        let sets = parse_sets(value);
        let names: Vec<_> = sets.sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Good", "AlsoGood"]);
    }

    #[test]
    fn parse_sets_assigns_unique_ids() {
        let value = serde_json::json!({
            "schema_version": 1,
            "sets": [
                { "name": "A" },
                { "id": 5, "name": "B" },
                { "id": 5, "name": "C" }
            ]
        });
        let sets = parse_sets(value);
        let ids: Vec<u64> = sets.sets.iter().map(|s| s.id).collect();
        assert_eq!(ids, [6, 5, 7]);
    }

    #[test]
    fn parse_sets_clamps_future_version() {
        let value = serde_json::json!({ "schema_version": 999999999999u64, "sets": [] });
        let sets = parse_sets(value);
        assert_eq!(sets.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn session_round_trips() {
        let session = Session {
            tabs: vec![crate::config::SavedTab {
                exe_path: "C:\\a.exe".into(),
                exe_name: "a".into(),
                title_hint: "A".into(),
                accent: Some(2),
                pid: None,
                rect: None,
            }],
        };
        let text = serde_json::to_string(&session).unwrap();
        let back: Session = serde_json::from_str(&text).unwrap();
        assert_eq!(back.tabs.len(), 1);
        assert_eq!(back.tabs[0].exe_path, "C:\\a.exe");
        assert_eq!(back.tabs[0].accent, Some(2));
    }

    #[test]
    fn app_value_keeps_restore_session() {
        let settings = Settings {
            restore_session: false,
            ..Settings::default()
        };
        let value = crate::config::save::app_value(&settings);
        assert_eq!(value["restore_session"], false);
        assert_eq!(value["theme"], "system");
        assert_eq!(value["accent"], "none");
    }

    fn scratch(name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "slate-config-test-{}-{name}-{n}",
            std::process::id()
        ))
    }

    fn write_at(path: &Path, text: &str, modified: std::time::SystemTime) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(path, text).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    #[test]
    fn migration_copies_state_and_merges_theme() {
        let old = scratch("old");
        let new = scratch("new");
        std::fs::create_dir_all(old.join("slate")).unwrap();
        std::fs::write(
            old.join("slate").join("settings.json"),
            r#"{ "schema_version": 1, "restore_session": false }"#,
        )
        .unwrap();
        std::fs::write(
            old.join("slate").join("sets.json"),
            r#"{ "schema_version": 1, "sets": [] }"#,
        )
        .unwrap();
        std::fs::write(
            old.join("config.json"),
            r#"{ "theme": "dark", "accent": "red" }"#,
        )
        .unwrap();

        assert!(migrate_legacy(&old, &new));

        let text = std::fs::read_to_string(new.join("settings.json")).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["restore_session"], false);
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["accent"], "red");
        assert!(new.join("sets.json").is_file());

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migration_refuses_corrupt_files() {
        let old = scratch("corrupt-old");
        let new = scratch("corrupt-new");
        std::fs::create_dir_all(old.join("slate")).unwrap();
        std::fs::write(old.join("slate").join("sets.json"), "not json").unwrap();

        assert!(!migrate_legacy(&old, &new));
        assert!(!new.join("sets.json").exists());

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn migration_is_a_noop_without_a_legacy_dir() {
        let old = scratch("absent-old");
        let new = scratch("absent-new");
        assert!(migrate_legacy(&old, &new));
        assert!(!new.exists());
    }

    #[test]
    fn newer_legacy_copy_replaces_a_stale_destination() {
        let old = scratch("newer-old");
        let new = scratch("newer-new");
        let now = std::time::SystemTime::now();
        let hour = std::time::Duration::from_secs(3600);

        write_at(
            &new.join("settings.json"),
            r#"{ "theme": "light" }"#,
            now - hour,
        );
        write_at(
            &new.join("config.json"),
            r#"{ "accent": "red" }"#,
            now - hour,
        );
        write_at(
            &old.join("slate").join("settings.json"),
            r#"{ "theme": "dark" }"#,
            now,
        );

        assert!(migrate_legacy(&old, &new));

        let text = std::fs::read_to_string(new.join("settings.json")).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["accent"], "red");
        assert!(!new.join("config.json").exists());
        let backups = std::fs::read_dir(new.join("backups")).unwrap().count();
        assert_eq!(backups, 2);

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn older_legacy_copy_does_not_replace_a_newer_destination() {
        let old = scratch("older-old");
        let new = scratch("older-new");
        let now = std::time::SystemTime::now();
        let hour = std::time::Duration::from_secs(3600);

        write_at(
            &old.join("slate").join("settings.json"),
            r#"{ "theme": "dark" }"#,
            now - hour,
        );
        write_at(&new.join("settings.json"), r#"{ "theme": "light" }"#, now);

        assert!(migrate_legacy(&old, &new));
        let text = std::fs::read_to_string(new.join("settings.json")).unwrap();
        assert!(text.contains("light"));
        assert!(!new.join("backups").exists());

        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn one_bad_rule_does_not_reset_the_settings() {
        let s = settings_from_value(serde_json::json!({
            "theme": "dark",
            "rules": [1, { "exe": "chrome.exe", "never_host": true }, "nope"]
        }));
        assert_eq!(s.theme, crate::config::ThemeMode::Dark);
        assert_eq!(s.rules.len(), 1);
        assert_eq!(s.rules[0].exe, "chrome.exe");
    }

    #[test]
    fn a_field_that_still_fails_keeps_the_others() {
        let kept = settings_field_by_field(serde_json::json!({
            "theme": "dark",
            "preview_delay_ms": "soon"
        }));
        assert_eq!(kept.theme, crate::config::ThemeMode::Dark);
        assert_eq!(
            kept.preview_delay_ms,
            crate::timings::DEFAULT_PREVIEW_DELAY_MS
        );
    }

    #[test]
    fn a_newer_settings_file_is_left_alone() {
        let dir = scratch("newer-schema");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let text = format!(
            r#"{{ "schema_version": {}, "theme": "dark" }}"#,
            SCHEMA_VERSION + 1
        );
        std::fs::write(&path, &text).unwrap();

        assert!(read_settings(&path).is_none());
        assert!(schema_is_newer());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_keeps_the_tabs_that_read() {
        let session = parse_session(serde_json::json!({
            "tabs": [
                { "exe_path": "C:\\a.exe", "exe_name": "a.exe" },
                7,
                { "exe_path": "C:\\b.exe", "exe_name": "b.exe" }
            ]
        }));
        assert_eq!(session.tabs.len(), 2);
        assert_eq!(session.tabs[1].exe_name, "b.exe");
    }

    #[test]
    fn a_failed_backup_reports_failure() {
        let blocked = scratch("backup-blocked");
        std::fs::write(&blocked, b"not a directory").unwrap();
        let source = blocked.with_extension("json");
        std::fs::write(&source, b"{}").unwrap();

        assert!(!backup_into(&blocked, &source));
        assert!(source.exists(), "the source must survive a failed backup");

        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&blocked);
    }

    #[test]
    fn shipped_templates_match_the_defaults() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("configs")
            .join("slate");
        let shipped: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            shipped,
            crate::config::save::app_value(&Settings::default())
        );

        let shipped: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("sets.json")).unwrap()).unwrap();
        assert_eq!(shipped, serde_json::to_value(SavedSets::default()).unwrap());
    }
}
