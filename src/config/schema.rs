use serde::{Deserialize, Serialize};

use crate::core::keys::{format_chord, KeyAction, ACTIONS};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub accent: Accent,
    #[serde(default = "yes")]
    pub restore_session: bool,
    #[serde(default = "preview_delay_default")]
    pub preview_delay_ms: u64,
    #[serde(default)]
    pub tabs: Tabs,
    #[serde(default)]
    pub updates: Updates,
    #[serde(default)]
    pub keybinds: Keybinds,
    #[serde(default)]
    pub rules: Vec<AppRule>,
    /// Keys Slate does not know yet, kept so a hand edit or a newer build survives a save.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A per-app hosting rule, matched on the executable name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRule {
    #[serde(default)]
    pub exe: String,
    #[serde(default)]
    pub never_host: bool,
    #[serde(default)]
    pub new_group: bool,
    #[serde(default)]
    pub label: Option<String>,
}

/// How the host picks its palette; unknown values warn and fall back to the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::System,
        }
    }
}

impl<'de> Deserialize<'de> for ThemeMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(match text.to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            "system" => Self::System,
            other => {
                tracing::warn!("unknown theme '{other}', using system");
                Self::System
            }
        })
    }
}

/// A named accent from the theme's hue palette; unknown names warn and fall back to blue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Accent(String);

impl Default for Accent {
    fn default() -> Self {
        Self("blue".to_string())
    }
}

impl Accent {
    pub fn name(&self) -> &str {
        &self.0
    }

    fn accepts(name: &str) -> bool {
        name.eq_ignore_ascii_case("none")
            || ely_gpui_component::theme::HUE_NAMES
                .iter()
                .any(|hue| hue.eq_ignore_ascii_case(name))
    }
}

