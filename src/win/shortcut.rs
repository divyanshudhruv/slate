//! Tabs are switched with system hotkeys, registered only while one of our windows has focus.
//! A global keyboard hook would see every key on the machine; this only ever hears its own chords.

use std::sync::atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
    MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VIRTUAL_KEY, VK_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetMessageW, PeekMessageW, PostThreadMessageW, EVENT_SYSTEM_FOREGROUND,
    MSG, PM_NOREMOVE, WINEVENT_OUTOFCONTEXT, WM_APP, WM_HOTKEY, WM_QUIT,
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

#[derive(Clone, Copy)]
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

/// One system hotkey to hold while our windows are in front.
#[derive(Clone, Copy)]
struct Hotkey {
    id: i32,
    mods: HOT_KEY_MODIFIERS,
    vk: u32,
}

impl PartialEq for Hotkey {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.mods.0 == other.mods.0 && self.vk == other.vk
    }
}

impl Eq for Hotkey {}

const ID_NEXT: i32 = 1;
const ID_PREV: i32 = 2;
const ID_CLOSE: i32 = 3;
/// Digit ids run 11 to 19, so the index is `id - 10`.
const ID_DIGIT_BASE: i32 = 10;
const VK_DIGIT_1: u32 = 0x31;

fn hotkey_id(shortcut: Shortcut) -> Option<i32> {
    match shortcut {
        Shortcut::NextTab => Some(ID_NEXT),
        Shortcut::PrevTab => Some(ID_PREV),
        Shortcut::CloseTab => Some(ID_CLOSE),
        Shortcut::SelectIndex(_) => None,
    }
}

fn action_for_id(id: i32) -> Option<Shortcut> {
    match id {
        ID_NEXT => Some(Shortcut::NextTab),
        ID_PREV => Some(Shortcut::PrevTab),
        ID_CLOSE => Some(Shortcut::CloseTab),
        11..=19 => Some(Shortcut::SelectIndex((id - ID_DIGIT_BASE) as u8)),
        _ => None,
    }
}

fn modifiers(chord: Chord) -> HOT_KEY_MODIFIERS {
    let mut mods = MOD_NOREPEAT;
    if chord.ctrl {
        mods |= MOD_CONTROL;
    }
    if chord.shift {
        mods |= MOD_SHIFT;
    }
    if chord.alt {
        mods |= MOD_ALT;
    }
    if chord.win {
        mods |= MOD_WIN;
    }
    mods
}

/// Expands the binding table into the ids the system is asked to hold.
fn hotkeys(table: &[Binding]) -> Vec<Hotkey> {
    let mut out = Vec::new();
    for binding in table {
        let mods = modifiers(binding.chord);
        if binding.digits {
            for digit in 0..9u32 {
                out.push(Hotkey {
                    id: ID_DIGIT_BASE + 1 + digit as i32,
                    mods,
                    vk: VK_DIGIT_1 + digit,
                });
            }
        } else if let Some(id) = hotkey_id(binding.shortcut) {
            out.push(Hotkey {
                id,
                mods,
                vk: binding.vk.unwrap_or(binding.chord.vk) as u32,
            });
        }
    }
    out
}

static BINDINGS: Mutex<Vec<Binding>> = Mutex::new(Vec::new());
static OWNED: Mutex<Vec<isize>> = Mutex::new(Vec::new());
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// The first chord the system refused, held until the UI shows it once.
static FAILED: Mutex<Option<String>> = Mutex::new(None);
/// True once a refusal has been queued, so a failing re-register does not repeat the banner.
static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Swaps the table the hotkey thread reads; unbindable chords are dropped, so it matches the validated settings.
pub fn set_bindings(entries: &[(Shortcut, Chord)]) {
    let table: Vec<Binding> = entries
        .iter()
        .filter_map(|(shortcut, chord)| Binding::new(*shortcut, *chord))
        .collect();
    let mut current = BINDINGS.lock().unwrap_or_else(|e| e.into_inner());
    *current = table;
    drop(current);
    wake();
}

