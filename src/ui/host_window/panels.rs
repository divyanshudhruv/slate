use ely_gpui_component::buttons::IconButton;
use ely_gpui_component::primitives::{Icon, IconName, Image};
use ely_gpui_component::theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize, HUE_NAMES};
use ely_gpui_component::typography::{Ellipsis, Kbd};
use gpui::prelude::*;
use gpui::{
    div, px, AnyElement, App, Bounds, Div, ElementId, Entity, FontWeight, Hsla, MouseButton, Point,
    Rems, Role, Styled, Window,
};

use super::overlays::{
    anchor_probe, enter_animation, menu_height, menu_panel, overlay_layer, MENU_W, MENU_W_SAVED,
    MENU_W_TAB_MENU,
};
use super::HostCtx;
use crate::controller::{Cmd, ContextMenuItem, Controller, PickerRow, SavedSetInfo, TabInfo};
use crate::core::{GroupId, TabId};

use crate::ui::metrics::{
    BADGE_FONT, BADGE_OFFSET, BADGE_PAD_X, BADGE_SIZE, DIVIDER_H, SWATCH_H, SWATCH_H_ACTIVE,
};

pub(super) fn tab_overflow(ctx: &mut HostCtx) -> (AnyElement, Option<AnyElement>) {
    let group = ctx.group;
    let viewport = ctx.viewport;
    let anchor =
        ctx.window
            .use_keyed_state(("host-tab-overflow-anchor", 0usize), ctx.cx, |_, _| {
                Bounds::default()
            });
    let info = ctx.info;
    let controller = ctx.controller.clone();
    let open = info.tab_menu_open;
    let show_badge = info.show_tab_badge;
    let icons_in_tabs = info.icons_in_tabs;

    let theme = ctx.cx.theme();
    let colors = &theme.colors;
    let text_sm = theme.text_size(TextSize::Sm);

    let chevron = IconButton::new(
        "host-tab-overflow",
        if open {
            IconName::Minus
        } else {
            IconName::ChevronDown
        },
    )
    .tooltip("All tabs")
    .on_click({
        let ctrl = controller.clone();
        move |_, _w, cx| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::ToggleTabMenu, cx));
        }
    });

    let rows = info.tabs.iter().enumerate().map(|(ix, tab)| {
        let marks = RowMarks {
            active: info.active.as_ref() == Some(&tab.id),
            highlighted: info.tab_menu_cursor == Some(ix),
        };
        overflow_row(ix, tab, marks, icons_in_tabs, group, &controller, ctx.cx)
    });

    let empty_row = info.tabs.is_empty().then(|| {
        div()
            .px_2()
            .py_1()
            .text_size(text_sm)
            .text_color(colors.fg_subtle)
            .child("No windows yet")
    });

    let panel = menu_panel(MENU_W, ctx.cx)
        .children(rows)
        .children(empty_row);

    let trigger = if show_badge && !info.tabs.is_empty() {
        div()
            .relative()
            .flex_none()
            .child(chevron)
            .child(
                div()
                    .absolute()
                    .top(px(BADGE_OFFSET))
                    .right(px(BADGE_OFFSET))
                    .child(tab_badge(info.tabs.len(), ctx.cx)),
            )
            .into_any_element()
    } else {
        chevron.into_any_element()
    };

    let wrap = div()
        .id("tab-overflow-wrap")
        .relative()
        .flex_none()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_ev, _w, cx| cx.stop_propagation())
        .child(trigger)
        .child(anchor_probe(anchor.clone()))
        .into_any_element();

    let overlay = open.then(|| {
        let at = anchor.read(ctx.cx).bottom_left();
        overlay_layer(
            "host-tab-overflow",
            at,
            MENU_W,
            menu_height(info.tabs.len().max(1)),
            viewport,
            enter_animation(ctx.cx),
            panel,
        )
    });
    (wrap, overlay)
}

