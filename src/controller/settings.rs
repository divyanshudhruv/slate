use ely_gpui_component::feedback::Toast;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::{ActiveTheme, Mode, Palette, Theme, HUE_NAMES};
use gpui::{Context, Hsla};

use super::dto::render_image;
use super::Controller;
use crate::config;
use crate::win::window_icon;

impl Controller {
    pub(super) fn toggle_icons_in_tabs(&mut self) {
        self.settings.tabs.icons = !self.settings.tabs.icons;
        if self.settings.tabs.icons {
            for hwnd in self.styling.keys().copied().collect::<Vec<_>>() {
                self.ensure_icon(hwnd);
            }
        }
        self.save_settings();
    }

    pub(super) fn toggle_tab_badge(&mut self) {
        self.settings.tabs.count_badge = !self.settings.tabs.count_badge;
        self.save_settings();
    }

    pub(super) fn save_settings(&self) {
        let _ = config::save(&self.settings);
    }

    /// Re-reads config.json and settings.json so a hand edit lands without a restart.
    pub(super) fn reload_settings(&mut self, cx: &mut Context<Self>) {
        let fresh = config::load();
        let icons_turned_on = !self.settings.tabs.icons && fresh.tabs.icons;
        self.settings = fresh;
        if icons_turned_on {
            for hwnd in self.styling.keys().copied().collect::<Vec<_>>() {
                self.ensure_icon(hwnd);
            }
        }
        super::dialog::push_shortcuts(&self.settings);
        self.apply_theme(cx);
        let toast = Toast::new("Settings reloaded").severity(Severity::Info);
        self.toaster
            .update(cx, |toaster, cx| toaster.push(toast, cx));
    }

    pub(super) fn apply_theme(&self, cx: &mut Context<Self>) {
        let system = Mode::from(cx.window_appearance());
        Theme::set_mode_now(self.resolve_mode(system), cx);
        self.apply_accent(cx);
    }

    fn resolve_mode(&self, system: Mode) -> Mode {
        match self.settings.theme.as_str() {
            "dark" => Mode::Dark,
            "light" => Mode::Light,
            _ => system,
        }
    }

    /// Keeps both modes' palettes wearing the chosen accent, so a mode switch never falls back to Ely's own.
    fn apply_accent(&self, cx: &mut Context<Self>) {
        let contrast = cx.theme().high_contrast();
        let index = accent_hue(&self.settings.accent);
        for mode in [Mode::Light, Mode::Dark] {
            let palette = index.map(|index| {
                let mut palette = match mode {
                    Mode::Light => Palette::light(contrast),
                    Mode::Dark => Palette::dark(contrast),
                };
                let color = palette.hue(index, "accent");
                palette.accent = color;
                palette.accent_hover = hover_of(color);
                palette.on_accent = readable_on(color);
                palette
            });
            Theme::set_palette(mode, palette, cx);
        }
    }

    pub(super) fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = match self.settings.theme.as_str() {
            "system" => "dark",
            "dark" => "light",
            _ => "system",
        };
        self.settings.theme = next.to_string();
        self.apply_theme(cx);
        self.save_theme();
    }

    fn save_theme(&self) {
        let _ = config::save_theme(&self.settings.theme, &self.settings.accent);
    }

    pub(super) fn ensure_icon(&mut self, hwnd: isize) {
        if self.icons.contains_key(&hwnd) {
            return;
        }
        let icon = window_icon(hwnd).and_then(render_image);
        self.icons.insert(hwnd, icon);
    }
}

/// The hue `name` picks, or none when it names nothing (`"none"` included).
pub(super) fn accent_hue(name: &str) -> Option<usize> {
    HUE_NAMES
        .iter()
        .position(|hue| hue.eq_ignore_ascii_case(name))
}

/// The accent's hover shade: darker on a light fill, lighter on a dark one.
fn hover_of(color: Hsla) -> Hsla {
    let shift = if is_light(color) { -0.08 } else { 0.08 };
    Hsla {
        l: (color.l + shift).clamp(0.0, 1.0),
        ..color
    }
}

/// Black or white, whichever stays readable on `color`.
fn readable_on(color: Hsla) -> Hsla {
    let l = if is_light(color) { 0.1 } else { 0.98 };
    Hsla {
        h: 0.0,
        s: 0.0,
        l,
        a: 1.0,
    }
}

fn is_light(color: Hsla) -> bool {
    let rgb = color.to_rgb();
    let channel = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luma = 0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b);
    luma > 0.2
}
