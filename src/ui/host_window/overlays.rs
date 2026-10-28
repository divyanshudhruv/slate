use ely_gpui_component::motion;
use ely_gpui_component::theme::{ActiveTheme, Elevation, Radius};
use gpui::{
    canvas, div, px, Animation, AnimationExt, App, Bounds, Div, Entity, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, Role, Size, StatefulInteractiveElement,
    Styled,
};

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

/// Places an overlay at its anchor, clamped inside the viewport with a margin all round.
pub(super) fn overlay_origin(
    at: (f32, f32),
    width: f32,
    height: f32,
    viewport: (f32, f32),
) -> (f32, f32) {
    let (vw, vh) = viewport;
    let avail = (vh - OVERLAY_MARGIN * 2.0).max(0.0);
    let panel_h = height.min(avail);
    let left = at.0.min(vw - width - OVERLAY_MARGIN).max(0.0);
    let top = at.1.min((vh - panel_h - OVERLAY_MARGIN).max(0.0)).max(0.0);
    (left, top)
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
    let (left, top) = overlay_origin((at.x.as_f32(), at.y.as_f32()), width, height, (vw, vh));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_height_grows_a_row_at_a_time() {
        assert_eq!(menu_height(0), PANEL_PAD);
        assert_eq!(menu_height(1), ROW_H + PANEL_PAD);
        assert_eq!(menu_height(9), 9.0 * ROW_H + PANEL_PAD);
    }

    #[test]
    fn overlay_origin_keeps_an_inside_anchor() {
        let viewport = (1000.0, 800.0);
        assert_eq!(
            overlay_origin((100.0, 100.0), 200.0, 150.0, viewport),
            (100.0, 100.0)
        );
    }

    #[test]
    fn overlay_origin_clamps_past_the_right_edge() {
        let viewport = (1000.0, 800.0);
        let (left, top) = overlay_origin((990.0, 40.0), 220.0, 120.0, viewport);
        assert_eq!(left, 1000.0 - 220.0 - OVERLAY_MARGIN);
        assert_eq!(top, 40.0);
    }

    #[test]
    fn overlay_origin_clamps_past_the_bottom_edge() {
        let viewport = (1000.0, 800.0);
        let (left, top) = overlay_origin((40.0, 795.0), 220.0, 300.0, viewport);
        assert_eq!(left, 40.0);
        assert_eq!(top, 800.0 - 300.0 - OVERLAY_MARGIN);
    }

    #[test]
    fn overlay_origin_never_leaves_the_top_left() {
        let viewport = (1000.0, 800.0);
        assert_eq!(
            overlay_origin((-50.0, -80.0), 220.0, 120.0, viewport),
            (0.0, 0.0)
        );
    }

    #[test]
    fn overlay_origin_stops_at_the_margin_for_a_taller_than_viewport_panel() {
        let viewport = (1000.0, 800.0);
        let (_, top) = overlay_origin((10.0, 700.0), 220.0, 5000.0, viewport);
        assert_eq!(top, OVERLAY_MARGIN);
    }
}
