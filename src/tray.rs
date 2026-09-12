//! The status item lives for the application lifetime, independently of window visibility.
use gpui_kit::*;
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

gpui_kit::actions!(switcher_tray, [Show, Hide, Quit]);

#[derive(Clone, PartialEq, Eq)]
pub struct Status {
    pub title: String,
    pub account: String,
    pub lines: Vec<String>,
}

pub struct Tray {
    _icon: TrayIcon,
    _events: Task<()>,
    actions: [MenuItem; 3],
    status: Option<Status>,
}
impl Global for Tray {}

pub fn show(cx: &mut App) {
    crate::dock::show();
    cx.activate(true);
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}

pub fn hide(cx: &mut App) {
    cx.hide();
    crate::dock::hide();
}

fn title_for(view: Option<&crate::UsageView>) -> String {
    let Some(data) = view
        .filter(|view| view.error.is_none())
        .and_then(|view| view.data.as_ref())
    else {
        return "—%".into();
    };
    let mut windows = data.windows().collect::<Vec<_>>();
    windows.sort_by_key(|window| window.limit_window_seconds);
    match windows.as_slice() {
        [] => "—%".into(),
        [window] => format!("{:.0}%", 100.0 - window.used_percent),
        _ => windows
            .iter()
            .map(|window| {
                let label = if window.limit_window_seconds == 604800 {
                    "7d".into()
                } else {
                    window.label()
                };
                format!("{label}: {:.0}%", 100.0 - window.used_percent)
            })
            .collect::<Vec<_>>()
            .join(" · "),
    }
}

