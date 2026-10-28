use std::rc::Rc;

use ely_gpui_component::motion;
use ely_gpui_component::theme::{ActiveTheme, Elevation, Radius};
use gpui::{
    canvas, div, px, Animation, AnimationExt, App, Bounds, Div, Entity, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, Role, Size, StatefulInteractiveElement,
    Styled, Window,
};

pub(super) type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub(super) use crate::ui::metrics::{MENU_W, MENU_W_SAVED, MENU_W_TAB_MENU};
use crate::ui::metrics::{OVERLAY_MARGIN, PANEL_PAD, ROW_H};

pub(super) fn menu_panel(width: f32, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .p_1()
        .w(px(width))
        .rounded(theme.radius(Radius::Lg))
        .bg(theme.colors.overlay)
        .border_1()
        .border_color(theme.colors.border)
        .shadow(theme.elevation(Elevation::Floating))
}

pub(super) fn menu_height(rows: usize) -> f32 {
    rows as f32 * ROW_H + PANEL_PAD
}

pub(super) fn overlay_layer(
    tag: &'static str,
    at: Point<Pixels>,
    width: f32,
    height: f32,
    viewport: Size<Pixels>,
    enter: Animation,
    panel: Div,
) -> gpui::AnyElement {
    let vw = viewport.width.as_f32();
    let vh = viewport.height.as_f32();
    let avail = (vh - OVERLAY_MARGIN * 2.0).max(0.0);
    let panel_h = height.min(avail);
    let left = at.x.as_f32().min(vw - width - OVERLAY_MARGIN).max(0.0);
    let top =
        at.y.as_f32()
            .min((vh - panel_h - OVERLAY_MARGIN).max(0.0))
            .max(0.0);
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .on_mouse_down(MouseButton::Left, |_ev, _w, cx| cx.stop_propagation())
        .child(
            div()
                .id(tag)
                .role(Role::Menu)
                .max_h(px(avail))
                .overflow_y_scroll()
                .with_animation((tag, 0usize), enter, |panel, t| {
                    panel.opacity(t).mt(motion::NUDGE * (t - 1.0))
                })
                .child(panel),
        )
        .into_any_element()
}

pub(super) fn enter_animation(cx: &App) -> Animation {
    Animation::new(motion::duration(motion::FAST, cx)).with_easing(motion::ease_out_cubic)
}

/// Tracks the trigger's bounds so its overlay can open against it; a move asks for one repaint.
pub(super) fn anchor_probe(anchor: Entity<Bounds<Pixels>>) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            let changed = anchor.update(cx, |tracked, _| {
                std::mem::replace(tracked, bounds) != bounds
            });
            if changed {
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