pub(super) fn picker(ctx: &mut HostCtx) -> (AnyElement, Option<AnyElement>) {
    let viewport = ctx.viewport;
    let anchor = ctx
        .window
        .use_keyed_state(("host-picker-anchor", 0usize), ctx.cx, |_, _| {
            Bounds::default()
        });
    let info = ctx.info;
    let controller = ctx.controller.clone();

    let wrap = div()
        .id("picker-wrap")
        .relative()
        .flex_none()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_ev, _w, cx| cx.stop_propagation())
        .child(
            IconButton::new("host-add-window", IconName::Plus)
                .tooltip("Add a window")
                .on_click({
                    let ctrl = controller.clone();
                    move |_, _w, cx| {
                        ctrl.update(cx, |c, cx| c.dispatch(Cmd::TogglePicker, cx));
                    }
                }),
        )
        .child(anchor_probe(anchor.clone()));

    let overlay = info.picker_open.then(|| {
        let mut rows: Vec<AnyElement> = Vec::with_capacity(info.picker_rows.len() + 1);
        rows.push(picker_action_row(
            info.picker_picks.len(),
            &controller,
            ctx.cx,
        ));
        for (ix, row) in info.picker_rows.iter().enumerate() {
            let picked = info.picker_picks.contains(&row.hwnd);
            let highlighted = info.picker_cursor == Some(ix);
            rows.push(picker_row(
                ix,
                row,
                picked,
                highlighted,
                &controller,
                ctx.cx,
            ));
        }

        let panel = menu_panel(MENU_W, ctx.cx).children(rows);

        let at = anchor.read(ctx.cx).bottom_left();
        overlay_layer(
            "host-picker",
            at,
            MENU_W,
            menu_height(info.picker_rows.len() + 1),
            viewport,
            enter_animation(ctx.cx),
            panel,
        )
    });
    (wrap.into_any_element(), overlay)
}

pub(super) fn saved(ctx: &mut HostCtx) -> (AnyElement, Option<AnyElement>) {
    let viewport = ctx.viewport;
    let anchor = ctx
        .window
        .use_keyed_state(("host-saved-anchor", 0usize), ctx.cx, |_, _| {
            Bounds::default()
        });
    let info = ctx.info;
    let controller = ctx.controller.clone();

    let wrap = div()
        .id("saved-wrap")
        .relative()
        .flex_none()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_ev, _w, cx| cx.stop_propagation())
        .child(
            IconButton::new("host-saved-sets", IconName::Layers)
                .tooltip("Saved tab sets")
                .on_click({
                    let ctrl = controller.clone();
                    move |_, _w, cx| {
                        ctrl.update(cx, |c, cx| c.dispatch(Cmd::ToggleSavedMenu, cx));
                    }
                }),
        )
        .child(anchor_probe(anchor.clone()));

    let overlay = info.saved_menu_open.then(|| {
        let save_enabled = !info.tabs.is_empty();
        let mut rows: Vec<AnyElement> = Vec::with_capacity(info.saved_sets.len() + 1);

        rows.push(saved_save_row(
            save_enabled,
            info.saved_menu_cursor == Some(0),
            &controller,
            ctx.cx,
        ));
        for (ix, set) in info.saved_sets.iter().enumerate() {
            rows.push(saved_set_row(
                set,
                info.saved_menu_cursor == Some(ix + 1),
                &controller,
                ctx.cx,
            ));
        }

        if info.saved_sets.is_empty() {
            let theme = ctx.cx.theme();
            rows.push(
                div()
                    .px_2()
                    .py_1()
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(theme.colors.fg_subtle)
                    .child("No saved sets yet")
                    .into_any_element(),
            );
        }

        let height = menu_height(rows.len());
        let panel = menu_panel(MENU_W_SAVED, ctx.cx).children(rows);

        let at = anchor.read(ctx.cx).bottom_left();
        overlay_layer(
            "host-saved",
            at,
            MENU_W_SAVED,
            height,
            viewport,
            enter_animation(ctx.cx),
            panel,
        )
    });
    (wrap.into_any_element(), overlay)
}