pub(super) fn status_for(
    account: Option<&crate::accounts::Account>,
    view: Option<&crate::UsageView>,
    loading: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Status {
    let Some(account) = account else {
        return Status {
            title: "—%".into(),
            account: "Nie wybrano konta".into(),
            lines: vec!["Wybierz konto w oknie aplikacji".into()],
        };
    };
    let mut lines = Vec::new();
    if let Some(data) = view.and_then(|v| v.data.as_ref()) {
        for limit in data.windows() {
            lines.push(format!(
                "{}: {:.0}% pozostało",
                limit.label(),
                100.0 - limit.used_percent
            ));
            lines.push(limit.reset_label(now));
        }
        if data.windows().next().is_none() {
            lines.push("Brak udostępnionych okien limitów".into());
        }
        if let Some(count) = data
            .reset_details
            .as_ref()
            .map(|d| d.available_count)
            .or_else(|| {
                data.rate_limit_reset_credits
                    .as_ref()
                    .map(|d| d.available_count)
            })
        {
            lines.push(format!("Dostępne restarty: {count}"));
        }
    } else {
        lines.push(
            if loading {
                "Pobieranie limitów…"
            } else {
                "Limity niedostępne"
            }
            .into(),
        );
    }
    if view.is_some_and(|v| v.error.is_some()) {
        lines.push("Błąd odczytu · dane nieaktualne".into());
    }
    if loading {
        lines.push("Odświeżanie…".into());
    }
    if let Some(at) = view.and_then(|v| v.checked) {
        lines.push(format!(
            "Odczyt {} · co minutę",
            at.with_timezone(&chrono::Local).format("%H:%M")
        ));
    }
    Status {
        title: title_for(view),
        account: format!("{} · {}", account.email, account.plan),
        lines,
    }
}

pub fn update(status: Status, cx: &mut App) {
    if cx.try_global::<Tray>().is_none() {
        return;
    }
    let tray = cx.global_mut::<Tray>();
    if tray.status.as_ref() == Some(&status) {
        return;
    }
    let heading = MenuItem::new(&status.account, false, None);
    let lines = status
        .lines
        .iter()
        .map(|line| MenuItem::new(line, false, None))
        .collect::<Vec<_>>();
    let separator = PredefinedMenuItem::separator();
    let bottom = PredefinedMenuItem::separator();
    let mut items: Vec<&dyn tray_icon::menu::IsMenuItem> = vec![&heading];
    items.extend(lines.iter().map(|i| i as &dyn tray_icon::menu::IsMenuItem));
    items.extend([
        &separator as &dyn tray_icon::menu::IsMenuItem,
        &tray.actions[0],
        &tray.actions[1],
        &bottom,
        &tray.actions[2],
    ]);
    if let Ok(menu) = Menu::with_items(&items) {
        tray._icon.set_title(Some(&status.title));
        tray._icon.set_menu(Some(Box::new(menu)));
        let _ = tray._icon.set_tooltip(Some(format!(
            "Codex Sub Switcher\n{}\n{}",
            status.account,
            status.lines.join("\n")
        )));
        tray.status = Some(status);
    }
}

pub fn install(cx: &mut App) -> anyhow::Result<()> {
    let show = MenuItem::new("Pokaż okno", true, None);
    let hide = MenuItem::new("Schowaj okno", true, None);
    let quit = MenuItem::new("Zakończ Codex Sub Switcher", true, None);
    let menu = Menu::with_items(&[&show, &hide, &PredefinedMenuItem::separator(), &quit])?;
    let icon = TrayIconBuilder::new()
        .with_title("—%")
        .with_tooltip("Codex Sub Switcher")
        .with_menu(Box::new(menu))
        .build()?;
    let show_id = show.id().clone();
    let hide_id = hide.id().clone();
    let quit_id = quit.id().clone();
    let events = cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            // No repaint or App borrow while idle. Drain a bounded batch per tick.
            for event in MenuEvent::receiver().try_iter().take(32) {
                cx.update(|cx| {
                    if event.id == show_id {
                        self::show(cx);
                    } else if event.id == hide_id {
                        self::hide(cx);
                    } else if event.id == quit_id {
                        cx.quit();
                    }
                });
                if event.id == quit_id {
                    return;
                }
            }
        }
    });
    cx.on_action(|_: &Show, cx| self::show(cx));
    cx.on_action(|_: &Hide, cx| self::hide(cx));
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    cx.set_menus([gpui_kit::Menu::new("Codex Sub Switcher").items([
        gpui_kit::MenuItem::action("Pokaż okno", Show),
        gpui_kit::MenuItem::action("Schowaj okno", Hide),
        gpui_kit::MenuItem::separator(),
        gpui_kit::MenuItem::action("Zakończ Codex Sub Switcher", Quit),
    ])]);
    cx.set_global(Tray {
        _icon: icon,
        _events: events,
        actions: [show, hide, quit],
        status: None,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{status_for, title_for};

    fn view(windows: &[(i64, f32)]) -> crate::UsageView {
        let window = |index: usize| {
            windows.get(index).map(|&(seconds, used)| {
                serde_json::json!({"limit_window_seconds":seconds,"used_percent":used})
            })
        };
        let data = crate::usage::Usage::parse(
            &serde_json::to_vec(&serde_json::json!({"rate_limit":{
                "primary_window":window(0), "secondary_window":window(1)
            }}))
            .unwrap(),
        )
        .unwrap();
        crate::UsageView {
            data: Some(data),
            checked: Some(chrono::Utc::now()),
            error: None,
        }
    }

    #[test]
    fn title_shows_remaining_limits_in_duration_order_as_whole_percentages() {
        assert_eq!(
            title_for(Some(&view(&[(604800, 20.0), (18000, 60.0)]))),
            "5h: 40% · 7d: 80%"
        );
        assert_eq!(
            title_for(Some(&view(&[(18000, 100.0), (604800, 0.0)]))),
            "5h: 0% · 7d: 100%"
        );
        assert_eq!(
            title_for(Some(&view(&[(18000, 59.6), (604800, 20.4)]))),
            "5h: 40% · 7d: 80%"
        );
        assert_eq!(title_for(Some(&view(&[(604800, 9.0)]))), "91%");
    }

    #[test]
    fn title_does_not_invent_a_percentage_for_missing_or_failed_reads() {
        assert_eq!(title_for(None), "—%");
        assert_eq!(title_for(Some(&crate::UsageView::default())), "—%");
        assert_eq!(title_for(Some(&view(&[]))), "—%");
        let mut stale = view(&[(18000, 60.0)]);
        stale.error = Some("offline".into());
        assert_eq!(title_for(Some(&stale)), "—%");
    }

    #[test]
    fn tray_shows_only_reported_windows_and_marks_stale_values() {
        let account = crate::accounts::Account {
            id: "a".repeat(64),
            email: "demo@example.test".into(),
            plan: "pro".into(),
        };
        let usage=crate::usage::Usage::parse(br#"{"rate_limit":{"primary_window":{"used_percent":73,"limit_window_seconds":604800,"reset_at":2000000000}},"rate_limit_reset_credits":{"available_count":2}}"#).unwrap();
        let view = crate::UsageView {
            data: Some(usage),
            checked: Some(chrono::Utc::now()),
            error: Some("offline".into()),
        };
        let status = status_for(Some(&account), Some(&view), false, chrono::Utc::now());
        assert_eq!(status.title, "—%");
        assert!(status.account.contains("demo@example.test"));
        assert!(
            status
                .lines
                .iter()
                .any(|s| s.contains("Weekly") && s.contains("27% pozostało"))
        );
        assert!(!status.lines.iter().any(|s| s.starts_with("5h")));
        assert!(status.lines.iter().any(|s| s.contains("nieaktualne")));
        assert!(
            status
                .lines
                .iter()
                .any(|s| s.contains("Dostępne restarty: 2"))
        );
        let empty = status_for(None, Some(&view), false, chrono::Utc::now());
        assert_eq!(empty.account, "Nie wybrano konta");
        assert_eq!(empty.title, "—%");
        assert!(!empty.lines.iter().any(|s| s.contains("27%")));
    }

    #[test]
    fn switching_accounts_updates_title_and_loading_keeps_last_read() {
        let first = crate::accounts::Account {
            id: "a".repeat(64),
            email: "first@example.test".into(),
            plan: "pro".into(),
        };
        let second = crate::accounts::Account {
            id: "b".repeat(64),
            email: "second@example.test".into(),
            plan: "pro".into(),
        };
        let now = chrono::Utc::now();
        let first_status = status_for(Some(&first), Some(&view(&[(18000, 60.0)])), true, now);
        assert_eq!(first_status.title, "40%");
        let second_status = status_for(Some(&second), Some(&view(&[(604800, 9.0)])), false, now);
        assert_eq!(second_status.title, "91%");
        assert!(second_status.account.contains("second@example.test"));
        assert_eq!(status_for(Some(&second), None, true, now).title, "—%");
        assert_eq!(
            status_for(None, Some(&view(&[(18000, 60.0)])), false, now).title,
            "—%"
        );
    }
}
