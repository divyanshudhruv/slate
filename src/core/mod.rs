pub mod group;
pub mod identity;
pub mod install;
pub mod keys;
pub mod matching;
pub mod update;

pub use group::{Effect, GroupSet};
pub use identity::{GroupId, TabId, WindowIdentity};
pub use matching::{match_tabs, TabHint, Target};
pub use update::UpdateState;

/// The one id policy: a counter hands out its value, then moves on, wrapping at the top.
/// Wrapping is deliberate - a long session must not panic, and an id only repeats after 2^64 takes.
pub fn take_id(counter: &mut u64) -> u64 {
    let id = *counter;
    *counter = counter.wrapping_add(1);
    id
}

/// Steps a menu's keyboard cursor through the rows that can run, wrapping at both ends.
/// `None` enters from the end the step points at, so the first key press always lands on a row.
pub fn step_cursor(current: Option<usize>, enabled: &[usize], delta: isize) -> Option<usize> {
    if enabled.is_empty() {
        return None;
    }
    let len = enabled.len() as isize;
    let landed = match current.and_then(|ix| enabled.iter().position(|&row| row == ix)) {
        Some(pos) => (pos as isize + delta).rem_euclid(len),
        None if delta < 0 => len - 1,
        None => 0,
    };
    Some(enabled[landed as usize])
}

/// Resolves a case-insensitive name against a table of settings values.
pub fn from_name<T: Copy>(text: &str, table: &[(&str, T)]) -> Option<T> {
    let lower = text.to_ascii_lowercase();
    table
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, value)| *value)
}

#[cfg(test)]
mod tests {
    use super::step_cursor;

    #[test]
    fn a_fresh_cursor_enters_from_the_end_it_steps_toward() {
        assert_eq!(step_cursor(None, &[1, 3, 5], 1), Some(1));
        assert_eq!(step_cursor(None, &[1, 3, 5], -1), Some(5));
    }

    #[test]
    fn the_cursor_wraps_at_both_ends_and_skips_disabled_rows() {
        assert_eq!(step_cursor(Some(1), &[1, 3, 5], 1), Some(3));
        assert_eq!(step_cursor(Some(5), &[1, 3, 5], 1), Some(1));
        assert_eq!(step_cursor(Some(1), &[1, 3, 5], -1), Some(5));
        assert_eq!(step_cursor(Some(3), &[1, 3, 5], 4), Some(5));
    }

    #[test]
    fn an_empty_menu_has_no_cursor() {
        assert_eq!(step_cursor(None, &[], 1), None);
        assert_eq!(step_cursor(Some(2), &[], -1), None);
    }

    #[test]
    fn a_cursor_left_on_a_disabled_row_re_enters_the_list() {
        assert_eq!(step_cursor(Some(4), &[1, 3, 5], 1), Some(1));
        assert_eq!(step_cursor(Some(4), &[1, 3, 5], -1), Some(5));
    }
}
