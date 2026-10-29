use ely_gpui_component::theme::{ActiveTheme, Mode, Palette, Theme, HUE_NAMES};
use gpui::{Context, Hsla};

use super::dto::render_image;
use super::Controller;
use crate::config;
use crate::win::{window_icon, Highlight};

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
        self.note_save(config::save(&self.settings));
    }

    /// Remembers the first write failure so the next tick can tell the user.
    pub(super) fn note_save(&self, result: std::io::Result<()>) {
        if let Err(err) = result {
            tracing::warn!("config write failed: {err}");
            self.save_error.replace(Some(err.to_string()));
        }
    }

    pub(super) fn flush_save_error(&mut self, cx: &mut Context<Self>) {
        let Some(err) = self.save_error.borrow_mut().take() else {
            return;
        };
        self.note_error(
            format!(
                "{err}. Check that {} is writable.",
                config::paths::data_dir().display()
            ),
            None,
            cx,
        );
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
        self.note_success("Settings reloaded", cx);
    }

    pub(super) fn apply_theme(&mut self, cx: &mut Context<Self>) {
        let system = Mode::from(cx.window_appearance());
        Theme::set_mode_now(self.resolve_mode(system), cx);
        self.apply_accent(cx);
    }

    fn resolve_mode(&self, system: Mode) -> Mode {
        match self.settings.theme {
            config::ThemeMode::Dark => Mode::Dark,
            config::ThemeMode::Light => Mode::Light,
            config::ThemeMode::System => system,
        }
    }

    /// Keeps both modes' palettes wearing the chosen accent, so a mode switch never falls back to Ely's own.
    fn apply_accent(&mut self, cx: &mut Context<Self>) {
        let contrast = cx.theme().high_contrast();
        let hue = accent_hue(self.settings.accent.name());
        for mode in [Mode::Light, Mode::Dark] {
            let mut palette = match mode {
                Mode::Light => Palette::light(contrast),
                Mode::Dark => Palette::dark(contrast),
            };
            let color = match hue {
                Some(index) => palette.hue(index, "accent"),
                None => neutral_accent(mode),
            };
            palette.accent = color;
            palette.accent_hover = hover_of(color);
            palette.on_accent = readable_on(color);
            Theme::set_palette(mode, Some(palette), cx);
        }
        // The pick frame wears the accent; a settings file naming no hue leaves it its own blue.
        let frame = match hue {
            Some(_) => colorref(cx.theme().colors.accent),
            None => Highlight::DEFAULT_COLOR,
        };
        self.highlight_color = frame;
        if let Some(highlight) = self.highlight.as_mut() {
            highlight.set_color(frame);
        }
    }

    pub(super) fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.settings.theme = self.settings.theme.next();
        self.apply_theme(cx);
        self.save_settings();
    }

    pub(super) fn ensure_icon(&mut self, hwnd: isize) {
        if self.icons.contains_key(&hwnd) {
            return;
        }
        let icon = window_icon(hwnd).and_then(render_image);
        self.icons.insert(hwnd, icon);
    }
}

/// The accent's hue `name` picks, or none when it names nothing (`"none"` included).
pub(super) fn accent_hue(name: &str) -> Option<usize> {
    HUE_NAMES
        .iter()
        .position(|hue| hue.eq_ignore_ascii_case(name))
}

/// Packs `color` the way `COLORREF` wants it: blue first, then green, then red.
fn colorref(color: Hsla) -> u32 {
    let rgb = color.to_rgb();
    let channel = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(rgb.b) << 16) | (channel(rgb.g) << 8) | channel(rgb.r)
}

/// The untinted accent, so `"none"` reads as white-on-dark and black-on-light rather than Ely's blue.
fn neutral_accent(mode: Mode) -> Hsla {
    let l = match mode {
        Mode::Dark => 0.93,
        Mode::Light => 0.16,
    };
    Hsla {
        h: 0.0,
        s: 0.0,
        l,
        a: 1.0,
    }
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

#[cfg(test)]
mod tests {
    use super::colorref;
    use gpui::Hsla;

    #[test]
    fn colorref_packs_blue_green_red() {
        let hue = |h: f32| Hsla {
            h,
            s: 1.0,
            l: 0.5,
            a: 1.0,
        };
        assert_eq!(colorref(hue(0.0)), 0x0000_00FF);
        assert_eq!(colorref(hue(2.0 / 3.0)), 0x00FF_0000);
    }
}
