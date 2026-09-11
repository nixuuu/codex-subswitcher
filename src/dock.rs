#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn set_policy(policy: cocoa::appkit::NSApplicationActivationPolicy) {
    use cocoa::{appkit::NSApplication, base::nil};

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        let _ = app.setActivationPolicy_(policy);
    }
}

#[cfg(target_os = "macos")]
pub fn show() {
    set_policy(cocoa::appkit::NSApplicationActivationPolicyRegular);
}

#[cfg(target_os = "macos")]
pub fn hide() {
    set_policy(cocoa::appkit::NSApplicationActivationPolicyAccessory);
}

#[cfg(not(target_os = "macos"))]
pub fn show() {}

#[cfg(not(target_os = "macos"))]
pub fn hide() {}
