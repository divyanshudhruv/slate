use windows::Win32::Foundation::{GetLastError, SetLastError, HWND, RECT, WIN32_ERROR};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMNCRP_USEWINDOWSTYLE, DWMWA_NCRENDERING_POLICY,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DEFAULT, DWMWCP_DONOTROUND,
};
use windows::Win32::Graphics::Gdi::SetWindowRgn;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, IsHungAppWindow, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, GWL_HWNDPARENT, GWL_STYLE, HWND_TOP, SET_WINDOW_POS_FLAGS, SWP_ASYNCWINDOWPOS,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE,
    SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWMINNOACTIVE, SW_SHOWNA, SW_SHOWNORMAL,
    WINDOWPLACEMENT, WINDOW_LONG_PTR_INDEX, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
};

const FRAME: isize = WS_CAPTION.0 as isize
    | WS_THICKFRAME.0 as isize
    | WS_SYSMENU.0 as isize
    | WS_MINIMIZEBOX.0 as isize
    | WS_MAXIMIZEBOX.0 as isize;

#[derive(Clone, Debug)]
pub struct StylingState {
    pub style: isize,
    pub exstyle: isize,
    pub owner: isize,
    pub rect: (i32, i32, i32, i32),
    pub placement: WINDOWPLACEMENT,
}

pub(super) fn set_long_ptr(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> bool {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let prev = SetWindowLongPtrW(hwnd, index, value);
        prev != 0 || GetLastError() == WIN32_ERROR(0)
    }
}

pub fn register_member(host: isize, member: isize) -> Option<StylingState> {
    if host == 0 || member == 0 {
        return None;
    }
    let mh = HWND(member as *mut _);
    unsafe {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(mh, &mut placement).is_err() {
            return None;
        }
        let mut r = RECT::default();
        if GetWindowRect(mh, &mut r).is_err() {
            return None;
        }
        let style = GetWindowLongPtrW(mh, GWL_STYLE);
        let exstyle = GetWindowLongPtrW(mh, GWL_EXSTYLE);
        let owner = GetWindowLongPtrW(mh, GWL_HWNDPARENT);
        // Probe the style writes before the owner changes, so a refusal leaves the window untouched.
        if !set_long_ptr(mh, GWL_STYLE, style) || !set_long_ptr(mh, GWL_EXSTYLE, exstyle) {
            return None;
        }
        if !set_long_ptr(mh, GWL_HWNDPARENT, host) {
            return None;
        }
        let hosted_ex = exstyle & !(WS_EX_APPWINDOW.0 as isize);
        let _ = set_long_ptr(mh, GWL_EXSTYLE, hosted_ex);
        let _ = set_long_ptr(mh, GWL_STYLE, style & !FRAME);
        set_frame_treatment(mh, true);
        if IsIconic(mh).as_bool() || IsZoomed(mh).as_bool() {
            let _ = ShowWindow(mh, SW_RESTORE);
        }
        let _ = ShowWindow(mh, SW_HIDE);
        let _ = SetWindowPos(
            mh,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS,
        );
        Some(StylingState {
            style,
            exstyle,
            owner,
            rect: (r.left, r.top, r.right, r.bottom),
            placement,
        })
    }
}

fn set_frame_treatment(mh: HWND, hosted: bool) {
    let (corner, ncrp) = if hosted {
        (DWMWCP_DONOTROUND, DWMNCRP_DISABLED)
    } else {
        (DWMWCP_DEFAULT, DWMNCRP_USEWINDOWSTYLE)
    };
    unsafe {
        let _ = DwmSetWindowAttribute(
            mh,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<windows::Win32::Graphics::Dwm::DWM_WINDOW_CORNER_PREFERENCE>()
                as u32,
        );
        let _ = DwmSetWindowAttribute(
            mh,
            DWMWA_NCRENDERING_POLICY,
            &ncrp as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<windows::Win32::Graphics::Dwm::DWMNCRENDERINGPOLICY>() as u32,
        );
    }
}

/// Style-only flags; a hung target gets the asynchronous variant so restore never blocks on it.
fn frame_flags(mh: HWND) -> SET_WINDOW_POS_FLAGS {
    let base = SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE;
    if unsafe { IsHungAppWindow(mh).as_bool() } {
        base | SWP_ASYNCWINDOWPOS
    } else {
        base
    }
}

/// Gives a member back to the desktop. Returns false when the window outlived the writes, so a
/// crash-recovery replay can tell a partial restore from a clean one.
pub fn unregister_member(member: isize, st: &StylingState) -> bool {
    if member == 0 {
        return false;
    }
    let mh = HWND(member as *mut _);
    unsafe {
        if !IsWindow(Some(mh)).as_bool() {
            return true;
        }
        let minimized = IsIconic(mh).as_bool() || st.placement.showCmd as i32 == SW_SHOWMINIMIZED.0;
        let maximized = st.placement.showCmd as i32 == SW_SHOWMAXIMIZED.0;
        let _ = ShowWindow(mh, SW_HIDE);
        let _ = SetWindowRgn(mh, None, true);
        let mut ok = set_long_ptr(mh, GWL_HWNDPARENT, st.owner);
        ok &= set_long_ptr(mh, GWL_STYLE, st.style);
        ok &= set_long_ptr(mh, GWL_EXSTYLE, st.exstyle);
        let _ = SetWindowPos(mh, None, 0, 0, 0, 0, frame_flags(mh));
        set_frame_treatment(mh, false);
        ok &= SetWindowPlacement(mh, &st.placement).is_ok();
        // A maximized or minimized placement already carries its geometry; moving it again would rewrite the restored bounds.
        if !minimized && !maximized {
            let (l, t, r, b) = st.rect;
            let w = (r - l).max(0);
            let h = (b - t).max(0);
            let mut flags = SWP_FRAMECHANGED | SWP_NOACTIVATE;
            if IsHungAppWindow(mh).as_bool() {
                flags |= SWP_ASYNCWINDOWPOS;
            }
            let _ = SetWindowPos(mh, Some(HWND_TOP), l, t, w, h, flags);
        }
        let _ = ShowWindow(
            mh,
            if minimized {
                SW_SHOWMINNOACTIVE
            } else {
                SW_SHOWNA
            },
        );
        // Some apps ignore the no-activate show after a frame change; make sure a normal window is really back on screen.
        if !minimized && !IsWindowVisible(mh).as_bool() {
            let _ = ShowWindow(mh, SW_SHOWNORMAL);
        }
        // The shell only notices a window is taskbar-eligible once it is visible, so the button is refreshed last.
        let wants_button = st.owner == 0 || st.exstyle & WS_EX_APPWINDOW.0 as isize != 0;
        let excludes_button =
            st.exstyle & (WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize) != 0;
        if wants_button && !excludes_button {
            refresh_taskbar_button(mh);
        }
        ok
    }
}

/// Asks the shell to rebuild this window's taskbar button. Ownership and style writes on a hosted
/// window leave the shell's cache stale, so windows restored behind the focused one never get a
/// button until they are clicked. `ITaskbarList` is the supported way to force that rebuild.
fn refresh_taskbar_button(mh: HWND) {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::ITaskbarList;
    const CLSID_TASKBAR_LIST: windows::core::GUID =
        windows::core::GUID::from_u128(0x56fdf344_fd6d_11d0_958a_006097c9a090);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(list) =
            CoCreateInstance::<_, ITaskbarList>(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER)
        else {
            return;
        };
        if list.HrInit().is_err() {
            return;
        }
        let _ = list.DeleteTab(mh);
        let _ = list.AddTab(mh);
    }
}
