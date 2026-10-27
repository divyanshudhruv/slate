use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::Controller;
use crate::config;
use crate::win::{enumerate_windows, is_blacklisted};

impl Controller {
    pub(super) fn set_saved_menu(&mut self, open: bool) {
        if open {
            self.close_other_dropdowns();
            self.windows = enumerate_windows();
            self.refresh_saved_avail();
        }
        self.saved_menu_open = open;
    }

    pub(super) fn refresh_saved_avail(&mut self) {
        self.saved_avail = self
            .saved_sets
            .iter()
            .map(|set| (set.id, self.resolve_set(set).len()))
            .collect();
    }

    pub(super) fn resolve_set(&self, set: &config::SavedSet) -> Vec<isize> {
        match_tabs(&set.tabs, &self.live_targets())
            .into_iter()
            .map(|(_, hwnd)| hwnd)
            .collect()
    }

    /// Windows that can still be added.
    fn live_targets(&self) -> Vec<Target> {
        self.windows
            .iter()
            .filter(|w| !self.styling.contains_key(&w.hwnd))
            .filter(|w| !is_blacklisted(&w.class) && !w.elevated)
            .map(|w| Target::new(w.hwnd, &w.exe_path, &w.title))
            .collect()
    }

    /// Live windows plus hosted tabs, so a restore recolors open tabs too.
    fn restore_targets(&self) -> Vec<Target> {
        let mut targets = self.live_targets();
        let Some(group) = self.active_group.and_then(|id| self.groups.group(id)) else {
            return targets;
        };
        for tab in &group.tabs {
            if targets.iter().any(|t| t.hwnd == tab.window.hwnd) {
                continue;
            }
            targets.push(Target::new(
                tab.window.hwnd,
                &tab.window.exe_path,
                &tab.window.title,
            ));
        }
        targets
    }

    pub(super) fn commit_save_set(&mut self, name: String) {
        self.saved_prompt = None;
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let tabs = self.current_tabs();
        if tabs.is_empty() {
            return;
        }
        if let Some(existing) = self.saved_sets.iter_mut().find(|s| s.name == name) {
            existing.tabs = tabs;
        } else {
            let id = self.saved_sets.iter().map(|s| s.id).max().unwrap_or(0) + 1;
            self.saved_sets.push(config::SavedSet { id, name, tabs });
        }
        self.persist_sets();
        self.refresh_saved_avail();
    }

    pub(super) fn commit_rename_set(&mut self, id: u64, name: String) {
        self.saved_prompt = None;
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Some(set) = self.saved_sets.iter_mut().find(|s| s.id == id) {
            set.name = name;
            self.persist_sets();
        }
    }

    pub(super) fn delete_set(&mut self, id: u64) {
        let before = self.saved_sets.len();
        self.saved_sets.retain(|s| s.id != id);
        if self.saved_sets.len() != before {
            self.persist_sets();
        }
    }

    pub(super) fn restore_set(&mut self, id: u64) {
        let Some(set) = self.saved_sets.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.windows = enumerate_windows();
        self.restore_tabs(&set.tabs);
        self.saved_menu_open = false;
    }

    pub(super) fn restore_tabs(&mut self, tabs: &[config::SavedTab]) {
        let pairs = match_tabs(tabs, &self.restore_targets());
        for (_, hwnd) in &pairs {
            self.add_window(*hwnd);
        }
        for (ix, hwnd) in pairs {
            let Some(accent) = tabs[ix].accent else {
                continue;
            };
            if let Some((group, tab)) = self.tab_of_hwnd(hwnd) {
                self.set_tab_color(group, tab, Some(accent));
            }
        }
    }

    fn persist_sets(&self) {
        let sets = config::SavedSets {
            schema_version: config::SCHEMA_VERSION,
            sets: self.saved_sets.clone(),
        };
        let _ = config::save_sets(&sets);
    }

    fn current_tabs(&self) -> Vec<config::SavedTab> {
        let Some(group) = self.active_group.and_then(|id| self.groups.group(id)) else {
            return Vec::new();
        };
        group
            .tabs
            .iter()
            .map(|t| config::SavedTab {
                exe_path: t.window.exe_path.clone(),
                exe_name: t.window.exe_name.clone(),
                title_hint: t.window.title.clone(),
                accent: self.tab_colors.get(&t.window.hwnd).copied(),
            })
            .collect()
    }

    pub(super) fn restore_session(&mut self) {
        if !self.settings.restore_session {
            return;
        }
        let tabs = config::load_session().tabs;
        if tabs.is_empty() {
            return;
        }
        self.windows = enumerate_windows();
        self.restore_tabs(&tabs);
    }

    pub(super) fn persist_session(&self) {
        let _ = config::save_session(&config::Session {
            tabs: self.current_tabs(),
        });
    }

    pub(super) fn session_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        match self.active_group.and_then(|id| self.groups.group(id)) {
            Some(group) => {
                group.tabs.len().hash(&mut hasher);
                for tab in &group.tabs {
                    tab.window.hwnd.hash(&mut hasher);
                    self.tab_colors.get(&tab.window.hwnd).hash(&mut hasher);
                }
            }
            None => 0u8.hash(&mut hasher),
        }
        hasher.finish()
    }
}

struct Target {
    hwnd: isize,
    exe_path: String,
    title: String,
}

impl Target {
    fn new(hwnd: isize, exe_path: &str, title: &str) -> Self {
        Self {
            hwnd,
            exe_path: exe_path.to_string(),
            title: title.to_string(),
        }
    }
}

/// Pairs saved tabs with windows, exact titles first, so a tab whose title
/// changed cannot take a window another tab still matches by name.
fn match_tabs(tabs: &[config::SavedTab], targets: &[Target]) -> Vec<(usize, isize)> {
    let mut taken = vec![false; targets.len()];
    let mut pairs = Vec::with_capacity(tabs.len());
    let mut pending = Vec::new();
    for (ix, tab) in tabs.iter().enumerate() {
        let hit = targets.iter().enumerate().find(|(ti, target)| {
            !taken[*ti]
                && target.exe_path.eq_ignore_ascii_case(&tab.exe_path)
                && target.title == tab.title_hint
        });
        match hit {
            Some((ti, target)) => {
                taken[ti] = true;
                pairs.push((ix, target.hwnd));
            }
            None => pending.push(ix),
        }
    }
    for ix in pending {
        let tab = &tabs[ix];
        let hit = targets.iter().enumerate().find(|(ti, target)| {
            !taken[*ti] && target.exe_path.eq_ignore_ascii_case(&tab.exe_path)
        });
        if let Some((ti, target)) = hit {
            taken[ti] = true;
            pairs.push((ix, target.hwnd));
        }
    }
    pairs.sort_by_key(|(ix, _)| *ix);
    pairs
}
