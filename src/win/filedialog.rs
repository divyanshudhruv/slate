use std::path::PathBuf;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Asks for a JSON file; None when the user cancels.
pub fn choose_json(open: bool, title: &str, suggested: &str) -> Option<PathBuf> {
    let filter = wide("Slate sets (*.json)\0*.json\0All files (*.*)\0*.*\0");
    let title = wide(title);
    let mut buffer = [0u16; 1024];
    for (slot, ch) in buffer.iter_mut().zip(suggested.encode_utf16()) {
        *slot = ch;
    }
    let mut flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST;
    flags |= if open {
        OFN_FILEMUSTEXIST
    } else {
        OFN_OVERWRITEPROMPT
    };
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: flags,
        ..Default::default()
    };
    let picked = unsafe {
        if open {
            GetOpenFileNameW(&mut dialog)
        } else {
            GetSaveFileNameW(&mut dialog)
        }
    };
    if !picked.as_bool() {
        return None;
    }
    let end = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..end]);
    (!path.is_empty()).then(|| PathBuf::from(path))
}