pub(super) fn tab_menu(ctx: &mut HostCtx) -> Option<(Point<gpui::Pixels>, gpui::Div, f32)> {
    let info = ctx.info;
    let controller = ctx.controller.clone();
    let (group, tab, at) = {
        let menu = info.tab_menu.as_ref()?;
        let group = ctx.group?;
        (group, menu.tab, menu.at)
    };

    let accent_now = info
        .tabs
        .iter()
        .find(|t| t.id == tab)
        .and_then(|t| t.accent);

    let mut rows: Vec<AnyElement> = Vec::with_capacity(info.ctx_menu_items.len() + 2);
    rows.push(accent_row(
        accent_now,
        group,
        tab,
        &controller,
        &ctx.palette,
        ctx.cx,
    ));
    for (ix, item) in info.ctx_menu_items.iter().enumerate() {
        if item.starts_group {
            rows.push(menu_divider(ix, ctx.cx));
        }
        rows.push(ctx_menu_row(
            item,
            info.ctx_menu_cursor == Some(ix),
            &controller,
            ctx.cx,
        ));
    }

    let height = menu_height(rows.len());
    let panel = menu_panel(MENU_W_TAB_MENU, ctx.cx).children(rows);

    Some((gpui::point(px(at.0), px(at.1)), panel, height))
}

fn tab_badge(count: usize, cx: &App) -> Div {
    let theme = cx.theme();
    let colors = &theme.colors;
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(BADGE_SIZE))
        .h(px(BADGE_SIZE))
        .px(px(BADGE_PAD_X))
        .rounded_full()
        .bg(colors.accent)
        .text_color(colors.on_accent)
        .text_size(px(BADGE_FONT))
        .line_height(px(BADGE_FONT))
        .font_weight(FontWeight::MEDIUM)
        .child(badge_label(count))
}

/// The badge text, capped so a wide count cannot stretch the tab.
fn badge_label(count: usize) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

/// Which row of the all-tabs menu is current, and whether it sits under the keyboard cursor.
struct RowMarks {
    active: bool,
    highlighted: bool,
}

/// The frame every menu row shares: padding, radius and type size, so rows line up.
fn row_frame(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .px_2()
        .py_1()
        .rounded(theme.radius(Radius::Sm))
        .text_size(theme.text_size(TextSize::Sm))
}

/// One menu row: a leading glyph, a title over an optional muted line, and a trailing slot.
/// Callers add their own hover, cursor and click behaviour to the built row.
struct RowShell {
    id: ElementId,
    icon: AnyElement,
    title: String,
    title_color: Hsla,
    detail: Option<(String, Hsla)>,
    trailing: Option<AnyElement>,
    min_height: Option<Rems>,
}

impl RowShell {
    fn new(
        id: impl Into<ElementId>,
        icon: AnyElement,
        title: impl Into<String>,
        title_color: Hsla,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            title: title.into(),
            title_color,
            detail: None,
            trailing: None,
            min_height: None,
        }
    }

    fn detail(mut self, text: impl Into<String>, color: Hsla) -> Self {
        self.detail = Some((text.into(), color));
        self
    }

    fn trailing(mut self, element: AnyElement) -> Self {
        self.trailing = Some(element);
        self
    }

    fn min_height(mut self, height: Rems) -> Self {
        self.min_height = Some(height);
        self
    }

    fn build(self, cx: &App) -> gpui::Stateful<Div> {
        let text_xs = cx.theme().text_size(TextSize::Xs);
        row_frame(cx)
            .id(self.id)
            .when_some(self.min_height, |el, height| el.min_h(height))
            .child(self.icon)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .text_color(self.title_color)
                    .child(Ellipsis::new(self.title))
                    .when_some(self.detail, |el, (text, color)| {
                        el.child(
                            div()
                                .text_size(text_xs)
                                .text_color(color)
                                .child(Ellipsis::new(text)),
                        )
                    }),
            )
            .when_some(self.trailing, |el, trailing| el.child(trailing))
    }
}

