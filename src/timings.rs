use std::time::Duration;

/// How often the controller processes queued work while active.
pub const UI_TICK: Duration = Duration::from_millis(50);
/// Backoff when nothing interacts with the host; shortcuts arrive on a global hook, so it bounds queued chords.
pub const UI_IDLE_TICK: Duration = Duration::from_millis(80);
/// How often the pick overlay checks for Tab/Escape.
pub const PICK_POLL: Duration = Duration::from_millis(40);
/// How often the host worker re-checks member windows while settling.
pub const MEMBER_SYNC: Duration = Duration::from_millis(50);
/// Idle member sync interval; location events wake the worker sooner.
pub const MEMBER_SYNC_IDLE: Duration = Duration::from_millis(250);
/// Extra passes that re-place a member after a move, for apps that fight it.
pub const SETTLE_PASSES: u8 = 12;
/// How long the hover card lingers after the pointer leaves.
pub const PREVIEW_LINGER_MS: u64 = 200;
/// Default delay before the hover card appears.
pub const DEFAULT_PREVIEW_DELAY_MS: u64 = 500;
/// Upper bound for the user-configurable preview delay.
pub const MAX_PREVIEW_DELAY_MS: u64 = 5_000;
/// How long the swap helper waits for Slate to exit before swapping anyway.
pub const SWAP_WAIT_SECS: u32 = 600;
