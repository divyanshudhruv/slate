use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub vk: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)]
pub enum KeyAction {
    NextTab,
    PrevTab,
    CloseTab,
    /// Template action: the chord's modifiers select the Nth tab for every digit key.
    SelectTab,
}

pub const ACTIONS: [KeyAction; 4] = [
    KeyAction::NextTab,
    KeyAction::PrevTab,
    KeyAction::CloseTab,
    KeyAction::SelectTab,
];

const VK_TAB: u16 = 0x09;
const VK_ESCAPE: u16 = 0x1B;
const VK_SPACE: u16 = 0x20;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_DIGIT0: u16 = 0x30;
const VK_KEY_A: u16 = 0x41;
const VK_F1: u16 = 0x70;
const VK_F24: u16 = 0x87;

impl KeyAction {
    pub fn label(self) -> &'static str {
        match self {
            KeyAction::NextTab => "Next tab",
            KeyAction::PrevTab => "Previous tab",
            KeyAction::CloseTab => "Close tab",
            KeyAction::SelectTab => "Go to tab",
        }
    }

    pub fn default(self) -> Chord {
        match self {
            KeyAction::NextTab => Chord {
                ctrl: true,
                shift: false,
                alt: false,
                win: false,
                vk: VK_TAB,
            },
            KeyAction::PrevTab => Chord {
                ctrl: true,
                shift: true,
                alt: false,
                win: false,
                vk: VK_TAB,
            },
            KeyAction::CloseTab => Chord {
                ctrl: true,
                shift: false,
                alt: false,
                win: false,
                vk: 0x57,
            },
            KeyAction::SelectTab => Chord {
                ctrl: true,
                shift: false,
                alt: false,
                win: false,
                vk: VK_DIGIT0 + 1,
            },
        }
    }
}

const VK_OEM_1: u16 = 0xBA;
const VK_OEM_PLUS: u16 = 0xBB;
const VK_OEM_COMMA: u16 = 0xBC;
const VK_OEM_MINUS: u16 = 0xBD;
const VK_OEM_PERIOD: u16 = 0xBE;
const VK_OEM_2: u16 = 0xBF;
const VK_OEM_3: u16 = 0xC0;
const VK_OEM_4: u16 = 0xDB;
const VK_OEM_5: u16 = 0xDC;
const VK_OEM_6: u16 = 0xDD;
const VK_OEM_7: u16 = 0xDE;

/// The virtual-key code a key name means, or none when Slate cannot bind it.
pub fn vk_for(name: &str) -> Option<u16> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "tab" => return Some(VK_TAB),
        "space" => return Some(VK_SPACE),
        "left" => return Some(VK_LEFT),
        "up" => return Some(VK_UP),
        "right" => return Some(VK_RIGHT),
        "down" => return Some(VK_DOWN),
        // shifted digits
        "!" => return Some(VK_DIGIT0 + 1),
        "@" => return Some(VK_DIGIT0 + 2),
        "#" => return Some(VK_DIGIT0 + 3),
        "$" => return Some(VK_DIGIT0 + 4),
        "%" => return Some(VK_DIGIT0 + 5),
        "^" => return Some(VK_DIGIT0 + 6),
        "&" => return Some(VK_DIGIT0 + 7),
        "*" => return Some(VK_DIGIT0 + 8),
        "(" => return Some(VK_DIGIT0 + 9),
        ")" => return Some(VK_DIGIT0),
        // shifted symbols
        ":" | ";" => return Some(VK_OEM_1),
        "+" | "=" => return Some(VK_OEM_PLUS),
        "<" | "," => return Some(VK_OEM_COMMA),
        "_" | "-" => return Some(VK_OEM_MINUS),
        ">" | "." => return Some(VK_OEM_PERIOD),
        "?" | "/" => return Some(VK_OEM_2),
        "~" | "`" => return Some(VK_OEM_3),
        "{" | "[" => return Some(VK_OEM_4),
        "|" | "\\" => return Some(VK_OEM_5),
        "}" | "]" => return Some(VK_OEM_6),
        "\"" | "'" => return Some(VK_OEM_7),
        _ => {}
    }
    if name.len() == 1 {
        let bytes = name.as_bytes();
        let c = bytes[0] as char;
        if c.is_ascii_lowercase() {
            return Some(VK_KEY_A + (c as u16) - ('a' as u16));
        }
        if c.is_ascii_uppercase() {
            let lower_c = c.to_ascii_lowercase();
            return Some(VK_KEY_A + (lower_c as u16) - ('a' as u16));
        }
        if c.is_ascii_digit() {
            return Some(VK_DIGIT0 + (c as u16) - ('0' as u16));
        }
        return None;
    }
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(index) = rest.parse::<u16>() {
            if (1..=24).contains(&index) && rest.len() == index.to_string().len() {
                return Some(VK_F1 + index - 1);
            }
        }
        return None;
    }
    None
}

