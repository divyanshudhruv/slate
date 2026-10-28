use gpui::Context;

use super::dto::Dialog;
use super::Controller;
use crate::config;
use crate::core::keys::{self, Chord, KeyAction, ACTIONS};
use crate::win::Shortcut;

fn chords_conflict(
    action: KeyAction,
    chord: Chord,
    other_action: KeyAction,
    other_chord: Chord,
) -> bool {
    let same_modifiers = chord.ctrl == other_chord.ctrl
        && chord.shift == other_chord.shift
        && chord.alt == other_chord.alt
        && chord.win == other_chord.win;
    match (action, other_action) {
        (KeyAction::SelectTab, KeyAction::SelectTab) => same_modifiers,
        (KeyAction::SelectTab, _) => same_modifiers && (0x31..=0x39).contains(&other_chord.vk),
        (_, KeyAction::SelectTab) => same_modifiers && (0x31..=0x39).contains(&chord.vk),
        _ => chord == other_chord,
    }
}

fn shortcut_of(action: KeyAction) -> Shortcut {
    match action {
        KeyAction::NextTab => Shortcut::NextTab,
        KeyAction::PrevTab => Shortcut::PrevTab,
        KeyAction::CloseTab => Shortcut::CloseTab,
        KeyAction::SelectTab => Shortcut::SelectIndex(1),
    }
}

/// Rebuilds the keyboard hook's table from settings, so every chord change lands without a restart.
pub(super) fn push_shortcuts(settings: &config::Settings) {
    let table = settings
        .keybinds
        .bindings()
        .into_iter()
        .map(|(action, chord)| (shortcut_of(action), chord))
        .collect::<Vec<_>>();
    crate::win::shortcut::set_bindings(&table);
}

fn draft_from_settings(settings: &config::Settings) -> Vec<Option<Chord>> {
    ACTIONS
        .iter()
        .map(|action| {
            let text = match action {
                KeyAction::NextTab => settings.keybinds.next_tab.as_str(),
                KeyAction::PrevTab => settings.keybinds.prev_tab.as_str(),
                KeyAction::CloseTab => settings.keybinds.close_tab.as_str(),
                KeyAction::SelectTab => settings.keybinds.select_tab.as_str(),
            };
            keys::parse(text).ok().or(Some(action.default()))
        })
        .collect()
}

impl Controller {
    pub fn dialog(&self) -> Option<Dialog> {
        self.dialog
    }

    pub fn keybind_draft(&self) -> &[Option<Chord>] {
        &self.keybind_draft
    }

    pub(super) fn open_keybinds(&mut self) {
        self.close_other_dropdowns();
        self.overlay = false;
        self.tab_menu_open = false;
        self.tab_menu = None;
        self.picker_open = false;
        self.saved_menu_open = false;
        self.hover_preview = None;
        self.hide_hover_preview();
        self.keybind_draft = draft_from_settings(&self.settings);
        self.keybind_error = None;
        self.dialog = Some(Dialog::Keybinds);
    }

    pub(super) fn close_dialog(&mut self) {
        self.dialog = None;
    }

    pub(super) fn set_keybind_chord(
        &mut self,
        action: KeyAction,
        chord: Option<Chord>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = ACTIONS.iter().position(|a| a == &action) else {
            return;
        };
        // A cleared input falls back to the action's default, so no action is ever unbound.
        let mut chord = chord.unwrap_or_else(|| action.default());
        if action == KeyAction::SelectTab {
            // Only modifiers matter for "go to tab by number" - ignore the recorded key
            chord.vk = 0x31;
        }
        let conflict = ACTIONS
            .iter()
            .enumerate()
            .find(|(other_index, other_action)| {
                if *other_index == index {
                    return false;
                }
                self.keybind_draft[*other_index].is_some_and(|other_chord| {
                    chords_conflict(action, chord, **other_action, other_chord)
                })
            })
            .map(|(_, other)| *other);
        if let Some(conflict) = conflict {
            self.keybind_error = Some(format!(
                "This shortcut is already assigned to {}.",
                conflict.label()
            ));
            cx.notify();
            return;
        }
        self.keybind_draft[index] = Some(chord);
        self.keybind_error = None;
        cx.notify();
    }

    pub(super) fn reset_keybinds(&mut self, cx: &mut Context<Self>) {
        self.keybind_draft = ACTIONS
            .iter()
            .map(|action| Some(action.default()))
            .collect();
        self.keybind_error = None;
        cx.notify();
    }

    pub(super) fn commit_keybinds(&mut self, cx: &mut Context<Self>) {
        let mut chords = Vec::with_capacity(ACTIONS.len());
        for (index, action) in ACTIONS.iter().enumerate() {
            match self.keybind_draft.get(index).copied().flatten() {
                Some(chord) => chords.push((*action, chord)),
                None => {
                    self.keybind_error = Some(format!(
                        "{} needs a chord. Clear the input or reset to get a default back.",
                        action.label()
                    ));
                    cx.notify();
                    return;
                }
            }
        }
        for (action, chord) in chords {
            self.settings.keybinds.set_chord(action, chord);
        }
        self.save_settings();
        push_shortcuts(&self.settings);
        self.close_dialog();
        self.keybind_error = None;
        self.note_success("Keybinds saved", cx);
    }
}
