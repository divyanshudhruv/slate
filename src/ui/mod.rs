pub mod dialog_window;
pub mod host_window;
pub mod tab_bar;

pub use host_window::HostWindow;

use std::sync::Arc;

use gpui::{RenderImage, Window};

pub fn render_image(rgba: image::RgbaImage) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]))
}

pub(crate) fn window_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}
