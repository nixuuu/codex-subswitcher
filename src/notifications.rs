//! Request permission before GPUI posts anything; its macOS backend does not
//! wait for the authorization callback before scheduling a notification.
use gpui_kit::{App, Global, SharedString, SystemNotification};
use std::collections::HashMap;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Authorization {
    #[default]
    Checking,
    Granted,
    Denied,
    Unavailable,
    Failed,
    #[cfg(debug_assertions)]
    Demo,
}

#[derive(Default)]
struct Notifications {
    authorization: Authorization,
    pending: HashMap<SharedString, SystemNotification>,
}
impl Global for Notifications {}

impl Notifications {
    fn resolve(&mut self, authorization: Authorization) -> Vec<SystemNotification> {
        self.authorization = authorization;
        let pending = std::mem::take(&mut self.pending);
        if authorization == Authorization::Granted {
            pending.into_values().collect()
        } else {
            Vec::new()
        }
    }
}

pub fn install(cx: &mut App) {
    cx.set_global(Notifications::default());
    #[cfg(debug_assertions)]
    if crate::demo_mode() {
        cx.global_mut::<Notifications>().authorization = Authorization::Demo;
        return;
    }
    request(cx);
}

pub fn status(cx: &App) -> &'static str {
    match cx.global::<Notifications>().authorization {
        Authorization::Checking => "Notifications: waiting for macOS permission…",
        Authorization::Granted => "Notifications: permission granted",
        Authorization::Denied => {
            "Notifications are disabled. Enable them in System Settings → Notifications → Codex Sub Switcher."
        }
        Authorization::Unavailable => "Notifications require running Codex Sub Switcher.app.",
        Authorization::Failed => "Could not check notification permission. Try again.",
        #[cfg(debug_assertions)]
        Authorization::Demo => "Demo mode · system notifications disabled",
    }
}

pub fn can_test(cx: &App) -> bool {
    let authorization = cx.global::<Notifications>().authorization;
    #[cfg(debug_assertions)]
    if authorization == Authorization::Demo {
        return false;
    }
    authorization != Authorization::Checking
}

pub fn show(notification: SystemNotification, cx: &mut App) {
    let state = cx.global_mut::<Notifications>();
    #[cfg(debug_assertions)]
    if state.authorization == Authorization::Demo {
        return;
    }
    state.pending.insert(notification.tag.clone(), notification);
    if state.authorization != Authorization::Checking {
        // Recheck permission for each delivery, including after changes in Settings.
        request(cx);
    }
}

fn request(cx: &mut App) {
    cx.global_mut::<Notifications>().authorization = Authorization::Checking;
    cx.refresh_windows();
    cx.spawn(async move |cx| {
        let authorization = request_authorization().await;
        cx.update(|cx| {
            let pending = cx.global_mut::<Notifications>().resolve(authorization);
            for notification in pending {
                cx.show_system_notification(notification);
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

#[cfg(target_os = "macos")]
async fn request_authorization() -> Authorization {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSError};
    use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
    use std::sync::Mutex;

    // Calling currentNotificationCenter outside an app bundle can abort the process.
    if NSBundle::mainBundle().bundleIdentifier().is_none() {
        return Authorization::Unavailable;
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let completion = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        let authorization = if !error.is_null() {
            Authorization::Failed
        } else if granted.as_bool() {
            Authorization::Granted
        } else {
            Authorization::Denied
        };
        if let Some(sender) = sender.lock().unwrap().take() {
            let _ = sender.send(authorization);
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &completion,
        );
    receiver.await.unwrap_or(Authorization::Failed)
}

#[cfg(not(target_os = "macos"))]
async fn request_authorization() -> Authorization {
    Authorization::Unavailable
}

#[cfg(test)]
mod tests {
    use super::{Authorization, Notifications};
    use gpui_kit::SystemNotification;

    fn notification(tag: &str, body: &str) -> SystemNotification {
        SystemNotification {
            tag: tag.to_owned().into(),
            title: "Reset".into(),
            body: body.to_owned().into(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn permission_result_releases_queued_notifications_once() {
        let mut state = Notifications::default();
        let first = notification("account-a", "5h");
        let second = notification("account-b", "7d");
        state.pending.insert(first.tag.clone(), first.clone());
        state.pending.insert(second.tag.clone(), second.clone());
        assert_eq!(state.authorization, Authorization::Checking);
        let delivered = state.resolve(Authorization::Granted);
        assert_eq!(delivered.len(), 2);
        assert!(delivered.contains(&first));
        assert!(delivered.contains(&second));
        assert!(state.resolve(Authorization::Granted).is_empty());
    }

    #[test]
    fn denial_or_error_never_delivers_or_replays_old_notifications() {
        for authorization in [
            Authorization::Denied,
            Authorization::Unavailable,
            Authorization::Failed,
            #[cfg(debug_assertions)]
            Authorization::Demo,
        ] {
            let mut state = Notifications::default();
            let notification = notification("account-a", "5h");
            state.pending.insert(notification.tag.clone(), notification);
            assert!(state.resolve(authorization).is_empty());
            assert_eq!(state.authorization, authorization);
            assert!(state.resolve(Authorization::Granted).is_empty());
        }
    }
}
