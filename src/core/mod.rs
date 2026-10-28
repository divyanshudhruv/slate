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

/// Resolves a case-insensitive name against a table of settings values.
pub fn from_name<T: Copy>(text: &str, table: &[(&str, T)]) -> Option<T> {
    let lower = text.to_ascii_lowercase();
    table
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, value)| *value)
}