fn overflow_row(
    ix: usize,
    tab: &TabInfo,
    marks: RowMarks,
    icons_in_tabs: bool,
    group: Option<GroupId>,
    controller: &Entity<Controller>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let icon_px = theme.icon_size(IconSize::Sm).to_pixels(theme.base_rem());
    let RowMarks {
        active,
        highlighted,
    } = marks;
    let (app, exe, icon) = (&tab.app, &tab.exe, &tab.icon);
    let tab_id = tab.id;
    let lead = match (icons_in_tabs, icon) {
        (true, Some(image)) => Image::new(("tab-overflow-icon", ix), image.clone())
            .flex_none()
            .w(icon_px)
            .h(icon_px)
            .into_any_element(),
        _ => Icon::new(IconName::AppWindow)
            .size(IconSize::Sm)
            .color(colors.fg_subtle)
            .into_any_element(),
    };
    let title_color = if active { colors.fg } else { colors.fg_muted };
    let ctrl = controller.clone();
    RowShell::new(("tab-overflow-row", ix), lead, app.clone(), title_color)
        .detail(exe.clone(), colors.fg_subtle)
        .build(cx)
        .role(Role::MenuItem)
        .aria_label(app.clone())
        .cursor_pointer()
        .when(highlighted, |el| el.bg(colors.hover))
        .hover(|style| style.bg(colors.hover))
        .on_click(move |_, _w, cx| {
            ctrl.update(cx, |c, cx| {
                if let Some(g) = group {
                    c.dispatch(
                        Cmd::SelectTab {
                            group: g,
                            tab: tab_id,
                        },
                        cx,
                    );
                }
                c.dispatch(Cmd::SetTabMenu(false), cx);
            });
        })
        .into_any_element()
}

fn picker_action_row(count: usize, controller: &Entity<Controller>, cx: &App) -> AnyElement {
    let colors = &cx.theme().colors;
    let picking = count == 0;
    let (icon, color) = if picking {
        (IconName::Plus, colors.fg)
    } else {
        (IconName::Check, colors.accent)
    };
    let label = if picking {
        "Pick a window with"
    } else {
        "Done"
    };
    let aria_label = if picking {
        "Pick a window with Tab".to_string()
    } else {
        format!("Add {count} selected windows")
    };
    let picked_label = count.to_string();
    let ctrl = controller.clone();
    RowShell::new(
        "picker-action-row",
        Icon::new(icon)
            .size(IconSize::Sm)
            .color(color)
            .into_any_element(),
        label,
        color,
    )
    .trailing(Kbd::new(if picking { "tab" } else { &picked_label }).into_any_element())
    .build(cx)
    .role(Role::MenuItem)
    .aria_label(aria_label)
    .cursor_pointer()
    .hover(|style| style.bg(colors.hover))
    .on_click(move |_, _w, cx| {
        ctrl.update(cx, |c, cx| {
            if picking {
                c.dispatch(Cmd::BeginPick, cx);
            } else {
                c.dispatch(Cmd::AddPicked, cx);
            }
            c.dispatch(Cmd::SetPickerOpen(false), cx);
        });
    })
    .into_any_element()
}

fn picker_row(
    ix: usize,
    row: &PickerRow,
    picked: bool,
    highlighted: bool,
    controller: &Entity<Controller>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let icon_px = theme.icon_size(IconSize::Sm).to_pixels(theme.base_rem());
    let blocked = row.reason.is_some();
    let title_color = if blocked { colors.fg_subtle } else { colors.fg };
    let lead = match &row.icon {
        Some(image) => Image::new(("picker-icon", ix), image.clone())
            .flex_none()
            .w(icon_px)
            .h(icon_px)
            .into_any_element(),
        None => Icon::new(IconName::AppWindow)
            .size(IconSize::Sm)
            .color(colors.fg_subtle)
            .into_any_element(),
    };
    let detail = row.reason.clone().unwrap_or_else(|| row.exe.clone());
    let mut shell = RowShell::new(("picker-row", ix), lead, row.title.clone(), title_color)
        .detail(detail, colors.fg_subtle);
    if picked {
        shell = shell.trailing(
            Icon::new(IconName::Check)
                .size(IconSize::Sm)
                .color(colors.accent)
                .into_any_element(),
        );
    }
    let mut el = shell
        .build(cx)
        .role(Role::MenuItem)
        .aria_label(row.title.clone());
    if blocked {
        el = el.opacity(0.5);
    } else {
        el = el.cursor_pointer().hover(|style| style.bg(colors.hover));
        if picked || highlighted {
            el = el.bg(colors.hover);
        }
    }
    el.on_click({
        let ctrl = controller.clone();
        let hwnd = row.hwnd;
        move |_, _w, cx| {
            if blocked {
                return;
            }
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::TogglePickerPick { hwnd }, cx));
        }
    })
    .into_any_element()
}