/// The name a virtual-key code is written as, or none when it has no printable spelling.
pub fn name_for(vk: u16) -> Option<String> {
    if vk == VK_ESCAPE || vk == VK_F24 + 1 {
        return None;
    }
    match vk {
        VK_TAB => return Some("tab".to_string()),
        VK_SPACE => return Some("space".to_string()),
        VK_LEFT => return Some("left".to_string()),
        VK_UP => return Some("up".to_string()),
        VK_RIGHT => return Some("right".to_string()),
        VK_DOWN => return Some("down".to_string()),
        _ => {}
    }
    if (VK_DIGIT0..VK_DIGIT0 + 10).contains(&vk) {
        return Some(char::from_digit((vk - VK_DIGIT0) as u32, 10)?.to_string());
    }
    if (VK_KEY_A..VK_KEY_A + 26).contains(&vk) {
        return Some((char::from_u32(('a' as u32) + (vk - VK_KEY_A) as u32)?).to_string());
    }
    if (VK_F1..=VK_F24).contains(&vk) {
        return Some(format!("f{}", vk - VK_F1 + 1));
    }
    None
}

/// True when the chord carries at least one unshifted modifier, so a bare key never qualifies.
pub fn chord_valid(chord: Chord) -> bool {
    (chord.ctrl || chord.alt || chord.win) && name_for(chord.vk).is_some()
}

/// Parses a settings string like "ctrl+shift+tab".
pub fn parse(text: &str) -> Result<Chord, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("keybind is empty".to_string());
    }
    let mut chord = Chord {
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
        vk: 0,
    };
    for token in text.split('+') {
        let token = token.trim().to_ascii_lowercase();
        if token.is_empty() {
            return Err(format!("bad keybind '{text}'"));
        }
        match token.as_str() {
            "ctrl" | "control" => {
                if chord.ctrl {
                    return Err(format!("bad keybind '{text}'"));
                }
                chord.ctrl = true;
            }
            "shift" => {
                if chord.shift {
                    return Err(format!("bad keybind '{text}'"));
                }
                chord.shift = true;
            }
            "alt" => {
                if chord.alt {
                    return Err(format!("bad keybind '{text}'"));
                }
                chord.alt = true;
            }
            "win" => {
                if chord.win {
                    return Err(format!("bad keybind '{text}'"));
                }
                chord.win = true;
            }
            _ => {
                if chord.vk != 0 {
                    return Err(format!("bad keybind '{text}'"));
                }
                chord.vk = vk_for(&token).ok_or_else(|| format!("unknown key '{token}'"))?;
            }
        }
    }
    if chord.vk == 0 {
        return Err(format!("keybind '{text}' has no key"));
    }
    if !chord_valid(chord) {
        return Err(format!("keybind '{text}' needs ctrl, alt, or win"));
    }
    Ok(chord)
}

