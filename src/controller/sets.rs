use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::Controller;
use crate::config;
use crate::core::{match_tabs, TabHint, Target};
use crate::win::{enumerate_windows, is_blacklisted, window_rect};

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
        match_tabs(&hints(&set.tabs), &self.live_targets())
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
            .map(|w| target(w.hwnd, &w.exe_path, &w.title, w.pid))
            .collect()
    }

    /// Live windows plus hosted tabs, so a restore recolors open tabs too.
    fn restore_targets(&self) -> Vec<Target> {
        let mut targets = self.live_targets();
        let Some(group) = self.active_group.and_then(|id| self.groups.group(id)) else {
            return targets;
        };
        for tab in &group.tabs {
            let window = &tab.window;
            if targets.iter().any(|t| t.hwnd == window.hwnd) {
                continue;
            }
            targets.push(target(
                window.hwnd,
                &window.exe_path,
                &window.title,
                window.pid,
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
        let pairs = match_tabs(&hints(tabs), &self.restore_targets());
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
        self.note_save(config::save_sets(&sets));
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
                pid: Some(t.window.pid),
                rect: window_rect(t.window.hwnd).map(|(x, y, w, h)| [x, y, w, h]),
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
        self.note_save(config::save_session(&config::Session {
            tabs: self.current_tabs(),
        }));
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

fn hints(tabs: &[config::SavedTab]) -> Vec<TabHint<'_>> {
    tabs.iter()
        .map(|tab| TabHint {
            exe_path: &tab.exe_path,
            title: &tab.title_hint,
            pid: tab.pid,
            rect: tab.rect,
        })
        .collect()
}

fn target(hwnd: isize, exe_path: &str, title: &str, pid: u32) -> Target {
    Target {
        hwnd,
        exe_path: exe_path.to_string(),
        title: title.to_string(),
        pid,
        rect: window_rect(hwnd).map(|(x, y, w, h)| [x, y, w, h]),
    }
}