fn saved_save_row(
    save_enabled: bool,
    highlighted: bool,
    controller: &Entity<Controller>,
    cx: &App,
) -> AnyElement {
    let colors = &cx.theme().colors;
    let ctrl = controller.clone();
    RowShell::new(
        "saved-save-row",
        Icon::new(IconName::Save)
            .size(IconSize::Sm)
            .color(colors.fg_subtle)
            .into_any_element(),
        "Save current tabs\u{2026}",
        if save_enabled {
            colors.fg
        } else {
            colors.fg_subtle
        },
    )
    .build(cx)
    .role(Role::MenuItem)
    .aria_label("Save current tabs")
    .when(save_enabled, |el| {
        el.cursor_pointer()
            .hover(|style| style.bg(colors.hover))
            .on_click(move |_, _w, cx| {
                ctrl.update(cx, |c, cx| c.dispatch(Cmd::BeginSaveSet, cx));
            })
    })
    .when(save_enabled && highlighted, |el| el.bg(colors.hover))
    .when(!save_enabled, |el| el.opacity(0.5))
    .into_any_element()
}

fn saved_set_row(
    set: &SavedSetInfo,
    highlighted: bool,
    controller: &Entity<Controller>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let rad_sm = theme.radius(Radius::Sm);
    let id = set.id;
    let restore = {
        let ctrl = controller.clone();
        move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            ctrl.update(cx, |c, cx| {
                c.dispatch(Cmd::RestoreSet { id }, cx);
                c.dispatch(Cmd::SetSavedMenu(false), cx);
            });
        }
    };
    let rename = {
        let ctrl = controller.clone();
        move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::BeginRenameSet { id }, cx));
        }
    };
    let delete = {
        let ctrl = controller.clone();
        move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            ctrl.update(cx, |c, cx| c.dispatch(Cmd::DeleteSet { id }, cx));
        }
    };
    let detail_color = if set.available == 0 {
        colors.fg_subtle
    } else {
        colors.fg_muted
    };
    let title_color = if set.available == 0 {
        colors.fg_muted
    } else {
        colors.fg
    };
    let restore_row = RowShell::new(
        ("saved-restore", id),
        Icon::new(IconName::FolderOpen)
            .size(IconSize::Sm)
            .color(colors.fg_subtle)
            .into_any_element(),
        set.name.clone(),
        title_color,
    )
    .detail(
        format!("{} windows - {} available", set.total, set.available),
        detail_color,
    )
    .build(cx)
    .role(Role::MenuItem)
    .aria_label(set.name.clone())
    .when(highlighted, |el| el.bg(colors.hover))
    .cursor_pointer()
    .on_click(restore);
    div()
        .id(("saved-row", id))
        .flex()
        .items_center()
        .gap_1()
        .w_full()
        .rounded(rad_sm)
        .hover(|style| style.bg(colors.hover))
        .child(restore_row)
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_0p5()
                .px_1()
                .occlude()
                .child(
                    IconButton::new(("saved-rename", id), IconName::Pencil)
                        .size(ControlSize::Sm)
                        .tooltip("Rename set")
                        .on_click(rename),
                )
                .child(
                    IconButton::new(("saved-delete", id), IconName::Trash2)
                        .size(ControlSize::Sm)
                        .tooltip("Delete set")
                        .on_click(delete),
                ),
        )
        .into_any_element()
}

