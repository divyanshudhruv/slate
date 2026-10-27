use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::core::keys::Chord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NextTab,
    PrevTab,
    CloseTab,
    /// Triggered by holding the SelectTab chord's modifiers and pressing 1-9
    SelectIndex(u8),
}

struct Binding {
    shortcut: Shortcut,
    /// None means "any digit 1-9": the SelectTab action binds a family, not one key.
    vk: Option<u16>,
    chord: Chord,
    digits: bool,
}

impl Binding {
    fn new(shortcut: Shortcut, chord: Chord) -> Option<Self> {
        use crate::core::keys::chord_valid;
        if !chord_valid(chord) {
            return None;
        }
        let digits = matches!(shortcut, Shortcut::SelectIndex(_));
        let vk = (!digits).then_some(chord.vk);
        Some(Self {
            shortcut,
            vk,
            chord,
            digits,
        })
    }
}

impl PartialEq<u16> for Binding {
    fn eq(&self, vk: &u16) -> bool {
        if self.digits {
            // A digit binding stands for the whole 1-9 family the action selects into.
            (0x31..=0x39).contains(vk)
        } else {
            self.vk == Some(*vk)
        }
    }
}

fn bindings() -> &'static Mutex<Vec<Binding>> {
    static BINDINGS: OnceLock<Mutex<Vec<Binding>>> = OnceLock::new();
    BINDINGS.get_or_init(|| Mutex::new(Vec::new()))
}

fn queue() -> &'static Mutex<Vec<Shortcut>> {
    static QUEUE: OnceLock<Mutex<Vec<Shortcut>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Swaps the shortcut table the hook reads. Chords that cannot be bound are dropped,
/// so the table always matches what the settings dialog validated.
pub fn set_bindings(entries: &[(Shortcut, Chord)]) {
    let table: Vec<Binding> = entries
        .iter()
        .filter_map(|(s, c)| Binding::new(*s, *c))
        .collect();
    if let Ok(mut bindings) = bindings().lock() {
        *bindings = table;
    }
}

const MAX_OWNED: usize = 1024;

struct OwnedSet {
    len: std::sync::atomic::AtomicUsize,
    items: [std::sync::atomic::AtomicIsize; MAX_OWNED],
}

impl OwnedSet {
    const fn new() -> Self {
        Self {
            len: std::sync::atomic::AtomicUsize::new(0),
            items: [const { std::sync::atomic::AtomicIsize::new(0) }; MAX_OWNED],
        }
    }

    fn set(&self, hwnds: impl IntoIterator<Item = isize>) {
        use std::sync::atomic::Ordering;
        let mut n = 0;
        for hwnd in hwnds {
            if n == MAX_OWNED {
                break;
            }
            self.items[n].store(hwnd, Ordering::Relaxed);
            n += 1;
        }
        self.len.store(n, Ordering::Release);
    }

    fn contains(&self, hwnd: isize) -> bool {
        use std::sync::atomic::Ordering;
        let n = self.len.load(Ordering::Acquire).min(MAX_OWNED);
        (0..n).any(|i| self.items[i].load(Ordering::Relaxed) == hwnd)
    }
}

fn owned() -> &'static OwnedSet {
    static OWNED: OwnedSet = OwnedSet::new();
    &OWNED
}

pub fn set_owned(hwnds: impl IntoIterator<Item = isize>) {
    owned().set(hwnds);
}

fn held(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

pub fn ctrl_held() -> bool {
    held(VK_CONTROL)
}

fn modifiers_match(chord: Chord, alt_down: bool) -> bool {
    let win_held = held(VK_LWIN) || held(VK_RWIN);
    chord.ctrl == held(VK_CONTROL)
        && chord.shift == held(VK_SHIFT)
        && chord.alt == (alt_down || held(VK_MENU))
        && chord.win == win_held
}

fn match_shortcut(vk: u16, alt_down: bool) -> Option<Shortcut> {
    let Ok(table) = bindings().lock() else {
        return None;
    };
    table.iter().find_map(|binding| {
        if binding == &vk && modifiers_match(binding.chord, alt_down) {
            match binding.shortcut {
                Shortcut::SelectIndex(_) if binding.digits => {
                    Some(Shortcut::SelectIndex((vk - 0x31 + 1) as u8))
                }
                shortcut => Some(shortcut),
            }
        } else {
            None
        }
    })
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
            let keyboard = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = keyboard.vkCode as u16;
            let alt_held = keyboard.flags.0 & LLKHF_ALTDOWN.0 != 0;
            if let Some(shortcut) = match_shortcut(vk, alt_held) {
                let foreground = unsafe { GetForegroundWindow() }.0 as isize;
                if owned().contains(foreground) {
                    if let Ok(mut queue) = queue().lock() {
                        queue.push(shortcut);
                    }
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub struct ShortcutCapture {
    hook: HHOOK,
}

impl ShortcutCapture {
    pub fn install() -> Option<Self> {
        let hook =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) }.ok()?;
        Some(Self { hook })
    }

    pub fn drain(&self) -> Vec<Shortcut> {
        queue()
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }
}

impl Drop for ShortcutCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
        }
    }
}
