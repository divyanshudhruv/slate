use std::time::Duration;

use ely_gpui_component::primitives::Severity;
use gpui::{Context, SharedString};

use super::dto::{banner_message, BannerAction, BannerKind, BannerView};
use super::Controller;
use crate::core::UpdateState;

impl Controller {
    /// Puts a banner under the title bar. A previous one is replaced, never stacked.
    pub(super) fn show_banner(
        &mut self,
        kind: BannerKind,
        ttl: Option<Duration>,
        cx: &mut Context<Self>,
    ) {
        self.banner_id = self.banner_id.wrapping_add(1);
        let id = self.banner_id;
        self.banner = Some(BannerView { id, kind });
        self.invalidate_info();
        cx.notify();
        let Some(ttl) = ttl else {
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ttl).await;
            this.update(cx, |controller, cx| {
                // Only clear the banner this timer was started for.
                if controller
                    .banner
                    .as_ref()
                    .is_some_and(|banner| banner.id == id)
                {
                    controller.banner = None;
                    controller.invalidate_info();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn note_success(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let kind = banner_message(Severity::Success, text, None);
        self.show_banner(kind, Some(Duration::from_millis(3_000)), cx);
    }

    pub(super) fn note_error(
        &mut self,
        text: impl Into<SharedString>,
        action: Option<(SharedString, BannerAction)>,
        cx: &mut Context<Self>,
    ) {
        let kind = banner_message(Severity::Danger, text, action);
        self.show_banner(kind, Some(Duration::from_millis(7_000)), cx);
    }

    /// The close button on a message banner.
    pub(super) fn dismiss_banner(&mut self, cx: &mut Context<Self>) {
        let is_message = matches!(
            self.banner.as_ref().map(|banner| &banner.kind),
            Some(BannerKind::Message { .. })
        );
        if is_message {
            self.banner = None;
        }
        self.invalidate_info();
        cx.notify();
    }

    /// Later on an update banner: hidden until a newer version shows up.
    pub(super) fn dismiss_update_banner(&mut self, cx: &mut Context<Self>) {
        if let UpdateState::Available(version) = &self.update {
            self.update_banner_hidden = Some(version.clone());
        }
        self.invalidate_info();
        cx.notify();
    }
}
