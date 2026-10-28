use std::sync::Arc;

use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::TextInput;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::ActiveTheme;
use ely_gpui_component::typography::{Caption, Kbd};
use gpui::prelude::*;
use gpui::{
    div, App, Context, ElementId, Entity, IntoElement, MouseButton, ParentElement, Render,
    RenderImage, ScrollHandle, Size, Styled, Subscription, Window,
};

use crate::controller::{
    BannerAction, BannerKind, Cmd, Controller, HostInfo, SavedPrompt, TabInfo,
};
use crate::core::{GroupId, TabId, UpdateState};
use crate::ui::tab_bar::{SlateTab, SlateTabBar, TabDrag, TabIcon};
use crate::ui::window_hwnd;

mod art;
mod dialogs;
pub(crate) mod hover_card;
mod overlays;
mod panels;
mod title_bar;

use art::{empty_state, grayscale_mark, key_cap};

fn hwnd_of(window: &Window) -> Option<isize> {
    window_hwnd(window)
}

/// Which surfaces above the tab strip are open, snapshotted when the input closures are built.
#[derive(Clone, Copy)]
struct Surfaces {
    overlay: bool,
    tab_menu: bool,
    picker: bool,
    saved: bool,
    ctx_menu: bool,
    renaming: bool,
    prompt: bool,
    preview: bool,
    dialog: bool,
}

impl Surfaces {
    fn of(info: &HostInfo) -> Self {
        Self {
            overlay: info.overlay,
            tab_menu: info.tab_menu_open,
            picker: info.picker_open,
            saved: info.saved_menu_open,
            ctx_menu: info.tab_menu.is_some(),
            renaming: info.renaming.is_some(),
            prompt: info.saved_prompt.is_some(),
            preview: info.hover_preview.is_some(),
            dialog: info.dialog.is_some(),
        }
    }

    /// A click in the body closes these; a rename or a prompt owns its own focus.
    fn dismissable(&self) -> bool {
        self.overlay || self.tab_menu || self.picker || self.saved || self.ctx_menu || self.dialog
    }

    fn escapable(&self) -> bool {
        self.overlay
            || self.tab_menu
            || self.picker
            || self.saved
            || self.renaming
            || self.prompt
            || self.preview
            || self.dialog
    }
}

pub struct HostWindow {
    controller: Entity<Controller>,
    ready: bool,
    close_routed: bool,
    empty_icon: Option<Arc<RenderImage>>,
    rename_field: Option<Entity<TextInput>>,
    rename_primed: Option<TabId>,
    saved_field: Option<Entity<TextInput>>,
    saved_primed: Option<SavedPrompt>,
    tab_scroll: ScrollHandle,
    _observe_controller: Subscription,
}

pub(super) struct HostCtx<'a> {
    pub(super) window: &'a mut Window,
    pub(super) cx: &'a mut App,
    pub(super) controller: Entity<Controller>,
    pub(super) info: &'a crate::controller::HostInfo,
    pub(super) group: Option<GroupId>,
    pub(super) viewport: Size<gpui::Pixels>,
    pub(super) palette: ely_gpui_component::theme::Palette,
}

impl HostWindow {
    pub fn new(controller: Entity<Controller>, cx: &mut Context<Self>) -> Self {
        let _observe_controller = cx.observe(&controller, |_this, _controller, cx| cx.notify());
        Self {
            controller,
            ready: false,
            close_routed: false,
            empty_icon: grayscale_mark(),
            rename_field: None,
            rename_primed: None,
            saved_field: None,
            saved_primed: None,
            tab_scroll: ScrollHandle::new(),
            _observe_controller,
        }
    }
}

/// Pins a banner under the title strip. Absolute, so showing one never reflows the body.
fn render_banner(
    info: &crate::controller::HostInfo,
    controller: Entity<Controller>,
    top: gpui::Rems,
) -> gpui::AnyElement {
    let strip = banner_strip(info, controller);
    div()
        .absolute()
        .top(top)
        .left_0()
        .right_0()
        .when_some(strip, |host, strip| host.child(strip))
        .into_any_element()
}

