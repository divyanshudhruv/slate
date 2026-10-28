use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HALFTONE, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetForegroundWindow, IsIconic};

const MAX_W: i32 = 640;

const PW_CLIENTONLY: u32 = 1;
const PW_RENDERFULLCONTENT: u32 = 2;

// `PrintWindow` sits behind the crate's XPS feature, so declare it locally.
#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hwnd: HWND, hdc: HDC, flags: u32) -> i32;
}

pub struct WindowSnapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn capture_window(hwnd: isize) -> Option<WindowSnapshot> {
    if hwnd == 0 {
        return None;
    }
    for _ in 0..2 {
        if let Some(snapshot) = unsafe { grab_once(hwnd) } {
            return Some(snapshot);
        }
        POOL.with(|pool| *pool.borrow_mut() = None);
    }
    None
}

/// Swaps the BGR channel order Windows hands back for RGBA.
pub fn bgra_to_rgba(mut bytes: Vec<u8>) -> Vec<u8> {
    for px in bytes.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    bytes
}

struct ScreenDc(HDC);
impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe { ReleaseDC(None, self.0) };
    }
}

struct MemDc(HDC);
impl Drop for MemDc {
    fn drop(&mut self) {
        let _ = unsafe { DeleteDC(self.0) };
    }
}

struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(self.0 .0)) };
        }
    }
}

struct Pooled {
    w: i32,
    h: i32,
    mem: MemDc,
    bmp: Bitmap,
}

thread_local! {
    static POOL: RefCell<Option<Pooled>> = const { RefCell::new(None) };
}

unsafe fn grab_once(hwnd: isize) -> Option<WindowSnapshot> {
    let h = HWND(hwnd as *mut _);
    if IsIconic(h).as_bool() {
        return None;
    }
    let mut rc = RECT::default();
    GetClientRect(h, &mut rc).ok()?;
    let (w, hgt) = (rc.right - rc.left, rc.bottom - rc.top);
    if w <= 0 || hgt <= 0 {
        return None;
    }

    let tw = w.clamp(1, MAX_W);
    let th = ((hgt as f32 * tw as f32 / w as f32).round() as i32).max(1);

    let screen = ScreenDc(GetDC(None));
    if screen.0.is_invalid() {
        return None;
    }
    let cached = POOL.with(|p| {
        let mut slot = p.borrow_mut();
        match slot.as_ref() {
            Some(existing) if existing.w == tw && existing.h == th => slot.take(),
            _ => {
                *slot = None;
                None
            }
        }
    });
    let pooled = match cached {
        Some(p) => p,
        None => {
            let mem = MemDc(CreateCompatibleDC(Some(screen.0)));
            if mem.0.is_invalid() {
                return None;
            }
            let bmp = Bitmap(CreateCompatibleBitmap(screen.0, tw, th));
            if bmp.0.is_invalid() {
                return None;
            }
            Pooled {
                w: tw,
                h: th,
                mem,
                bmp,
            }
        }
    };

    let old = SelectObject(pooled.mem.0, HGDIOBJ(pooled.bmp.0 .0));
    // The window draws itself first, so an occluded window never leaks pixels.
    let mut bgra = print_window(h, pooled.mem.0, pooled.bmp.0, w, hgt, tw, th);
    // Screen pixels are only trusted for the window the user is looking at.
    if bgra.is_none() && GetForegroundWindow() == h {
        let mut origin = POINT::default();
        if ClientToScreen(h, &mut origin).as_bool() {
            SetStretchBltMode(pooled.mem.0, HALFTONE);
            let drawn = StretchBlt(
                pooled.mem.0,
                0,
                0,
                tw,
                th,
                Some(screen.0),
                origin.x,
                origin.y,
                w,
                hgt,
                SRCCOPY,
            )
            .as_bool();
            if drawn {
                bgra = read_pixels(pooled.mem.0, pooled.bmp.0, tw, th).filter(|px| !is_flat(px));
            }
        }
    }
    SelectObject(pooled.mem.0, old);
    let bgra = bgra?;
    POOL.with(|p| *p.borrow_mut() = Some(pooled));

    Some(WindowSnapshot {
        width: tw as u32,
        height: th as u32,
        bgra,
    })
}

/// Draws the window itself into `mem` when the screen grab returns nothing usable.
unsafe fn print_window(
    hwnd: HWND,
    mem: HDC,
    mem_bmp: HBITMAP,
    w: i32,
    hgt: i32,
    tw: i32,
    th: i32,
) -> Option<Vec<u8>> {
    let src = MemDc(CreateCompatibleDC(Some(mem)));
    if src.0.is_invalid() {
        return None;
    }
    let bmp = Bitmap(CreateCompatibleBitmap(mem, w, hgt));
    if bmp.0.is_invalid() {
        return None;
    }
    let old = SelectObject(src.0, HGDIOBJ(bmp.0 .0));
    let printed = PrintWindow(hwnd, src.0, PW_CLIENTONLY | PW_RENDERFULLCONTENT) != 0;
    SetStretchBltMode(mem, HALFTONE);
    let drawn =
        printed && StretchBlt(mem, 0, 0, tw, th, Some(src.0), 0, 0, w, hgt, SRCCOPY).as_bool();
    SelectObject(src.0, old);
    drawn
        .then(|| read_pixels(mem, mem_bmp, tw, th))
        .flatten()
        .filter(|pixels| !is_flat(pixels))
}

/// A frame of one repeated pixel means the grab caught nothing usable.
fn is_flat(bgra: &[u8]) -> bool {
    let Some((first, rest)) = bgra.as_chunks::<4>().0.split_first() else {
        return true;
    };
    // Also treat very dark/empty frames as unusable to avoid black patches
    let mut nonzero = 0;
    for px in bgra.as_chunks::<4>().0 {
        if px[0] != 0 || px[1] != 0 || px[2] != 0 {
            nonzero += 1;
            if nonzero > 16 {
                break;
            }
        }
    }
    if nonzero <= 4 {
        return true;
    }
    rest.iter().all(|px| px == first)
}

unsafe fn read_pixels(mem: HDC, bmp: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    let rows = GetDIBits(
        mem,
        bmp,
        0,
        height as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        &mut info,
        DIB_RGB_COLORS,
    );
    if rows == 0 {
        return None;
    }
    let (chunks, _) = buf.as_chunks_mut::<4>();
    for px in chunks {
        px[3] = 0xFF;
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::bgra_to_rgba;

    #[test]
    fn bgra_becomes_rgba() {
        assert_eq!(
            bgra_to_rgba(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            vec![3, 2, 1, 4, 7, 6, 5, 8]
        );
    }
}