/// Writes a chord as a settings string like "ctrl+shift+tab", or none when Slate cannot spell it.
pub fn format_chord(chord: Chord) -> Option<String> {
    if !chord_valid(chord) {
        return None;
    }
    let mut parts = Vec::with_capacity(5);
    if chord.ctrl {
        parts.push("ctrl");
    }
    if chord.shift {
        parts.push("shift");
    }
    if chord.alt {
        parts.push("alt");
    }
    if chord.win {
        parts.push("win");
    }
    let key = name_for(chord.vk)?;
    parts.push(key.as_str());
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> [Chord; 4] {
        ACTIONS.map(|action| action.default())
    }

    #[test]
    fn actions_carry_labels_and_defaults() {
        assert_eq!(KeyAction::NextTab.label(), "Next tab");
        assert_eq!(KeyAction::SelectTab.label(), "Go to tab");
        assert_eq!(KeyAction::PrevTab.default().vk, VK_TAB);
        assert!(KeyAction::PrevTab.default().shift);
        assert_eq!(KeyAction::SelectTab.default().vk, VK_DIGIT0 + 1);
        assert!(defaults()[0].ctrl);
        assert_eq!(defaults()[2].vk, 0x57);
    }

    #[test]
    fn parse_formats_round_trip() {
        let chord = Chord {
            ctrl: true,
            shift: true,
            alt: false,
            win: false,
            vk: VK_TAB,
        };
        let text = format_chord(chord).unwrap();
        assert_eq!(text, "ctrl+shift+tab");
        assert_eq!(parse(&text).unwrap(), chord);
        for action in ACTIONS {
            let default = action.default();
            assert_eq!(parse(&format_chord(default).unwrap()), Ok(default));
        }
    }

    #[test]
    fn parse_accepts_aliases_and_order() {
        assert_eq!(
            parse("shift + ctrl + tab").unwrap(),
            KeyAction::PrevTab.default()
        );
        assert_eq!(parse("control+w").unwrap(), KeyAction::CloseTab.default());
        assert!(parse("CTRL+1").is_ok());
    }

    #[test]
    fn parse_rejects_modifier_less_and_unknown_keys() {
        assert!(parse("w").is_err());
        assert!(parse("shift+w").is_err());
        assert!(parse("ctrl").is_err());
        assert!(parse("").is_err());
        assert!(parse("ctrl+").is_err());
        assert!(parse("ctrl+++w").is_err());
        assert!(parse("ctrl+capslock").is_err());
        assert!(parse("ctrl+w+w").is_err());
        assert!(parse("ctrl++").is_err());
    }

    #[test]
    fn escape_cannot_be_bound() {
        assert_eq!(vk_for("escape"), None);
        assert_eq!(name_for(VK_ESCAPE), None);
        assert!(parse("ctrl+escape").is_err());
    }

    #[test]
    fn key_names_cover_the_modifiers_note() {
        assert_eq!(vk_for("f12"), Some(VK_F1 + 12 - 1));
        assert_eq!(name_for(VK_F24), Some("f24".to_string()));
        assert_eq!(vk_for("0"), Some(VK_DIGIT0));
        assert_eq!(vk_for("9"), Some(VK_DIGIT0 + 9));
        assert_eq!(vk_for("W"), Some(0x57));
        assert_eq!(vk_for("a"), Some(VK_KEY_A));
        assert_eq!(vk_for("f25"), None);
        assert_eq!(vk_for("ctrl"), None);
        assert_eq!(name_for(VK_TAB), Some("tab".to_string()));
        assert!(name_for(0xFF).is_none());
    }

    #[test]
    fn format_rejects_unspellable_chords() {
        assert!(format_chord(Chord {
            ctrl: true,
            shift: false,
            alt: false,
            win: false,
            vk: 0xFF,
        })
        .is_none());
        assert!(format_chord(Chord {
            ctrl: false,
            shift: false,
            alt: false,
            win: false,
            vk: VK_TAB,
        })
        .is_none());
    }

    #[test]
    fn f_range_bounds() {
        assert!((VK_F1..=VK_F24).contains(&0x70));
        assert_eq!(VK_F24, 0x87);
    }
}
