use gpui::Context;

use super::dto::{Cmd, SavedPrompt, TabMenu};
use super::Controller;
use crate::config;
use crate::win::{activate_window, enumerate_windows, install_window_icon};

impl Controller {
    pub fn dispatch(&mut self, cmd: Cmd, cx: &mut Context<Self>) {
        self.invalidate_info();
        self.drain_events();
        match cmd {
            Cmd::Refresh => self.windows = enumerate_windows(),
            Cmd::HostReady { hwnd, titlebar } => {
                self.host_hwnd = Some(hwnd);
                self.titlebar = titlebar;
                install_window_icon(hwnd);
                if let Some(tray) = &self.tray {
                    tray.set_host(hwnd);
                }
                self.worker.start();
                self.worker.install_move_tracker(hwnd);
                self.restore_session();
            }
            Cmd::SelectTab { group, tab } => self.select_tab(group, tab),
            Cmd::Ungroup { group, tab } => {
                if let Some(hwnd) = self.hwnd_of_tab(group, tab) {
                    self.ungroup(group, tab);
                    activate_window(hwnd);
                }
            }
            Cmd::CloseTab { group, tab } => self.close_tab(group, tab),
            Cmd::DetachTabOut {
                from,
                screen_x,
                screen_y,
            } => self.detach_tab_out(from, screen_x, screen_y),
            Cmd::ReorderTab { group, from, to } => self.reorder(group, from, to),
            Cmd::BeginPick => self.begin_pick(cx),
            Cmd::ToggleIconsInTabs => self.toggle_icons_in_tabs(),
            Cmd::SetOverlay(open) => self.overlay = open,
            Cmd::ToggleOverlay => self.overlay = !self.overlay,
            Cmd::SetTabMenu(open) => {
                if open {
                    self.close_other_dropdowns();
                }
                self.tab_menu_open = open;
            }
            Cmd::ToggleTabMenu => {
                let open = !self.tab_menu_open;
                if open {
                    self.close_other_dropdowns();
                }
                self.tab_menu_open = open;
            }
            Cmd::CloseOtherTabs { group, tab } => self.close_other_tabs(group, tab),
            Cmd::CloseTabsToRight { group, tab } => self.close_tabs_to_right(group, tab),
            Cmd::CloseTabsToLeft { group, tab } => self.close_tabs_to_left(group, tab),
            Cmd::OpenTabMenu { tab, at } => {
                self.tab_menu = Some(TabMenu { tab, at });
                self.tab_menu_open = false;
                self.picker_open = false;
                self.saved_menu_open = false;
            }
            Cmd::CloseTabMenu => self.tab_menu = None,
            Cmd::BeginRename { group, tab } => {
                self.tab_menu = None;
                self.renaming = Some((group, tab));
            }
            Cmd::CancelRename => self.renaming = None,
            Cmd::RenameTab { group, tab, title } => self.rename_tab(group, tab, title),
            Cmd::SetPickerOpen(open) => self.set_picker_open(open),
            Cmd::TogglePicker => {
                let open = !self.picker_open;
                self.set_picker_open(open);
            }
            Cmd::TogglePickerPick { hwnd } => self.toggle_picker_pick(hwnd),
            Cmd::MovePickerCursor { delta } => self.move_picker_cursor(delta),
            Cmd::TogglePickerCursor => self.toggle_picker_cursor(),
            Cmd::AddPicked => {
                for hwnd in std::mem::take(&mut self.picker_picks) {
                    self.add_window(hwnd);
                }
            }
            Cmd::SetSavedMenu(open) => self.set_saved_menu(open),
            Cmd::ToggleSavedMenu => {
                let open = !self.saved_menu_open;
                self.set_saved_menu(open);
            }
            Cmd::BeginSaveSet => {
                self.saved_menu_open = false;
                self.saved_prompt = Some(SavedPrompt::Save);
            }
            Cmd::BeginRenameSet { id } => {
                if self.saved_sets.iter().any(|s| s.id == id) {
                    self.saved_menu_open = false;
                    self.saved_prompt = Some(SavedPrompt::Rename(id));
                }
            }
            Cmd::CancelSavedPrompt => self.saved_prompt = None,
            Cmd::CommitSaveSet { name } => self.commit_save_set(name),
            Cmd::CommitRenameSet { id, name } => self.commit_rename_set(id, name),
            Cmd::RestoreSet { id } => self.restore_set(id),
            Cmd::DeleteSet { id } => self.delete_set(id),
            Cmd::ExportSets => self.export_sets(cx),
            Cmd::ImportSets => self.import_sets(cx),
            Cmd::ToggleTabBadge => self.toggle_tab_badge(),
            Cmd::ToggleHoverPreview => self.toggle_hover_preview(),
            Cmd::ToggleTheme => self.toggle_theme(cx),
            Cmd::ToggleRestoreSession => {
                self.settings.restore_session = !self.settings.restore_session;
                self.save_settings();
            }
            Cmd::SetTabColor { group, tab, color } => self.set_tab_color(group, tab, color),
            Cmd::PreviewTab { tab, anchor } => self.preview_hover(tab, anchor, cx),
            Cmd::PreviewHide { tab } => self.preview_unhover(tab, cx),
            Cmd::PreviewCardHover(hovered) => self.preview_card_hover(hovered, cx),
            Cmd::DismissPreview => self.dismiss_hover_preview(),
            Cmd::Shutdown => self.shutdown(),
            Cmd::ToggleTabBar => {
                self.settings.tabs.show_bar = !self.settings.tabs.show_bar;
                self.save_settings();
            }
            Cmd::OpenRepository => {
                crate::win::shell::open_url(crate::core::update::REPOSITORY_URL);
            }
            Cmd::CheckForUpdates => self.check_for_updates(cx),
            Cmd::InstallAvailableUpdate => {
                if let crate::core::UpdateState::Available(version) = self.update.clone() {
                    self.install_update_from_dialog(version, cx);
                }
            }
            Cmd::OpenSlateFolder => {
                crate::win::shell::open_folder(&config::paths::data_dir());
            }
            Cmd::ReloadSettings => self.reload_settings(cx),
            Cmd::OpenKeybinds => self.open_keybinds(),
            Cmd::CloseDialog => self.close_dialog(),
            Cmd::SetKeybindChord { action, chord } => self.set_keybind_chord(action, chord, cx),
            Cmd::ResetKeybinds => self.reset_keybinds(cx),
            Cmd::CommitKeybinds => self.commit_keybinds(cx),
        }
        self.sync_worker();
        cx.notify();
    }

    pub(super) fn close_other_dropdowns(&mut self) {
        self.picker_open = false;
        self.saved_menu_open = false;
        self.tab_menu_open = false;
        self.tab_menu = None;
    }
}
