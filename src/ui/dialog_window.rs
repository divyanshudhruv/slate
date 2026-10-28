use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, Keystroke, Modifiers, Window};

use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::HotkeyInput;
use ely_gpui_component::overlays::Dialog;
use ely_gpui_component::primitives::{Icon, IconName};
use ely_gpui_component::theme::{ActiveTheme, IconSize};

use crate::controller::{Cmd, Controller, Dialog as SlateDialog};
use crate::core::keys::{self, Chord, KeyAction, ACTIONS};
use crate::core::update::UpdateState;

/// Dialog surfaces use Ely's Dialog component.
pub(super) fn in_host_dialog(
    controller: &Entity<Controller>,
    cx: &App,
) -> Option<gpui::AnyElement> {
    let kind = controller.read(cx).dialog()?;
    match kind {
        SlateDialog::Keybinds => Some(keybinds_dialog(controller, cx)),
        SlateDialog::Update => Some(update_dialog(controller, cx)),
        SlateDialog::Recovery => Some(recovery_dialog(controller, cx)),
    }
}

fn recovery_dialog(controller: &Entity<Controller>, cx: &App) -> gpui::AnyElement {
    let theme = cx.theme();
    let count = controller.read(cx).recovered_count();
    let title = if count == 1 {
        "Restore last window?".to_string()
    } else {
        "Restore last session?".to_string()
    };
    let note = if count == 1 {
        "Slate closed unexpectedly and freed one window. Open it as a tab again?".to_string()
    } else {
        format!("Slate closed unexpectedly and freed {count} windows. Open them as tabs again?")
    };
    let restore = {
        let ctrl = controller.clone();
        move |_ev: &gpui::ClickEvent, _w: &mut Window, cx: &mut App| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::RestoreRecovered, cx));
        }
    };
    let dismiss = {
        let ctrl = controller.clone();
        move |_ev: &gpui::ClickEvent, _w: &mut Window, cx: &mut App| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::DismissRecovered, cx));
        }
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .p(px(28.0))
        .child(
            Icon::new(IconName::Archive)
                .size(IconSize::Lg)
                .color(theme.colors.accent),
        )
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.colors.fg)
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.colors.fg_muted)
                .child(note),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .mt_2()
                .child(
                    Button::new("rec-later", "Not now")
                        .variant(ely_gpui_component::buttons::ButtonVariant::Ghost)
                        .on_click(dismiss),
                )
                .child(
                    Button::new("rec-now", "Restore")
                        .primary()
                        .on_click(restore),
                ),
        )
        .into_any_element()
}

fn stroke_of(chord: Chord) -> Option<Keystroke> {
    let key = keys::name_for(chord.vk)?;
    Some(Keystroke {
        modifiers: Modifiers {
            control: chord.ctrl,
            alt: chord.alt,
            shift: chord.shift,
            platform: chord.win,
            function: false,
        },
        key,
        key_char: None,
    })
}

fn chord_of(stroke: &Keystroke) -> Option<Chord> {
    // Shifted forms (`!`, `A`, ...) normalize to their base virtual key; shift stays a modifier.
    let vk = keys::vk_for(stroke.key.trim())?;
    let m = stroke.modifiers;
    Some(Chord {
        ctrl: m.control,
        shift: m.shift,
        alt: m.alt,
        win: m.platform,
        vk,
    })
}

