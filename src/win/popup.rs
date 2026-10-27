use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_HWNDPARENT,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
};

fn hwnd(id: isize) -> Option<HWND> {
    (id != 0).then_some(HWND(id as *mut _))
}

/// Owner the popup to the host so it dies with it.
pub fn attach(popup: isize, host: isize) {
    let (Some(popup), Some(host)) = (hwnd(popup), hwnd(host)) else {
        return;
    };
    unsafe {
        let _ = SetWindowLongPtrW(popup, GWL_HWNDPARENT, host.0 as isize);
    }
}

/// Focus-less and mouse-transparent, so a parked popup cannot swallow clicks.
pub fn passive(popup: isize) {
    let Some(popup) = hwnd(popup) else {
        return;
    };
    set_click_through(popup, true);
}

fn set_click_through(popup: HWND, through: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(popup, GWL_EXSTYLE);
        let mut bits = ex | WS_EX_NOACTIVATE.0 as isize;
        if through {
            bits |= WS_EX_TRANSPARENT.0 as isize;
        } else {
            bits &= !(WS_EX_TRANSPARENT.0 as isize);
        }
        let _ = SetWindowLongPtrW(popup, GWL_EXSTYLE, bits);
    }
}

/// Screen origin, physical client size, and DPI scale of the host window.
pub fn host_metrics(host: isize) -> Option<(i32, i32, i32, i32, f32)> {
    let host = hwnd(host)?;
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    unsafe {
        GetClientRect(host, &mut rect).ok()?;
        if !ClientToScreen(host, &mut origin).as_bool() {
            return None;
        }
        let dpi = GetDpiForWindow(host);
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        Some((
            origin.x,
            origin.y,
            rect.right - rect.left,
            rect.bottom - rect.top,
            scale,
        ))
    }
}

/// Place the popup in physical screen pixels, above every member window.
pub fn place(popup: isize, x: i32, y: i32, w: i32, h: i32) {
    let Some(popup) = hwnd(popup) else {
        return;
    };
    set_click_through(popup, false);
    unsafe {
        let _ = SetWindowPos(
            popup,
            Some(HWND_TOPMOST),
            x,
            y,
            w.max(1),
            h.max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Park the popup offscreen. The size stays untouched, since resizing rebuilds
/// the swap chain and flashes the cleared surface.
pub fn park(popup: isize) {
    let Some(popup) = hwnd(popup) else {
        return;
    };
    set_click_through(popup, true);
    unsafe {
        let _ = SetWindowPos(
            popup,
            Some(HWND_TOPMOST),
            -32000,
            -32000,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE,
        );
    }
}