/// Which banner wins: a message first, then what the updater has to say.
fn banner_strip(
    info: &crate::controller::HostInfo,
    controller: Entity<Controller>,
) -> Option<gpui::AnyElement> {
    if let Some(view) = info.banner.as_ref() {
        let banner = match &view.kind {
            BannerKind::Message {
                severity,
                text,
                action,
            } => {
                let banner = Banner::new(
                    ElementId::named_usize("banner", view.id as usize),
                    *severity,
                    text.clone(),
                );
                let banner = match action.clone() {
                    Some((label, action)) => {
                        let ctrl = controller.clone();
                        banner.action(label, move |_window, cx| {
                            ctrl.update(cx, |c, cx| c.dispatch(Cmd::BannerAction(action), cx));
                        })
                    }
                    None => banner,
                };
                let ctrl = controller.clone();
                banner.on_dismiss(move |_window, cx| {
                    ctrl.update(cx, |c, cx| c.dispatch(Cmd::DismissBanner, cx));
                })
            }
            BannerKind::Pick => Banner::new(
                ElementId::named_usize("pick-banner", 0),
                Severity::Info,
                "Hover a window, then press Tab to add it, or press Escape to cancel.",
            ),
        };
        return Some(banner.into_any_element());
    }
    // The updater uses the same banner, wearing the severity that fits the state.
    let (severity, text, action) =
        update_banner(&info.update, info.update_banner_hidden.as_deref())?;
    let mut banner = Banner::new(ElementId::named_usize("update-banner", 0), severity, text);
    if let Some((label, action)) = action {
        let ctrl = controller.clone();
        banner = banner.action(label, move |_window, cx| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::BannerAction(action), cx));
        });
    }
    // Only an offer can be waved off; a download or a restart is already underway.
    if matches!(info.update, UpdateState::Available(_)) {
        let ctrl = controller.clone();
        banner = banner.on_dismiss(move |_window, cx| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::DismissUpdateBanner, cx));
        });
    }
    Some(banner.into_any_element())
}

