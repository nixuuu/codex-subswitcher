//! Native window opacity includes the NSVisualEffectView behind the GPUI surface.
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub fn opacity(window: &Window, value: f32, closing: bool) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // The borrowed GPUI handle keeps the NSView alive for this main-thread call.
    // Its owning window is neither retained nor cached past the call.
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<objc2::runtime::AnyObject>();
        let native: *mut objc2::runtime::AnyObject = objc2::msg_send![view, window];
        if !native.is_null() {
            let _: () = objc2::msg_send![native, setAlphaValue: f64::from(value.clamp(0., 1.))];
            let _: () = objc2::msg_send![native, setIgnoresMouseEvents: closing];
        }
    }
}