impl<'de> Deserialize<'de> for Accent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if Self::accepts(&text) {
            return Ok(Self(text));
        }
        tracing::warn!("unknown accent '{text}', using blue");
        Ok(Self::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tabs {
    #[serde(default = "yes")]
    pub icons: bool,
    #[serde(default = "yes")]
    pub count_badge: bool,
    #[serde(default = "no")]
    pub hover_preview: bool,
    #[serde(default = "yes")]
    pub show_bar: bool,
}

impl Default for Tabs {
    fn default() -> Self {
        Self {
            icons: true,
            count_badge: true,
            hover_preview: false,
            show_bar: true,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme: ThemeMode::default(),
            accent: Accent::default(),
            restore_session: true,
            preview_delay_ms: preview_delay_default(),
            tabs: Tabs::default(),
            updates: Updates::default(),
            keybinds: Keybinds::default(),
            rules: Vec::new(),
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSet {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tabs: Vec<SavedTab>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Updates {
    #[serde(default)]
    pub latest: Option<String>,
    #[serde(default)]
    pub checked_at: i64,
    #[serde(default)]
    pub channel: crate::core::update::Channel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedTab {
    #[serde(default)]
    pub exe_path: String,
    #[serde(default)]
    pub exe_name: String,
    #[serde(default)]
    pub title_hint: String,
    #[serde(default)]
    pub accent: Option<usize>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub rect: Option<[i32; 4]>,
}

/// Per-action chords written as strings like "ctrl+shift+tab" so hand edits stay possible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Keybinds {
    #[serde(default = "next_tab_default")]
    pub next_tab: String,
    #[serde(default = "prev_tab_default")]
    pub prev_tab: String,
    #[serde(default = "close_tab_default")]
    pub close_tab: String,
    #[serde(default = "select_tab_default")]
    pub select_tab: String,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            next_tab: next_tab_default(),
            prev_tab: prev_tab_default(),
            close_tab: close_tab_default(),
            select_tab: select_tab_default(),
        }
    }
}

fn next_tab_default() -> String {
    format_chord(KeyAction::NextTab.default()).unwrap_or_else(|| "ctrl+tab".into())
}
fn prev_tab_default() -> String {
    format_chord(KeyAction::PrevTab.default()).unwrap_or_else(|| "ctrl+shift+tab".into())
}
fn close_tab_default() -> String {
    format_chord(KeyAction::CloseTab.default()).unwrap_or_else(|| "ctrl+w".into())
}
fn select_tab_default() -> String {
    format_chord(KeyAction::SelectTab.default()).unwrap_or_else(|| "ctrl+1".into())
}

impl Keybinds {
    fn chord_text(&self, action: KeyAction) -> &str {
        match action {
            KeyAction::NextTab => &self.next_tab,
            KeyAction::PrevTab => &self.prev_tab,
            KeyAction::CloseTab => &self.close_tab,
            KeyAction::SelectTab => &self.select_tab,
        }
    }

    /// Writes a chord for an action, falling back to the action's own default when it cannot be spelled.
    pub fn set_chord(&mut self, action: KeyAction, chord: crate::core::keys::Chord) {
        let text = match format_chord(chord) {
            Some(text) => text,
            None => crate::core::keys::format_chord(action.default()).unwrap_or_default(),
        };
        match action {
            KeyAction::NextTab => self.next_tab = text,
            KeyAction::PrevTab => self.prev_tab = text,
            KeyAction::CloseTab => self.close_tab = text,
            KeyAction::SelectTab => self.select_tab = text,
        }
    }

    /// Parsed chords, one per action, falling back to the default
    /// when a value does not parse or collides with an earlier one.
    pub fn bindings(&self) -> Vec<(KeyAction, crate::core::keys::Chord)> {
        let mut used: Vec<crate::core::keys::Chord> = Vec::new();
        let mut out = Vec::with_capacity(ACTIONS.len());
        for action in ACTIONS {
            let mut chord = crate::core::keys::parse(self.chord_text(action))
                .unwrap_or_else(|_| action.default());
            if used.contains(&chord) {
                chord = action.default();
            }
            if used.contains(&chord) {
                continue;
            }
            used.push(chord);
            out.push((action, chord));
        }
        out
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub tabs: Vec<SavedTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSets {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub sets: Vec<SavedSet>,
}

impl Default for SavedSets {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            sets: Vec::new(),
        }
    }
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}
fn preview_delay_default() -> u64 {
    crate::timings::DEFAULT_PREVIEW_DELAY_MS
}
fn yes() -> bool {
    true
}
fn no() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_hover_preview_off() {
        let s = Settings::default();
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert!(s.tabs.icons && s.tabs.count_badge);
        assert!(!s.tabs.hover_preview);
    }

    #[test]
    fn settings_round_trip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.theme, s.theme);
        assert_eq!(back.tabs.icons, s.tabs.icons);
        assert_eq!(back.keybinds, s.keybinds);
    }

    #[test]
    fn unknown_theme_and_accent_fall_back() {
        let back: Settings =
            serde_json::from_str(r#"{ "theme": "drak", "accent": "tangerine" }"#).unwrap();
        assert_eq!(back.theme, ThemeMode::System);
        assert_eq!(back.accent.name(), "blue");
    }

    #[test]
    fn keybinds_defaults_match_actions() {
        let keybinds = Keybinds::default();
        let bindings = keybinds.bindings();
        assert_eq!(bindings.len(), ACTIONS.len());
        for (action, chord) in bindings {
            assert_eq!(chord, action.default());
        }
    }

    #[test]
    fn keybinds_falls_back_to_defaults() {
        let keybinds = serde_json::from_str::<Keybinds>(
            "{ \"next_tab\": \"nonsense\", \"prev_tab\": \"ctrl+tab\" }",
        )
        .unwrap();
        let bindings = keybinds.bindings();
        let find = |want: KeyAction| {
            bindings
                .iter()
                .find_map(|(action, chord)| (action == &want).then_some(*chord))
        };
        assert_eq!(find(KeyAction::NextTab), Some(KeyAction::NextTab.default()));
        assert_eq!(find(KeyAction::PrevTab), Some(KeyAction::PrevTab.default()));
        assert_eq!(
            find(KeyAction::CloseTab),
            Some(KeyAction::CloseTab.default())
        );
        assert_eq!(
            find(KeyAction::SelectTab),
            Some(KeyAction::SelectTab.default())
        );
    }

    #[test]
    fn keybinds_deduplicates_chords() {
        let keybinds = serde_json::from_str::<Keybinds>(
            "{ \"next_tab\": \"ctrl+tab\", \"prev_tab\": \"ctrl+tab\" }",
        )
        .unwrap();
        let bindings = keybinds.bindings();
        let mut seen = Vec::new();
        for (_action, chord) in bindings {
            assert!(!seen.contains(&chord), "duplicate chord {chord:?}");
            seen.push(chord);
        }
        assert!(seen.contains(&KeyAction::SelectTab.default()));
    }

    #[test]
    fn app_rules_round_trip() {
        let mut settings = Settings::default();
        settings.rules.push(AppRule {
            exe: "chrome.exe".into(),
            never_host: true,
            new_group: true,
            label: Some("Browser".into()),
        });
        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rules, settings.rules);
    }

    #[test]
    fn install_record_round_trips() {
        let record = InstallRecord {
            path: "C:\\apps\\slate.exe".into(),
            version: "0.1.1".into(),
            pid: 4321,
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: InstallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn install_record_tolerates_missing_fields() {
        let back: InstallRecord = serde_json::from_str("{}").unwrap();
        assert_eq!(back, InstallRecord::default());
    }

    #[test]
    fn saved_sets_round_trip() {
        let sets = SavedSets {
            schema_version: SCHEMA_VERSION,
            sets: vec![SavedSet {
                id: 1,
                name: "Work".into(),
                tabs: vec![SavedTab {
                    exe_path: "C:\\x\\chrome.exe".into(),
                    exe_name: "chrome.exe".into(),
                    title_hint: "Tab".into(),
                    accent: None,
                    pid: Some(42),
                    rect: Some([0, 0, 1280, 800]),
                }],
            }],
        };
        let json = serde_json::to_string(&sets).unwrap();
        let back: SavedSets = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sets.len(), 1);
        assert_eq!(back.sets[0].name, "Work");
        assert_eq!(back.sets[0].tabs[0].exe_name, "chrome.exe");
    }
}
