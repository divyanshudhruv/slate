use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, MapVirtualKeyW, VkKeyScanW, MAPVK_VK_TO_VSC,
};

/// Teaches the core key parser this machine's layout names.
pub fn install() {
    crate::core::keys::set_layout_hooks(name_for, vk_and_shift_for);
}

/// Layout name for OEM keys, which the US table in core cannot spell correctly.
fn name_for(vk: u16) -> Option<String> {
    if !(0xBA..=0xE2).contains(&vk) {
        return None;
    }
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    if scan == 0 {
        return None;
    }
    let mut buf = [0u16; 64];
    let len = unsafe { GetKeyNameTextW((scan as i32) << 16, &mut buf) };
    if len <= 0 {
        return None;
    }
    let name = String::from_utf16_lossy(&buf[..len as usize]);
    let name = name.trim().to_lowercase();
    (!name.is_empty()).then_some(name)
}

/// Virtual key and prerequisite shift state for a character this layout can type.
fn vk_and_shift_for(name: &str) -> Option<(u16, bool)> {
    let mut chars = name.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_ascii() || ch as u32 > 0xFFFF {
        return None;
    }
    let scan = unsafe { VkKeyScanW(ch as u16) };
    if scan == -1 {
        return None;
    }
    let raw = scan as u16;
    let vk = raw & 0xFF;
    let shift = (raw >> 8) & 1 != 0;
    (vk != 0).then_some((vk, shift))
}
