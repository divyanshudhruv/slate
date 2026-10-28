pub mod activate;
pub mod capture;
pub mod compat;
pub mod enumerate;
pub mod events;
pub mod highlight;
pub mod host;
pub mod icon;
pub mod instance;
pub mod integrity;
pub mod janitor;
pub mod keys;
pub mod pick;
pub mod shell;
pub mod shell_link;
pub mod shortcut;
pub mod stack;
pub mod tray;
pub mod update;

pub use activate::{activate_process, activate_window, close_window, move_window, window_rect};
pub use capture::{capture_window, WindowSnapshot};
pub use compat::is_blacklisted;
pub use enumerate::{enumerate_windows, window_title};
pub use events::{WinEvent, WinEventPump};
pub use highlight::Highlight;
pub use host::{content_rect_screen, register_member, unregister_member, HostWorker, StylingState};
pub use icon::{install_window_icon, window_icon, IconPixels};
pub use integrity::slate_elevated;
pub use janitor::{process_alive, Janitor};
pub use pick::{window_under_cursor, KeyboardCapture, PickKey};
pub use shortcut::{ctrl_held, set_owned, Shortcut, ShortcutCapture};
pub use tray::TrayIcon;

/// Keeps a thread's COM apartment balanced, including on early returns.
pub(crate) struct ComApartment(bool);

impl ComApartment {
    /// Initializes COM on this thread; a failure leaves the guard inert.
    pub(crate) fn init() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        // SAFETY: no pointer is passed, so nothing can outlive the call.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // S_FALSE means the thread already had COM, and that still needs a matching uninit.
        Self(result.is_ok())
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        use windows::Win32::System::Com::CoUninitialize;
        if self.0 {
            // SAFETY: paired with the successful init above, on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}
