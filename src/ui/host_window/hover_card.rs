use ely_gpui_component::primitives::Image;
use ely_gpui_component::theme::{ActiveTheme, Radius, TextSize};
use ely_gpui_component::typography::Ellipsis;
use gpui::prelude::*;
use gpui::{div, px, AnimationExt, App, Pixels, Size, Window};

use super::overlays::enter_animation;
use crate::controller::{HoverPreview, TabInfo};
use crate::ui::metrics::{CARD_GAP, CARD_H, CARD_IMAGE_H, CARD_MARGIN, CARD_W};

pub(crate) fn hover_card(
    cx: &App,
    preview: &HoverPreview,
    tab: Option<&TabInfo>,
    viewport: Size<Pixels>,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let rad_lg = theme.radius(Radius::Lg);
    let (x, y, _w, h) = preview.anchor;
    let (left, top) = card_origin(
        (x, y, h),
        (viewport.width.as_f32(), viewport.height.as_f32()),
    );

    let image = match preview.image.clone() {
        Some(img) => Image::new(("host-hover-shot", 0usize), img)
            .rounded_t(rad_lg)
            .w_full()
            .h(px(CARD_IMAGE_H))
            .into_any_element(),
        None => div()
            .w_full()
            .h(px(CARD_IMAGE_H))
            .rounded_t(rad_lg)
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.sunken)
            .when_some(tab.and_then(|t| t.icon.clone()), |area, icon| {
                area.child(
                    Image::new(("host-hover-icon", 0usize), icon)
                        .w(px(48.0))
                        .h(px(48.0)),
                )
            })
            .into_any_element(),
    };

    let title = tab
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "Window".to_string());
    let footer = div()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .border_t_1()
        .border_color(colors.border)
        .when_some(tab.and_then(|t| t.icon.clone()), |row, icon| {
            row.child(
                Image::new(("host-hover-tab-icon", 0usize), icon)
                    .w(px(16.0))
                    .h(px(16.0)),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(theme.text_size(TextSize::Sm))
                .text_color(colors.fg)
                .child(Ellipsis::new(title)),
        );

    let card = div()
        .w(px(CARD_W))
        .rounded(rad_lg)
        .bg(colors.overlay)
        .border_1()
        .border_color(colors.border)
        .overflow_hidden()
        .child(image)
        .child(footer);

    div()
        .id(("host-hover", preview.tab.0 as usize))
        .absolute()
        .left(px(left))
        .top(px(top))
        .on_hover(on_hover)
        .with_animation(
            ("host-hover-anim", preview.tab.0 as usize),
            enter_animation(cx),
            |el, t| el.opacity(t),
        )
        .child(card)
        .into_any_element()
}

/// Places the card under its tab, flipping above it and clamping inside the viewport.
/// `anchor` is the tab's x, y and height.
pub(super) fn card_origin(anchor: (f32, f32, f32), viewport: (f32, f32)) -> (f32, f32) {
    let (x, y, h) = anchor;
    let left = x.min(viewport.0 - CARD_W - CARD_MARGIN).max(CARD_MARGIN);
    let below = y + h + CARD_GAP;
    let top = if below + CARD_H <= viewport.1 - CARD_MARGIN {
        below
    } else {
        (y - CARD_GAP - CARD_H).max(CARD_MARGIN)
    };
    (left, top)
}

#[cfg(test)]
mod tests {
    use super::card_origin;
    use crate::ui::metrics::{CARD_GAP, CARD_H, CARD_MARGIN, CARD_W};

    #[test]
    fn a_card_under_a_tab_sits_below_it() {
        let (left, top) = card_origin((200.0, 20.0, 28.0), (1000.0, 800.0));
        assert_eq!(left, 200.0);
        assert_eq!(top, 20.0 + 28.0 + CARD_GAP);
    }

    #[test]
    fn a_card_past_the_right_edge_pulls_back() {
        let (left, _) = card_origin((990.0, 20.0, 28.0), (1000.0, 800.0));
        assert_eq!(left, 1000.0 - CARD_W - CARD_MARGIN);
    }

    #[test]
    fn a_card_off_the_left_edge_stays_a_margin_in() {
        let (left, _) = card_origin((-40.0, 20.0, 28.0), (1000.0, 800.0));
        assert_eq!(left, CARD_MARGIN);
    }

    #[test]
    fn a_card_with_no_room_below_flips_above_the_tab() {
        let (_, top) = card_origin((200.0, 760.0, 28.0), (1000.0, 800.0));
        assert_eq!(top, 760.0 - CARD_GAP - CARD_H);
        assert!(top >= CARD_MARGIN);
    }

    #[test]
    fn a_card_that_fits_neither_way_stays_a_margin_from_the_top() {
        let (_, top) = card_origin((200.0, 40.0, 28.0), (1000.0, 200.0));
        assert_eq!(top, CARD_MARGIN);
    }
}
