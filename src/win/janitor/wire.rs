use sha2::{Digest, Sha256};

use crate::win::host::StylingState;

const MAGIC: u32 = 0x534C_4A33;
const TAG_LEN: usize = 32;
const PLACEMENT_LEN: usize =
    std::mem::size_of::<windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT>();

pub fn encode(key: &[u8; 32], slate_hwnd: isize, members: &[(isize, StylingState)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + members.len() * (5 * 8 + 16 + PLACEMENT_LEN) + TAG_LEN);
    b.extend_from_slice(&MAGIC.to_le_bytes());
    b.extend_from_slice(&(slate_hwnd as i64).to_le_bytes());
    b.extend_from_slice(&(members.len() as u32).to_le_bytes());
    for (hwnd, st) in members {
        b.extend_from_slice(&(*hwnd as i64).to_le_bytes());
        b.extend_from_slice(&(st.style as i64).to_le_bytes());
        b.extend_from_slice(&(st.exstyle as i64).to_le_bytes());
        b.extend_from_slice(&(st.owner as i64).to_le_bytes());
        let (l, t, r, bo) = st.rect;
        for v in [l, t, r, bo] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        let raw = unsafe {
            std::slice::from_raw_parts(&st.placement as *const _ as *const u8, PLACEMENT_LEN)
        };
        b.extend_from_slice(raw);
    }
    let tag = hmac_sha256(key, &b);
    b.extend_from_slice(&tag);
    b
}

pub fn decode(key: &[u8; 32], bytes: &[u8]) -> Option<(isize, Vec<(isize, StylingState)>)> {
    if bytes.len() < 16 + TAG_LEN {
        return None;
    }
    let split = bytes.len() - TAG_LEN;
    let (body, tag) = bytes.split_at(split);
    if hmac_sha256(key, body) != tag {
        return None;
    }
    if u32::from_le_bytes(body[0..4].try_into().ok()?) != MAGIC {
        return None;
    }
    let slate_hwnd = i64::from_le_bytes(body[4..12].try_into().ok()?) as isize;
    let count = u32::from_le_bytes(body[12..16].try_into().ok()?) as usize;
    const PER_MEMBER: usize = 4 * 8 + 16 + PLACEMENT_LEN;
    if count > body.len().saturating_sub(16) / PER_MEMBER {
        return None;
    }
    let mut off = 16;
    let mut members = Vec::with_capacity(count);
    for _ in 0..count {
        let hwnd = i64::from_le_bytes(body.get(off..off + 8)?.try_into().ok()?) as isize;
        off += 8;
        let style = i64::from_le_bytes(body.get(off..off + 8)?.try_into().ok()?) as isize;
        off += 8;
        let exstyle = i64::from_le_bytes(body.get(off..off + 8)?.try_into().ok()?) as isize;
        off += 8;
        let owner = i64::from_le_bytes(body.get(off..off + 8)?.try_into().ok()?) as isize;
        off += 8;
        let rd = |o: usize| -> Option<i32> {
            Some(i32::from_le_bytes(body.get(o..o + 4)?.try_into().ok()?))
        };
        let (l, t, r, bo) = (rd(off)?, rd(off + 4)?, rd(off + 8)?, rd(off + 12)?);
        off += 16;
        let mut placement: windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT =
            Default::default();
        let src = body.get(off..off + PLACEMENT_LEN)?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                &mut placement as *mut _ as *mut u8,
                PLACEMENT_LEN,
            );
        }
        off += PLACEMENT_LEN;
        members.push((
            hwnd,
            StylingState {
                style,
                exstyle,
                owner,
                rect: (l, t, r, bo),
                placement,
            },
        ));
    }
    Some((slate_hwnd, members))
}

fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; TAG_LEN] {
    const BLOCK: usize = 64;
    let mut inner = [0x36u8; BLOCK];
    let mut outer = [0x5cu8; BLOCK];
    for (i, byte) in key.iter().enumerate() {
        inner[i] ^= byte;
        outer[i] ^= byte;
    }
    let mut hasher = Sha256::new();
    hasher.update(inner);
    hasher.update(message);
    let inner = hasher.finalize();
    let mut hasher = Sha256::new();
    hasher.update(outer);
    hasher.update(inner);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win::host::StylingState;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT;

    const KEY: [u8; 32] = [7; 32];

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
        let bytes = encode(&KEY, 0x1234, &[(0xABCD, sample())]);
        let (host, members) = decode(&KEY, &bytes).expect("decode");
        assert_eq!(host, 0x1234);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].0, 0xABCD);
        assert_eq!(encode(&KEY, host, &members), bytes);
    }

    #[test]
    fn empty_members_round_trip() {
        let bytes = encode(&KEY, 0x55, &[]);
        let (host, members) = decode(&KEY, &bytes).expect("decode");
        assert_eq!(host, 0x55);
        assert!(members.is_empty());
    }

    #[test]
    fn bad_magic_rejected() {
        assert!(decode(&KEY, &[0u8; 16]).is_none());
    }

    #[test]
    fn absurd_count_is_rejected_without_allocating() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.extend_from_slice(&0i64.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let tag = hmac_sha256(&KEY, &bytes);
        bytes.extend_from_slice(&tag);
        assert!(decode(&KEY, &bytes).is_none());
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let mut bytes = encode(&KEY, 0x1234, &[(0xABCD, sample())]);
        let last = bytes.len() - TAG_LEN - 1;
        bytes[last] ^= 0xFF;
        assert!(decode(&KEY, &bytes).is_none());
    }

    #[test]
    fn wrong_key_is_rejected() {
        let bytes = encode(&KEY, 0x1234, &[]);
        assert!(decode(&[9; 32], &bytes).is_none());
    }
}
