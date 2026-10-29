//! Crash recovery state as plain JSON, so the file is readable and neither a key nor a signature is needed.

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS};

use crate::win::host::StylingState;

#[derive(Serialize, Deserialize)]
struct Recovery {
    host: isize,
    members: Vec<Member>,
}

#[derive(Serialize, Deserialize)]
struct Member {
    hwnd: isize,
    style: isize,
    exstyle: isize,
    owner: isize,
    rect: [i32; 4],
    flags: u32,
    show_cmd: u32,
    min: [i32; 2],
    max: [i32; 2],
    normal: [i32; 4],
}

impl Member {
    fn encode(hwnd: isize, st: &StylingState) -> Self {
        let placement = &st.placement;
        Self {
            hwnd,
            style: st.style,
            exstyle: st.exstyle,
            owner: st.owner,
            rect: [st.rect.0, st.rect.1, st.rect.2, st.rect.3],
            flags: placement.flags.0,
            show_cmd: placement.showCmd,
            min: [placement.ptMinPosition.x, placement.ptMinPosition.y],
            max: [placement.ptMaxPosition.x, placement.ptMaxPosition.y],
            normal: [
                placement.rcNormalPosition.left,
                placement.rcNormalPosition.top,
                placement.rcNormalPosition.right,
                placement.rcNormalPosition.bottom,
            ],
        }
    }

    fn decode(&self) -> StylingState {
        let placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            flags: WINDOWPLACEMENT_FLAGS(self.flags),
            showCmd: self.show_cmd,
            ptMinPosition: POINT {
                x: self.min[0],
                y: self.min[1],
            },
            ptMaxPosition: POINT {
                x: self.max[0],
                y: self.max[1],
            },
            rcNormalPosition: RECT {
                left: self.normal[0],
                top: self.normal[1],
                right: self.normal[2],
                bottom: self.normal[3],
            },
        };
        StylingState {
            style: self.style,
            exstyle: self.exstyle,
            owner: self.owner,
            rect: (self.rect[0], self.rect[1], self.rect[2], self.rect[3]),
            placement,
        }
    }
}

pub fn encode(slate_hwnd: isize, members: &[(isize, StylingState)]) -> Option<Vec<u8>> {
    let payload = Recovery {
        host: slate_hwnd,
        members: members
            .iter()
            .map(|(hwnd, st)| Member::encode(*hwnd, st))
            .collect(),
    };
    serde_json::to_vec(&payload).ok()
}

pub fn decode(bytes: &[u8]) -> Option<(isize, Vec<(isize, StylingState)>)> {
    let payload: Recovery = serde_json::from_slice(bytes).ok()?;
    let members = payload
        .members
        .iter()
        .map(|member| (member.hwnd, member.decode()))
        .collect();
    Some((payload.host, members))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT;

    fn sample() -> StylingState {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        placement.showCmd = 3;
        placement.rcNormalPosition = RECT {
            left: 10,
            top: 20,
            right: 300,
            bottom: 400,
        };
        StylingState {
            style: 0x16CF_0000,
            exstyle: 0x100,
            owner: 0x99,
            rect: (10, 20, 300, 400),
            placement,
        }
    }

    #[test]
    fn round_trips_state() {
        let bytes = encode(0x1234, &[(0xABCD, sample())]).expect("encode");
        let (host, members) = decode(&bytes).expect("decode");
        assert_eq!(host, 0x1234);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].0, 0xABCD);
        assert_eq!(members[0].1.style, 0x16CF_0000);
        assert_eq!(members[0].1.placement.rcNormalPosition.right, 300);
        assert_eq!(encode(host, &members).as_deref(), Some(bytes.as_slice()));
    }

    #[test]
    fn empty_members_round_trip() {
        let bytes = encode(0x55, &[]).expect("encode");
        let (host, members) = decode(&bytes).expect("decode");
        assert_eq!(host, 0x55);
        assert!(members.is_empty());
    }

    #[test]
    fn the_payload_is_readable_json() {
        let bytes = encode(0x55, &[(1, sample())]).expect("encode");
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.contains("\"host\":"));
        assert!(text.contains("\"members\":"));
    }

    #[test]
    fn garbage_and_truncation_are_rejected() {
        assert!(decode(b"").is_none());
        assert!(decode(&[0u8; 16]).is_none());
        let bytes = encode(0x1234, &[(0xABCD, sample())]).expect("encode");
        assert!(decode(&bytes[..bytes.len() / 2]).is_none());
    }
}