/// Keeps the set of windows that may own the hotkeys, then asks the thread to re-check the foreground.
pub fn set_owned(hwnds: impl IntoIterator<Item = isize>) {
    let next: Vec<isize> = hwnds.into_iter().collect();
    let changed = {
        let mut owned = OWNED.lock().unwrap_or_else(|e| e.into_inner());
        if *owned == next {
            false
        } else {
            *owned = next;
            true
        }
    };
    if changed {
        wake();
    }
}

/// Wakes the hotkey thread; a missing id means it is not running, so there is nothing to tell.
fn wake() {
    let id = THREAD_ID.load(Ordering::Acquire);
    if id != 0 {
        unsafe {
            let _ = PostThreadMessageW(id, WM_APP + 1, WPARAM(0), LPARAM(0));
        }
    }
}

fn foreground_owned() -> bool {
    let hwnd = unsafe { GetForegroundWindow() }.0 as isize;
    if hwnd == 0 {
        return false;
    }
    OWNED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&hwnd)
}

/// Re-registers only when the wanted set changed, so a wake costs one comparison.
fn sync(held: &mut Vec<Hotkey>, wanted: &[Hotkey]) {
    if held.as_slice() == wanted {
        return;
    }
    for hotkey in held.iter() {
        unsafe {
            let _ = UnregisterHotKey(None, hotkey.id);
        }
    }
    held.clear();
    let mut refused: Option<String> = None;
    for hotkey in wanted {
        match unsafe { RegisterHotKey(None, hotkey.id, hotkey.mods, hotkey.vk) } {
            Ok(()) => held.push(*hotkey),
            Err(err) => {
                tracing::warn!("shortcut {} could not be registered: {err}", hotkey.id);
                refused.get_or_insert_with(|| describe(hotkey));
            }
        }
    }
    if refused.is_none() {
        // Every chord is ours again, so a later conflict deserves its own notice.
        WARNED.store(false, Ordering::Release);
        return;
    }
    if !WARNED.swap(true, Ordering::AcqRel) {
        *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = refused;
    }
}

/// A chord the system refused, for the UI to turn into a one-time banner.
pub fn take_failure() -> Option<String> {
    FAILED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The system's view of a chord, in the same order the settings file writes it.
fn describe(hotkey: &Hotkey) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (bit, name) in [
        (MOD_CONTROL, "ctrl"),
        (MOD_SHIFT, "shift"),
        (MOD_ALT, "alt"),
        (MOD_WIN, "win"),
    ] {
        if hotkey.mods & bit == bit {
            parts.push(name.to_string());
        }
    }
    let key = crate::core::keys::name_for(hotkey.vk as u16)
        .unwrap_or_else(|| format!("key {}", hotkey.vk));
    parts.push(key);
    parts.join("+")
}

/// Any foreground change is enough to re-check whether the hotkeys should be held.
unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: windows::Win32::Foundation::HWND,
    _idobject: i32,
    _idchild: i32,
    _thread: u32,
    _time: u32,
) {
    wake();
}

/// Single-producer ring between the hotkey thread and the UI; no locks on either side.
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

fn run(ready: mpsc::Sender<u32>) {
    let id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    // Peeking first gives the thread a message queue, so every later wake has somewhere to land.
    let mut msg = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    }
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    THREAD_ID.store(id, Ordering::Release);
    let _ = ready.send(id);

    let mut held: Vec<Hotkey> = Vec::new();
    loop {
        let wanted = if foreground_owned() {
            let table = BINDINGS.lock().unwrap_or_else(|e| e.into_inner());
            hotkeys(&table)
        } else {
            Vec::new()
        };
        sync(&mut held, &wanted);

        let mut msg = MSG::default();
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        if msg.message == WM_HOTKEY {
            if let Some(shortcut) = action_for_id(msg.wParam.0 as i32) {
                push(shortcut);
            }
        }
        // Any other message, including our own wake, just re-checks the foreground.
    }

    for hotkey in held {
        unsafe {
            let _ = UnregisterHotKey(None, hotkey.id);
        }
    }
    if !hook.0.is_null() {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
    THREAD_ID.store(0, Ordering::Release);
}

