use std::sync::atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering};

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

/// Fixed table the hook reads without locking; `set_bindings` only rewrites it.
const BINDING_SLOTS: usize = 4;
const VALID_BIT: u32 = 1 << 21;
const DIGITS_BIT: u32 = 1 << 20;

static BINDINGS: [AtomicU32; BINDING_SLOTS] = [const { AtomicU32::new(0) }; BINDING_SLOTS];
static BINDING_COUNT: AtomicUsize = AtomicUsize::new(0);

fn action_code(shortcut: Shortcut) -> u32 {
    match shortcut {
        Shortcut::NextTab => 0,
        Shortcut::PrevTab => 1,
        Shortcut::CloseTab => 2,
        Shortcut::SelectIndex(_) => 3,
    }
}

fn pack(binding: &Binding) -> u32 {
    binding.vk.unwrap_or(0) as u32
        | (binding.chord.ctrl as u32) << 16
        | (binding.chord.shift as u32) << 17
        | (binding.chord.alt as u32) << 18
        | (binding.chord.win as u32) << 19
        | (binding.digits as u32) << 20
        | VALID_BIT
        | action_code(binding.shortcut) << 22
}

/// Swaps the shortcut table the hook reads. Chords that cannot be bound are dropped,
/// so the table always matches what the settings dialog validated.
pub fn set_bindings(entries: &[(Shortcut, Chord)]) {
    let table: Vec<Binding> = entries
        .iter()
        .filter_map(|(s, c)| Binding::new(*s, *c))
        .collect();
    let count = table.len().min(BINDING_SLOTS);
    for (slot, binding) in BINDINGS.iter().zip(table.iter()) {
        slot.store(pack(binding), Ordering::Release);
    }
    BINDING_COUNT.store(count, Ordering::Release);
}

/// Window set read by the hook while the UI thread updates it, without truncation.
struct OwnedSet {
    seq: AtomicUsize,
    readers: AtomicUsize,
    items: std::cell::UnsafeCell<Vec<isize>>,
}

unsafe impl Sync for OwnedSet {}

impl OwnedSet {
    const fn new() -> Self {
        Self {
            seq: AtomicUsize::new(0),
            readers: AtomicUsize::new(0),
            items: std::cell::UnsafeCell::new(Vec::new()),
        }
    }

    fn set(&self, hwnds: impl IntoIterator<Item = isize>) {
        let next: Vec<isize> = hwnds.into_iter().collect();
        self.seq.fetch_add(1, Ordering::AcqRel);
        while self.readers.load(Ordering::Acquire) != 0 {
            std::thread::yield_now();
        }
        unsafe {
            *self.items.get() = next;
        }
        self.seq.fetch_add(1, Ordering::Release);
    }