fn keybinds_dialog(controller: &Entity<Controller>, cx: &App) -> gpui::AnyElement {
    let draft = controller.read(cx).keybind_draft().to_vec();
    let theme = cx.theme();

    let ctrl_close = controller.clone();
    let mut dialog = Dialog::new(
        "slate-keybinds",
        "Keyboard shortcuts",
        move |_window, cx| {
            ctrl_close.update(cx, |c, cx| c.dispatch(Cmd::CloseDialog, cx));
        },
    );

    let mut rows = Vec::with_capacity(ACTIONS.len());
    for (index, action) in ACTIONS.iter().enumerate() {
        let value = draft.get(index).copied().flatten().and_then(stroke_of);
        let action = *action;
        let on_change = {
            let ctrl = controller.clone();
            move |stroke: Option<Keystroke>, _window: &mut Window, cx: &mut App| {
                let chord = stroke.as_ref().and_then(chord_of);
                ctrl.update(cx, |c, cx| {
                    c.dispatch(Cmd::SetKeybindChord { action, chord }, cx);
                });
            }
        };
        let label = action.label();
        let desc = match action {
            KeyAction::NextTab => "Switch to next tab",
            KeyAction::PrevTab => "Switch to previous tab",
            KeyAction::CloseTab => "Remove tab from group",
            KeyAction::SelectTab => "Go to tab by number",
        };
        rows.push(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_1()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(theme.colors.fg)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.colors.fg_muted)
                                .child(desc),
                        ),
                )
                .child(HotkeyInput::new(("hotkey", index), value).on_change(on_change)),
        );
    }

    dialog = dialog.child(div().flex().flex_col().gap_1().children(rows));

    let ctrl_reset = controller.clone();
    dialog = dialog.action(move |_close| {
        let ctrl = ctrl_reset.clone();
        Button::new("kb-reset", "Reset defaults").on_click(move |_ev, _window, cx| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::ResetKeybinds, cx));
        })
    });

    let ctrl_cancel = controller.clone();
    dialog = dialog.action(move |close| {
        let ctrl = ctrl_cancel.clone();
        Button::new("kb-cancel", "Cancel").on_click(move |_ev, window, cx| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::CloseDialog, cx));
            close(window, cx);
        })
    });

    let ctrl_save = controller.clone();
    dialog = dialog.action(move |close| {
        let ctrl = ctrl_save.clone();
        Button::new("kb-save", "Save")
            .primary()
            .on_click(move |_ev, window, cx| {
                ctrl.update(cx, |c, cx| c.dispatch(Cmd::CommitKeybinds, cx));
                close(window, cx);
            })
    });

    dialog.into_any_element()
}

fn update_dialog(controller: &Entity<Controller>, cx: &App) -> gpui::AnyElement {
    let info = controller.read(cx).host_info();
    let theme = cx.theme();
    let (available, version) = match &info.update {
        UpdateState::Available(version) => (true, version.clone()),
        _ => (false, String::new()),
    };

    let update = {
        let ctrl = controller.clone();
        move |_ev: &gpui::ClickEvent, _w: &mut Window, cx: &mut App| {
            ctrl.update(cx, |c, cx| {
                c.dispatch(Cmd::InstallAvailableUpdate, cx);
                c.dispatch(Cmd::CloseDialog, cx);
            });
        }
    };
    let later = {
        let ctrl = controller.clone();
        move |_ev: &gpui::ClickEvent, _w: &mut Window, cx: &mut App| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::CloseDialog, cx));
        }
    };

    let (title, note) = if available {
        (
            format!("Slate update v{version}"),
            "A new version is ready. Download and verify it; Slate will restart to install.",
        )
    } else {
        (
            "Checking for updates".to_string(),
            "The update notice will appear when the check finishes.",
        )
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .p(px(28.0))
        .when(available, |card| {
            card.child(
                Icon::new(IconName::Download)
                    .size(IconSize::Lg)
                    .color(theme.colors.accent)
                    .into_any_element(),
            )
        })
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.colors.fg)
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.colors.fg_muted)
                .child(note),
        )
        .when(available, |card| {
            card.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mt_2()
                    .child(
                        Button::new("upd-later", "Later")
                            .variant(ely_gpui_component::buttons::ButtonVariant::Ghost)
                            .on_click(later),
                    )
                    .child(
                        Button::new("upd-now", "Update now")
                            .primary()
                            .on_click(update),
                    ),
            )
        })
        .into_any_element()
}