pub struct ShortcutCapture {
    thread: u32,
    handle: Option<JoinHandle<()>>,
}

impl ShortcutCapture {
    pub fn install() -> Option<Self> {
        let (ready, ready_rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("slate-hotkeys".into())
            .spawn(move || run(ready))
            .ok()?;
        let thread = ready_rx.recv().ok()?;
        Some(Self {
            thread,
            handle: Some(handle),
        })
    }

    pub fn drain(&self) -> Vec<Shortcut> {
        take_all()
    }
}

impl Drop for ShortcutCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn held(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

pub fn ctrl_held() -> bool {
    held(VK_CONTROL)
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
    fn each_action_gets_a_stable_id_and_back() {
        for shortcut in [Shortcut::NextTab, Shortcut::PrevTab, Shortcut::CloseTab] {
            let id = hotkey_id(shortcut).expect("id");
            assert_eq!(action_for_id(id), Some(shortcut));
        }
        assert_eq!(action_for_id(11), Some(Shortcut::SelectIndex(1)));
        assert_eq!(action_for_id(19), Some(Shortcut::SelectIndex(9)));
        assert_eq!(action_for_id(0), None);
        assert_eq!(action_for_id(10), None);
        assert_eq!(action_for_id(20), None);
    }

    #[test]
    fn chord_modifiers_become_system_modifiers() {
        let mods = modifiers(chord(0x57, true, false));
        assert_eq!(mods & MOD_CONTROL, MOD_CONTROL);
        assert_eq!(mods & MOD_SHIFT, HOT_KEY_MODIFIERS(0));
        assert_eq!(mods & MOD_NOREPEAT, MOD_NOREPEAT);
    }

    #[test]
    fn digit_bindings_expand_to_the_whole_family() {
        let table = vec![Binding::new(Shortcut::SelectIndex(1), chord(0x31, true, false)).unwrap()];
        let keys = hotkeys(&table);
        assert_eq!(keys.len(), 9);
        assert_eq!(keys[0].id, 11);
        assert_eq!(keys[0].vk, 0x31);
        assert_eq!(keys[8].id, 19);
        assert_eq!(keys[8].vk, 0x39);
    }

    #[test]
    fn plain_bindings_map_one_to_one() {
        let table = vec![
            Binding::new(Shortcut::CloseTab, chord(0x57, true, false)).unwrap(),
            Binding::new(Shortcut::NextTab, chord(0x09, true, false)).unwrap(),
        ];
        let keys = hotkeys(&table);
        assert_eq!(keys.len(), 2);
        assert_eq!((keys[0].id, keys[0].vk), (ID_CLOSE, 0x57));
        assert_eq!((keys[1].id, keys[1].vk), (ID_NEXT, 0x09));
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
    fn a_refused_chord_is_described_like_the_settings_file() {
        let shape = Hotkey {
            id: ID_NEXT,
            mods: modifiers(chord(0x09, true, true)),
            vk: 0x09,
        };
        assert_eq!(describe(&shape), "ctrl+shift+tab");
    }

    #[test]
    fn invalid_chords_are_not_packed() {
        assert!(Binding::new(Shortcut::NextTab, chord(0, false, false)).is_none());
    }

    #[test]
    fn the_owned_set_replaces_rather_than_appends() {
        set_owned(1..=2_000);
        assert!(OWNED.lock().unwrap().contains(&2_000));
        set_owned([7]);
        assert_eq!(*OWNED.lock().unwrap(), vec![7]);
    }
}
