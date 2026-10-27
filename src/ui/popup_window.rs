use gpui::prelude::*;
use gpui::{
    div, point, px, size, App, Bounds, Context, Entity, ParentElement, Render, Styled,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use super::host_window::hover_card::{hover_card, CARD_H, CARD_W};
use super::window_hwnd;
use crate::controller::{Cmd, Controller, HoverPreview};
use crate::win::popup;

const GAP: f32 = 6.0;
const MARGIN: f32 = 8.0;

/// Topmost, non-activating window that draws the hover preview above members.
pub struct PopupWindow {
    controller: Entity<Controller>,
    passive: bool,
    attached: bool,
    parked: bool,
    _observe: Subscription,
}

impl PopupWindow {
    pub fn open(controller: Entity<Controller>, cx: &mut App) -> Option<gpui::WindowHandle<Self>> {
        let options = WindowOptions {
            // gpui swaps off-screen bounds for the whole monitor, so start on
            // screen at the card size and park right after.
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(CARD_W), px(CARD_H)),
            ))),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            // Opaque windows clear to white, which shows through the corners.
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        cx.open_window(options, move |_window, cx| {
            cx.new(|cx| Self::new(controller, cx))
        })
        .ok()
    }

    fn new(controller: Entity<Controller>, cx: &mut Context<Self>) -> Self {
        let _observe = cx.observe(&controller, |_, _, cx| cx.notify());
        Self {
            controller,
            passive: false,
            attached: false,
            parked: false,
            _observe,
        }
    }
}

impl Render for PopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(hwnd) = window_hwnd(window) else {
            return div().into_any_element();
        };
        if !self.passive {
            popup::passive(hwnd);
            self.passive = true;
        }
        if !self.attached {
            if let Some(host) = self.controller.read(cx).host_hwnd() {
                popup::attach(hwnd, host);
                self.attached = true;
            }
        }

        let Some(preview) = self.controller.read_with(cx, |c, _| c.hover_preview()) else {
            if !self.parked {
                popup::park(hwnd);
                self.parked = true;
            }
            return div().into_any_element();
        };

        let placed = self
            .controller
            .read(cx)
            .host_hwnd()
            .map(|host| place_at(hwnd, host, &preview))
            .unwrap_or(false);
        if placed {
            self.parked = false;
        } else if !self.parked {
            popup::park(hwnd);
            self.parked = true;
        }

        let info = self.controller.read_with(cx, |c, _| c.host_info());
        let tab = info.tabs.iter().find(|t| t.id == preview.tab);
        let on_hover = {
            let ctrl = self.controller.clone();
            move |hovered: &bool, _w: &mut Window, cx: &mut App| {
                let hovered = *hovered;
                ctrl.update(cx, |c, cx| c.dispatch(Cmd::PreviewCardHover(hovered), cx));
            }
        };
        let card = hover_card(cx, &preview, tab, on_hover);

        div()
            .id("hover-popup")
            .w(px(CARD_W))
            .h(px(CARD_H))
            .child(card)
            .into_any_element()
    }
}

/// Position the popup under the hovered tab, flipping above it when it would
/// run off the bottom of the host window.
fn place_at(hwnd: isize, host: isize, preview: &HoverPreview) -> bool {
    let Some((origin_x, origin_y, client_w, client_h, scale)) = popup::host_metrics(host) else {
        return false;
    };
    let vw = client_w as f32 / scale;
    let vh = client_h as f32 / scale;
    let (x, y, _w, h) = preview.anchor;
    let left = x.min(vw - CARD_W - MARGIN).max(MARGIN);
    let below = y + h + GAP;
    let top = if below + CARD_H <= vh - MARGIN {
        below
    } else {
        (y - GAP - CARD_H).max(MARGIN)
    };
    popup::place(
        hwnd,
        origin_x + (left * scale).round() as i32,
        origin_y + (top * scale).round() as i32,
        (CARD_W * scale).round() as i32,
        (CARD_H * scale).round() as i32,
    );
    true
}
