use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId,
    SetWindowsHookExW, UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_ALTDOWN, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKey {
    Tab,
    Escape,
}

/// Keyboard picking never runs longer than this, even if the UI stops polling.
const WATCHDOG: Duration = Duration::from_secs(10);

static TAB_PENDING: AtomicBool = AtomicBool::new(false);
static ESCAPE_PENDING: AtomicBool = AtomicBool::new(false);

/// Only the app's own windows may drive the picker, so stray global keys are ignored.
fn own_foreground() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
            let keyboard = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let alt_held = keyboard.flags.0 & LLKHF_ALTDOWN.0 != 0;
            if !alt_held && own_foreground() {
                let key = keyboard.vkCode;
                if key == VK_TAB.0 as u32 {
                    TAB_PENDING.store(true, Ordering::SeqCst);
                    return LRESULT(1);
                }
                if key == VK_ESCAPE.0 as u32 {
                    ESCAPE_PENDING.store(true, Ordering::SeqCst);
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub struct KeyboardCapture {
    hook: HHOOK,
    started: Instant,
}

impl KeyboardCapture {
    pub fn install() -> Option<Self> {
        TAB_PENDING.store(false, Ordering::SeqCst);
        ESCAPE_PENDING.store(false, Ordering::SeqCst);
        let hook =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) }.ok()?;
        Some(Self {
            hook,
            started: Instant::now(),
        })
    }

    pub fn drain(&self) -> Vec<PickKey> {
        if self.started.elapsed() > WATCHDOG {
            return vec![PickKey::Escape];
        }
        let mut keys = Vec::new();
        if TAB_PENDING.swap(false, Ordering::SeqCst) {
            keys.push(PickKey::Tab);
        }
        if ESCAPE_PENDING.swap(false, Ordering::SeqCst) {
            keys.push(PickKey::Escape);
        }
        keys
    }
}

impl Drop for KeyboardCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
        }
    }
}

pub fn window_under_cursor() -> Option<isize> {
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).ok()?;
        let hwnd = WindowFromPoint(point);
        if hwnd.0.is_null() {
            return None;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        (!root.0.is_null()).then_some(root.0 as isize)
    }
}
