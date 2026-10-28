use super::dto::PickerRow;
use super::Controller;
use crate::win::{enumerate_windows, is_blacklisted, slate_elevated};
use std::collections::HashSet;

impl Controller {
    pub(super) fn set_picker_open(&mut self, open: bool) {
        self.menus.picker_picks.clear();
        self.menus.picker_cursor = None;
        if open {
            self.close_other_dropdowns();
            self.windows = enumerate_windows();
            self.refresh_picker();
        } else {
            let hosted: HashSet<isize> = self.styling.keys().copied().collect();
            self.icons.retain(|hwnd, _| hosted.contains(hwnd));
        }
        self.menus.picker_open = open;
    }

    pub(super) fn refresh_picker(&mut self) {
        let hosted: HashSet<isize> = self.styling.keys().copied().collect();
        let eligible: Vec<isize> = self
            .windows
            .iter()
            .filter(|w| !is_blacklisted(&w.class) && !hosted.contains(&w.hwnd))
            .filter(|w| !w.elevated || slate_elevated())
            .filter(|w| {
                !self
                    .rule_for(&w.exe_name)
                    .is_some_and(|rule| rule.never_host)
            })
            .map(|w| w.hwnd)
            .collect();
        for hwnd in eligible {
            self.ensure_icon(hwnd);
        }
        let mut rows: Vec<PickerRow> = self
            .windows
            .iter()
            .map(|w| {
                let reason = if is_blacklisted(&w.class) {
                    Some("This window can't be hosted".to_string())
                } else if self
                    .rule_for(&w.exe_name)
                    .is_some_and(|rule| rule.never_host)
                {
                    Some("Blocked by an app rule in settings.json".to_string())
                } else if w.elevated && !slate_elevated() {
                    Some("Run Slate as administrator to host this".to_string())
                } else if hosted.contains(&w.hwnd) {
                    Some("Already hosted".to_string())
                } else {
                    None
                };
                PickerRow {
                    hwnd: w.hwnd,
                    title: if w.title.is_empty() {
                        w.exe_name.clone()
                    } else {
                        w.title.clone()
                    },
                    exe: w.exe_name.clone(),
                    icon: self.icons.get(&w.hwnd).cloned().flatten(),
                    reason,
                }
            })
            .collect();
        rows.sort_by_cached_key(|row| (row.reason.is_some(), row.title.to_lowercase()));
        self.menus.picker_rows = rows;
        if self
            .menus
            .picker_cursor
            .is_some_and(|ix| ix >= self.menus.picker_rows.len())
        {
            self.menus.picker_cursor = None;
        }
    }

    pub(super) fn toggle_picker_pick(&mut self, hwnd: isize) {
        if let Some(ix) = self.menus.picker_picks.iter().position(|h| *h == hwnd) {
            self.menus.picker_picks.remove(ix);
        } else if self.menus.picker_rows.iter().any(|row| row.hwnd == hwnd) {
            self.menus.picker_picks.push(hwnd);
        }
    }

    /// Steps the highlight along the pickable rows, wrapping at both ends.
    pub(super) fn move_picker_cursor(&mut self, delta: isize) {
        let open: Vec<usize> = self
            .menus
            .picker_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.reason.is_none())
            .map(|(ix, _)| ix)
            .collect();
        if open.is_empty() {
            self.menus.picker_cursor = None;
            return;
        }
        let next = match self
            .menus
            .picker_cursor
            .and_then(|ix| open.iter().position(|o| *o == ix))
        {
            Some(pos) => {
                let step = pos as isize + delta;
                let len = open.len() as isize;
                open[step.rem_euclid(len) as usize]
            }
            None if delta < 0 => open[open.len() - 1],
            None => open[0],
        };
        self.menus.picker_cursor = Some(next);
    }

    pub(super) fn toggle_picker_cursor(&mut self) {
        let Some(row) = self
            .menus
            .picker_cursor
            .and_then(|ix| self.menus.picker_rows.get(ix))
        else {
            return;
        };
        if row.reason.is_some() {
            return;
        }
        let hwnd = row.hwnd;
        self.toggle_picker_pick(hwnd);
    }
}
