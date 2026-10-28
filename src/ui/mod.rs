pub mod dialog_window;
pub mod host_window;
pub(crate) mod metrics;
pub mod tab_bar;

pub use host_window::HostWindow;

use std::sync::Arc;

use gpui::{RenderImage, Window};

/// Wraps raw windows pixels as a gpui image; gpui reads the buffer as BGRA, which is what Win32 gives us.
pub fn render_image(bgra: image::RgbaImage) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(bgra)]))
}

pub(crate) fn window_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}