    fn contains(&self, hwnd: isize) -> bool {
        loop {
            let seq = self.seq.load(Ordering::Acquire);
            if seq & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            self.readers.fetch_add(1, Ordering::AcqRel);
            if self.seq.load(Ordering::Acquire) != seq {
                self.readers.fetch_sub(1, Ordering::AcqRel);
                continue;
            }
            let found = unsafe { (*self.items.get()).contains(&hwnd) };
            self.readers.fetch_sub(1, Ordering::Release);
            return found;
        }
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

fn modifiers_match(ctrl: bool, shift: bool, alt: bool, win: bool, alt_down: bool) -> bool {
    ctrl == held(VK_CONTROL)
        && shift == held(VK_SHIFT)
        && alt == (alt_down || held(VK_MENU))
        && win == (held(VK_LWIN) || held(VK_RWIN))
}

fn match_shortcut(vk: u16, alt_down: bool) -> Option<Shortcut> {
    let count = BINDING_COUNT.load(Ordering::Acquire).min(BINDING_SLOTS);
    for slot in BINDINGS.iter().take(count) {
        let bits = slot.load(Ordering::Acquire);
        if bits & VALID_BIT == 0 {
            continue;
        }
        let digits = bits & DIGITS_BIT != 0;
        if digits {
            if !(0x31..=0x39).contains(&vk) {
                continue;
            }
        } else if (bits & 0xFFFF) as u16 != vk {
            continue;
        }
        if !modifiers_match(
            bits & (1 << 16) != 0,
            bits & (1 << 17) != 0,
            bits & (1 << 18) != 0,
            bits & (1 << 19) != 0,
            alt_down,
        ) {
            continue;
        }
        return match (bits >> 22) & 0x3 {
            0 => Some(Shortcut::NextTab),
            1 => Some(Shortcut::PrevTab),
            2 => Some(Shortcut::CloseTab),
            3 => Some(Shortcut::SelectIndex((vk - 0x31 + 1) as u8)),
            _ => None,
        };
    }
    None
}

/// Single-producer ring between the hook and the UI; no locks in the hook path.
const QUEUE_LEN: usize = 64;
static QUEUE_SLOTS: [AtomicU8; QUEUE_LEN] = [const { AtomicU8::new(0) }; QUEUE_LEN];
static QUEUE_HEAD: AtomicUsize = AtomicUsize::new(0);
static QUEUE_TAIL: AtomicUsize = AtomicUsize::new(0);

fn encode_shortcut(shortcut: Shortcut) -> u8 {
    match shortcut {
        Shortcut::NextTab => 1,
        Shortcut::PrevTab => 2,
        Shortcut::CloseTab => 3,
        Shortcut::SelectIndex(n) => 10 + n,
    }
}

fn decode_shortcut(code: u8) -> Option<Shortcut> {
    match code {
        1 => Some(Shortcut::NextTab),
        2 => Some(Shortcut::PrevTab),
        3 => Some(Shortcut::CloseTab),
        11..=19 => Some(Shortcut::SelectIndex(code - 10)),
        _ => None,
    }
}

fn push(shortcut: Shortcut) {
    let tail = QUEUE_TAIL.load(Ordering::Relaxed);
    let head = QUEUE_HEAD.load(Ordering::Acquire);
    if tail.wrapping_sub(head) >= QUEUE_LEN {
        return;
    }
    QUEUE_SLOTS[tail % QUEUE_LEN].store(encode_shortcut(shortcut), Ordering::Relaxed);
    QUEUE_TAIL.store(tail.wrapping_add(1), Ordering::Release);
}

fn take_all() -> Vec<Shortcut> {
    let mut out = Vec::new();
    let mut head = QUEUE_HEAD.load(Ordering::Relaxed);
    let tail = QUEUE_TAIL.load(Ordering::Acquire);
    while head != tail {
        let code = QUEUE_SLOTS[head % QUEUE_LEN].load(Ordering::Relaxed);
        if let Some(shortcut) = decode_shortcut(code) {
            out.push(shortcut);
        }
        head = head.wrapping_add(1);
    }
    QUEUE_HEAD.store(head, Ordering::Release);
    out
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
                    push(shortcut);
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
        take_all()
    }
}

impl Drop for ShortcutCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(vk: u16, ctrl: bool, shift: bool) -> Chord {
        Chord {
            ctrl,
            shift,
            alt: false,
            win: false,
            vk,
        }
    }

    #[test]
    fn pack_keeps_the_key_modifiers_and_action() {
        let binding = Binding::new(Shortcut::CloseTab, chord(0x57, true, false)).unwrap();
        let bits = pack(&binding);
        assert_eq!(bits & 0xFFFF, 0x57);
        assert_ne!(bits & (1 << 16), 0);
        assert_eq!(bits & (1 << 17), 0);
        assert_ne!(bits & VALID_BIT, 0);
        assert_eq!((bits >> 22) & 0x3, 2);
    }

    #[test]
    fn digit_bindings_cover_the_whole_family() {
        let binding = Binding::new(Shortcut::SelectIndex(1), chord(0x31, true, false)).unwrap();
        let bits = pack(&binding);
        assert_ne!(bits & DIGITS_BIT, 0);
        assert_eq!((bits >> 22) & 0x3, 3);
    }

    #[test]
    fn queue_keeps_order_and_drains_clean() {
        push(Shortcut::NextTab);
        push(Shortcut::SelectIndex(3));
        push(Shortcut::CloseTab);
        assert_eq!(
            take_all(),
            vec![
                Shortcut::NextTab,
                Shortcut::SelectIndex(3),
                Shortcut::CloseTab
            ]
        );
        assert!(take_all().is_empty());
    }

    #[test]
    fn invalid_chords_are_not_packed() {
        assert!(Binding::new(Shortcut::NextTab, chord(0, false, false)).is_none());
    }

    #[test]
    fn owned_set_is_not_truncated() {
        set_owned(1..=2_000);
        assert!(owned().contains(1));
        assert!(owned().contains(2_000));
        assert!(!owned().contains(0));
    }
}