/// The severity, text and optional action for one update banner.
type UpdateBanner = (Severity, String, Option<(&'static str, BannerAction)>);

/// Which updater banner says something worth a strip, and whether a version was waved off.
fn update_banner(update: &UpdateState, hidden: Option<&str>) -> Option<UpdateBanner> {
    match update {
        UpdateState::Available(version) => {
            if hidden == Some(version.as_str()) {
                return None;
            }
            Some((
                Severity::Info,
                format!("Slate v{version} is available."),
                Some(("Download", BannerAction::InstallUpdate)),
            ))
        }
        UpdateState::Downloading(version, percent) => Some((
            Severity::Info,
            format!("Downloading Slate v{version}: {percent}%"),
            None,
        )),
        UpdateState::Installing(version) => Some((
            Severity::Warning,
            format!("Slate v{version} is ready - restart to finish."),
            Some(("Restart", BannerAction::RestartUpdate)),
        )),
        _ => None,
    }
}

/// Builds the tab strip; every behaviour only forwards a `Cmd` to the controller.
fn build_tab_bar(
    controller: &Entity<Controller>,
    info: &HostInfo,
    cx: &Context<HostWindow>,
) -> SlateTabBar {
    let group = info.group;
    let palette = cx.theme().palette();
    let mut tab_bar = SlateTabBar::new("host-tabs");
    for TabInfo {
        id,
        title,
        icon,
        accent,
        ..
    } in &info.tabs
    {
        let icon = match (info.icons_in_tabs, icon) {
            (true, Some(image)) => TabIcon::Real(image.clone()),
            _ => TabIcon::Glyph(ely_gpui_component::primitives::IconName::AppWindow),
        };
        let color = accent.map(|index| palette.hue(index, "tab accent"));
        tab_bar = tab_bar.tab(SlateTab::new(id.0.to_string(), title.clone(), icon).accent(color));
    }
    if let Some(active) = &info.active {
        tab_bar = tab_bar.selected(gpui::SharedString::from(active.0.to_string()));
    }
    tab_bar = tab_bar.on_select({
        let ctrl = controller.clone();
        move |id, _w, cx| {
            if let (Some(g), Ok(n)) = (group, id.to_string().parse::<u64>()) {
                ctrl.update(cx, |c, cx| {
                    c.dispatch(
                        Cmd::SelectTab {
                            group: g,
                            tab: TabId(n),
                        },
                        cx,
                    );
                });
            }
        }
    });
    tab_bar = tab_bar.on_close({
        let ctrl = controller.clone();
        move |id, _w, cx| {
            if let (Some(g), Ok(n)) = (group, id.to_string().parse::<u64>()) {
                ctrl.update(cx, |c, cx| {
                    c.dispatch(
                        Cmd::Ungroup {
                            group: g,
                            tab: TabId(n),
                        },
                        cx,
                    );
                });
            }
        }
    });
    tab_bar = tab_bar.on_reorder({
        let ctrl = controller.clone();
        move |from, to, _w, cx| {
            if let Some(g) = group {
                ctrl.update(cx, |c, cx| {
                    c.dispatch(Cmd::ReorderTab { group: g, from, to }, cx);
                });
            }
        }
    });
    tab_bar = tab_bar.on_context({
        let ctrl = controller.clone();
        move |id, position, _w, cx| {
            if let (Some(_g), Ok(n)) = (group, id.to_string().parse::<u64>()) {
                ctrl.update(cx, |c, cx| {
                    c.dispatch(
                        Cmd::OpenTabMenu {
                            tab: TabId(n),
                            at: (position.x.as_f32(), position.y.as_f32()),
                        },
                        cx,
                    );
                });
            }
        }
    });
    tab_bar = tab_bar.on_tab_hover({
        let ctrl = controller.clone();
        move |id, hovered, bounds, _window, cx| {
            let (Some(_g), Ok(n)) = (group, id.to_string().parse::<u64>()) else {
                return;
            };
            let tab = TabId(n);
            if hovered {
                let anchor = (
                    bounds.origin.x.as_f32(),
                    bounds.origin.y.as_f32(),
                    bounds.size.width.as_f32(),
                    bounds.size.height.as_f32(),
                );
                ctrl.update(cx, |c, cx| {
                    c.dispatch(Cmd::PreviewTab { tab, anchor }, cx);
                });
            } else {
                ctrl.update(cx, |c, cx| c.dispatch(Cmd::PreviewHide { tab }, cx));
            }
        }
    });
    tab_bar
}

impl Render for HostWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controller = self.controller.clone();

        if !self.ready {
            if let Some(hwnd) = hwnd_of(window) {
                let titlebar_h = {
                    let theme = cx.theme();
                    theme
                        .titlebar_height()
                        .to_pixels(window.rem_size())
                        .as_f32()
                };
                controller.update(cx, |c, cx| {
                    c.dispatch(
                        Cmd::HostReady {
                            hwnd,
                            titlebar: titlebar_h,
                        },
                        cx,
                    );
                });
                self.ready = true;
            }
        }

        if !self.close_routed {
            let ctrl = controller.clone();
            window.on_window_should_close(cx, move |window, cx| {
                ctrl.update(cx, |c, cx| c.dispatch(Cmd::Shutdown, cx));
                let own = window.window_handle().window_id();
                for handle in cx.windows() {
                    if handle.window_id() != own {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                }
                window.remove_window();
                false
            });
            self.close_routed = true;
        }

        let (titlebar_h, bg) = {
            let theme = cx.theme();
            (theme.titlebar_height(), theme.colors.bg)
        };

        let info = self.controller.read_with(cx, |c, _| c.host_info());
        let group = info.group;

        let palette = cx.theme().palette();
        let tab_bar = build_tab_bar(&controller, &info, cx);

        let viewport = window.viewport_size();

        let title = {
            let mut ctx = HostCtx {
                window: &mut *window,
                cx: &mut *cx,
                controller: controller.clone(),
                info: info.as_ref(),
                group,
                viewport,
                palette,
            };
            title_bar::build(&mut ctx, tab_bar, titlebar_h, bg, &self.tab_scroll)
        };

        let dismiss = {
            let ctrl = controller.clone();
            let open = Surfaces::of(&info);
            move |_ev: &gpui::MouseDownEvent, _w: &mut Window, cx: &mut App| {
                if !open.dismissable() {
                    return;
                }
                ctrl.update(cx, |c, cx| {
                    if open.overlay {
                        c.dispatch(Cmd::SetOverlay(false), cx);
                    }
                    if open.tab_menu {
                        c.dispatch(Cmd::SetTabMenu(false), cx);
                    }
                    if open.picker {
                        c.dispatch(Cmd::SetPickerOpen(false), cx);
                    }
                    if open.saved {
                        c.dispatch(Cmd::SetSavedMenu(false), cx);
                    }
                    if open.ctx_menu {
                        c.dispatch(Cmd::CloseTabMenu, cx);
                    }
                    if open.dialog {
                        c.dispatch(Cmd::CloseDialog, cx);
                    }
                });
            }
        };

        let escape = {
            let ctrl = controller.clone();
            let open = Surfaces::of(&info);
            move |ev: &gpui::KeyDownEvent, _w: &mut Window, cx: &mut App| {
                let key = ev.keystroke.key.as_str();
                if open.picker && matches!(key, "up" | "down" | "enter") {
                    cx.stop_propagation();
                    ctrl.update(cx, |c, cx| match key {
                        "up" => c.dispatch(Cmd::MovePickerCursor { delta: -1 }, cx),
                        "down" => c.dispatch(Cmd::MovePickerCursor { delta: 1 }, cx),
                        _ => c.dispatch(Cmd::TogglePickerCursor, cx),
                    });
                    return;
                }
                if open.tab_menu && matches!(key, "up" | "down" | "enter") {
                    cx.stop_propagation();
                    ctrl.update(cx, |c, cx| match key {
                        "up" => c.dispatch(Cmd::MoveTabMenuCursor { delta: -1 }, cx),
                        "down" => c.dispatch(Cmd::MoveTabMenuCursor { delta: 1 }, cx),
                        _ => c.dispatch(Cmd::ConfirmTabMenuCursor, cx),
                    });
                    return;
                }
                if key != "escape" {
                    return;
                }
                if !open.escapable() {
                    return;
                }
                ctrl.update(cx, |c, cx| {
                    if open.overlay {
                        c.dispatch(Cmd::SetOverlay(false), cx);
                    }
                    if open.tab_menu {
                        c.dispatch(Cmd::SetTabMenu(false), cx);
                    }
                    if open.picker {
                        c.dispatch(Cmd::SetPickerOpen(false), cx);
                    }
                    if open.saved {
                        c.dispatch(Cmd::SetSavedMenu(false), cx);
                    }
                    if open.renaming {
                        c.dispatch(Cmd::CancelRename, cx);
                    }
                    if open.prompt {
                        c.dispatch(Cmd::CancelSavedPrompt, cx);
                    }
                    if open.preview {
                        c.dispatch(Cmd::DismissPreview, cx);
                    }
                    if open.dialog {
                        c.dispatch(Cmd::CloseDialog, cx);
                    }
                });
            }
        };

        let detach_out = {
            let ctrl = controller.clone();
            move |drag: &TabDrag, window: &mut Window, cx: &mut App| {
                let origin = window.bounds().origin;
                let mouse = window.mouse_position();
                let screen_x = origin.x.as_f32() + mouse.x.as_f32();
                let screen_y = origin.y.as_f32() + mouse.y.as_f32();
                ctrl.update(cx, |c, cx| {
                    c.dispatch(
                        Cmd::DetachTabOut {
                            from: drag.from,
                            screen_x,
                            screen_y,
                        },
                        cx,
                    );
                });
            }
        };

        let content = div()
            .id("host-content")
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .on_mouse_down(MouseButton::Left, dismiss)
            .capture_key_down(escape)
            .on_drop::<TabDrag>(detach_out)
            .child(title.element);

        let body = match (info.tabs.is_empty(), info.overlaying()) {
            (true, _) => {
                let caption = Caption::new("No windows yet").into_any_element();
                let hint = div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child("Click")
                    .child(Kbd::new("+"))
                    .child("to add a window, or")
                    .child(key_cap("Drag", cx))
                    .child("one in while holding")
                    .child(key_cap("Ctrl", cx))
                    .into_any_element();
                empty_state(cx, self.empty_icon.clone(), caption, hint)
            }
            (false, true) => {
                let caption = Caption::new(info.caption.clone()).into_any_element();
                let hint = div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child("Press")
                    .child(Kbd::new("escape"))
                    .child("to bring them back")
                    .into_any_element();
                empty_state(cx, self.empty_icon.clone(), caption, hint)
            }
            (false, false) => div().flex_1(),
        };

        let mut content = content.child(body);
        if let Some(preview) = info.hover_preview.as_ref() {
            let tab = info.tabs.iter().find(|t| t.id == preview.tab);
            let on_hover = {
                let ctrl = controller.clone();
                move |hovered: &bool, _w: &mut Window, cx: &mut App| {
                    let hovered = *hovered;
                    ctrl.update(cx, |c, cx| c.dispatch(Cmd::PreviewCardHover(hovered), cx));
                }
            };
            content = content.child(hover_card::hover_card(cx, preview, tab, viewport, on_hover));
        }
        if let Some(overlay) = title.picker_overlay {
            content = content.child(overlay);
        }
        if let Some(overlay) = title.chevron_overlay {
            content = content.child(overlay);
        }
        if let Some(overlay) = title.tab_menu_overlay {
            content = content.child(overlay);
        }
        if let Some(overlay) = title.saved_overlay {
            content = content.child(overlay);
        }
        if let Some(dialog) = super::dialog_window::in_host_dialog(&controller, cx) {
            content = content.child(dialog);
        }
        if let Some(dialog) = self.rename_dialog(window, cx, info.as_ref()) {
            content = content.child(dialog);
        }
        if let Some(dialog) = self.saved_dialog(window, cx, info.as_ref()) {
            content = content.child(dialog);
        }
        // Last child, so the strip paints over the body and any overlay.
        content
            .child(render_banner(&info, controller.clone(), titlebar_h))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::update_banner;
    use crate::controller::BannerAction;
    use crate::core::UpdateState;
    use ely_gpui_component::primitives::Severity;

    #[test]
    fn an_available_update_offers_a_download() {
        let (severity, text, action) =
            update_banner(&UpdateState::Available("0.5.0".into()), None).expect("an offer");
        assert_eq!(severity, Severity::Info);
        assert_eq!(text, "Slate v0.5.0 is available.");
        let (label, action) = action.expect("an action");
        assert_eq!(label, "Download");
        assert!(matches!(action, BannerAction::InstallUpdate));
    }

    #[test]
    fn a_waved_off_version_stays_hidden() {
        assert!(update_banner(&UpdateState::Available("0.5.0".into()), Some("0.5.0")).is_none());
        assert!(
            update_banner(&UpdateState::Available("0.5.1".into()), Some("0.5.0")).is_some(),
            "a newer version shows again"
        );
    }

    #[test]
    fn a_download_reports_progress_and_cannot_be_dismissed() {
        let (severity, text, action) =
            update_banner(&UpdateState::Downloading("0.5.0".into(), 42), None).expect("progress");
        assert_eq!(severity, Severity::Info);
        assert_eq!(text, "Downloading Slate v0.5.0: 42%");
        assert!(action.is_none());
    }

    #[test]
    fn a_ready_build_offers_a_restart() {
        let (severity, _, action) =
            update_banner(&UpdateState::Installing("0.5.0".into()), None).expect("a restart");
        assert_eq!(severity, Severity::Warning);
        let (label, action) = action.expect("an action");
        assert_eq!(label, "Restart");
        assert!(matches!(action, BannerAction::RestartUpdate));
    }

    #[test]
    fn quiet_states_show_no_update_banner() {
        for state in [
            UpdateState::Idle,
            UpdateState::Checking,
            UpdateState::UpToDate,
            UpdateState::Failed,
        ] {
            assert!(update_banner(&state, None).is_none(), "{state:?}");
        }
    }
}