fn menu_divider(index: usize, cx: &App) -> AnyElement {
    div()
        .id(("ctx-divider", index))
        .my_1()
        .h(px(DIVIDER_H))
        .w_full()
        .bg(cx.theme().colors.border)
        .into_any_element()
}

/// One row of the tab context menu, built from the controller's list so the key cursor and the painted rows stay in step.
fn ctx_menu_row(
    item: &ContextMenuItem,
    highlighted: bool,
    controller: &Entity<Controller>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let row_h = theme.control_height(ControlSize::Md);
    let (title_color, icon_color) = if item.danger {
        (colors.danger, colors.danger)
    } else {
        (colors.fg_muted, colors.fg_subtle)
    };
    let mut shell = RowShell::new(
        item.id,
        Icon::new(item.icon)
            .size(IconSize::Sm)
            .color(icon_color)
            .into_any_element(),
        item.label,
        title_color,
    )
    .min_height(row_h);
    if let Some(desc) = item.desc {
        shell = shell.detail(desc, colors.fg_subtle);
    }
    let enabled = item.enabled;
    let cmd = item.cmd.clone();
    let ctrl = controller.clone();
    shell
        .build(cx)
        .role(Role::MenuItem)
        .aria_label(item.label)
        .when(item.desc.is_some(), |el| el.py_1())
        .when(!enabled, |el| el.opacity(0.4))
        .when(enabled, |el| {
            el.cursor_pointer().hover(|style| style.bg(colors.hover))
        })
        .when(enabled && highlighted, |el| el.bg(colors.hover))
        .on_click(move |_, _w, cx| {
            if !enabled {
                return;
            }
            ctrl.update(cx, |c, cx| {
                c.dispatch(cmd.clone(), cx);
                c.dispatch(Cmd::CloseTabMenu, cx);
            });
        })
        .into_any_element()
}

fn accent_row(
    accent_now: Option<usize>,
    group: GroupId,
    tab: TabId,
    controller: &Entity<Controller>,
    palette: &ely_gpui_component::theme::Palette,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let set_color = |color: Option<usize>| {
        let ctrl = controller.clone();
        move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            ctrl.update(cx, |c, cx| {
                c.dispatch(Cmd::SetTabColor { group, tab, color }, cx);
                c.dispatch(Cmd::CloseTabMenu, cx);
            });
        }
    };
    let selected = accent_now.is_none();
    let click = set_color(None);
    let none_seg = div()
        .id("ctx-swatch-none")
        .role(Role::Button)
        .aria_label("No accent color")
        .flex_1()
        .h(px(SWATCH_H))
        .rounded_full()
        .border_1()
        .border_color(colors.border_strong)
        .cursor_pointer()
        .when(selected, |el| el.h(px(SWATCH_H_ACTIVE)).bg(colors.hover))
        .hover(|style| style.opacity(0.8))
        .on_click(click);
    let hues: Vec<_> = HUE_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| palette.hue(index, name))
        .collect();
    let swatches = HUE_NAMES.iter().enumerate().map(|(index, name)| {
        let selected = accent_now == Some(index);
        let color = hues[index];
        let click = set_color(Some(index));
        div()
            .id(("ctx-swatch", index))
            .role(Role::Button)
            .aria_label(*name)
            .flex_1()
            .h(px(SWATCH_H))
            .rounded_full()
            .bg(color)
            .cursor_pointer()
            .when(selected, |el| el.h(px(SWATCH_H_ACTIVE)))
            .hover(|style| style.opacity(0.8))
            .on_click(click)
    });
    div()
        .id("ctx-color")
        .flex()
        .items_center()
        .gap_1()
        .w_full()
        .px_1()
        .py_2()
        .child(none_seg)
        .children(swatches)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_badge_past_ninety_nine_is_capped() {
        assert_eq!(super::badge_label(0), "0");
        assert_eq!(super::badge_label(1), "1");
        assert_eq!(super::badge_label(99), "99");
        assert_eq!(super::badge_label(100), "99+");
        assert_eq!(super::badge_label(usize::MAX), "99+");
    }
}
