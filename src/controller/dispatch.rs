use gpui::Context;

use super::dto::{BannerAction, Cmd, SavedPrompt, TabMenu};
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
                // The first frame is up, so a swapped-in build has proven it launches.
                crate::win::update::clear_update_marker();
                if let Some(tray) = &self.tray {
                    tray.set_host(hwnd);
                }
                self.worker.start();
                self.worker.install_move_tracker(hwnd);
                self.restore_session();
                self.check_for_updates(false, cx);
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
            Cmd::SetOverlay(open) => self.menus.overlay = open,
            Cmd::ToggleOverlay => self.menus.overlay = !self.menus.overlay,
            Cmd::SetTabMenu(open) => {
                if open {
                    self.close_other_dropdowns();
                }
                self.menus.tab_menu_open = open;
                self.menus.tab_menu_cursor = None;
            }
            Cmd::ToggleTabMenu => {
                let open = !self.menus.tab_menu_open;
                if open {
                    self.close_other_dropdowns();
                }
                self.menus.tab_menu_open = open;
                self.menus.tab_menu_cursor = None;
            }
            Cmd::MoveTabMenuCursor { delta } => self.move_tab_menu_cursor(delta),
            Cmd::ConfirmTabMenuCursor => self.open_tab_menu_cursor(),
            Cmd::CloseOtherTabs { group, tab } => self.close_other_tabs(group, tab),
            Cmd::CloseTabsToRight { group, tab } => self.close_tabs_to_right(group, tab),
            Cmd::CloseTabsToLeft { group, tab } => self.close_tabs_to_left(group, tab),
            Cmd::OpenTabMenu { tab, at } => {
                self.menus.tab_menu = Some(TabMenu { tab, at });
                self.menus.tab_menu_open = false;
                self.menus.picker_open = false;
                self.menus.saved_menu_open = false;
            }
            Cmd::CloseTabMenu => self.menus.tab_menu = None,
            Cmd::BeginRename { group, tab } => {
                self.menus.tab_menu = None;
                self.menus.renaming = Some((group, tab));
            }
            Cmd::CancelRename => self.menus.renaming = None,
            Cmd::RenameTab { group, tab, title } => self.rename_tab(group, tab, title),
            Cmd::SetPickerOpen(open) => self.set_picker_open(open),
            Cmd::TogglePicker => {
                let open = !self.menus.picker_open;
                self.set_picker_open(open);
            }
            Cmd::TogglePickerPick { hwnd } => self.toggle_picker_pick(hwnd),
            Cmd::MovePickerCursor { delta } => self.move_picker_cursor(delta),
            Cmd::TogglePickerCursor => self.toggle_picker_cursor(),
            Cmd::AddPicked => {
                for hwnd in std::mem::take(&mut self.menus.picker_picks) {
                    self.add_window(hwnd);
                }
            }
            Cmd::SetSavedMenu(open) => self.set_saved_menu(open),
            Cmd::ToggleSavedMenu => {
                let open = !self.menus.saved_menu_open;
                self.set_saved_menu(open);
            }
            Cmd::BeginSaveSet => {
                self.menus.saved_menu_open = false;
                self.menus.saved_prompt = Some(SavedPrompt::Save);
            }
            Cmd::BeginRenameSet { id } => {
                if self.saved_sets.iter().any(|s| s.id == id) {
                    self.menus.saved_menu_open = false;
                    self.menus.saved_prompt = Some(SavedPrompt::Rename(id));
                }
            }
            Cmd::CancelSavedPrompt => self.menus.saved_prompt = None,
            Cmd::CommitSaveSet { name } => self.commit_save_set(name),
            Cmd::CommitRenameSet { id, name } => self.commit_rename_set(id, name),
            Cmd::RestoreSet { id } => self.restore_set(id),
            Cmd::DeleteSet { id } => self.delete_set(id),
            Cmd::CopyDiagnostics => self.copy_diagnostics(cx),
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
            Cmd::CheckForUpdates { force } => self.check_for_updates(force, cx),
            Cmd::ToggleUpdateChannel => self.toggle_update_channel(cx),
            Cmd::OpenSlateFolder => {
                crate::win::shell::open_folder(&config::paths::data_dir());
            }
            Cmd::ReloadSettings => self.reload_settings(cx),
            Cmd::OpenKeybinds => self.open_keybinds(),
            Cmd::CloseDialog => self.close_dialog(),
            Cmd::SetKeybindChord { action, chord } => self.set_keybind_chord(action, chord, cx),
            Cmd::ResetKeybinds => self.reset_keybinds(cx),
            Cmd::CommitKeybinds => self.commit_keybinds(cx),
            Cmd::RestoreRecovered => {
                self.restore_recovered();
                self.close_dialog();
            }
            Cmd::DismissRecovered => {
                self.dismiss_recovered();
                self.close_dialog();
            }
            Cmd::BannerAction(action) => match action {
                BannerAction::InstallUpdate => {
                    if let crate::core::UpdateState::Available(version) = self.updater.state.clone()
                    {
                        self.install_available_update(version, cx);
                    }
                }
                BannerAction::RetryUpdateCheck => {
                    self.check_for_updates(true, cx);
                }
                BannerAction::RestartUpdate => self.restart_for_update(cx),
            },
            Cmd::DismissBanner => self.dismiss_banner(cx),
            Cmd::DismissUpdateBanner => self.dismiss_update_banner(cx),
        }
        self.sync_worker();
        cx.notify();
    }

    pub(super) fn close_other_dropdowns(&mut self) {
        self.menus.picker_open = false;
        self.menus.saved_menu_open = false;
        self.menus.tab_menu_open = false;
        self.menus.tab_menu_cursor = None;
        self.menus.tab_menu = None;
    }
}
