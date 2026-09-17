//! Page header, account actions, and operational feedback.
use super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

impl Switcher {
    fn add_account_menu(&self, id: &'static str, cx: &Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        compact_action(id, "Add account…")
            .disabled(self.busy)
            .loading(self.login_pending)
            .dropdown_menu(move |menu, _, _| {
                let local = entity.clone();
                let remote = entity.clone();
                menu.item(PopupMenuItem::new("Sign in on this computer…").on_click(
                    move |_, _, cx| {
                        let _ = local.update(cx, |this, cx| {
                            this.begin_login(launcher::LoginMode::Browser, cx)
                        });
                    },
                ))
                .item(
                    PopupMenuItem::new("Sign in on another computer…").on_click(move |_, _, cx| {
                        let _ = remote.update(cx, |this, cx| {
                            this.begin_login(launcher::LoginMode::Device, cx)
                        });
                    }),
                )
            })
    }

    pub(super) fn panel_footer(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let checked = self.usage.values().filter_map(|v| v.checked).min();
        let updated = if self.usage_loading {
            "Refreshing…".into()
        } else {
            checked
                .map(|at| {
                    format!(
                        "Updated {}",
                        at.with_timezone(&chrono::Local).format("%H:%M")
                    )
                })
                .unwrap_or("Refreshes every minute".into())
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_color(theme.foreground)
                            .child(if self.proxy.is_some() {
                                "Proxy running"
                            } else {
                                "Proxy unavailable"
                            }),
                    )
                    .child(div().text_color(theme.muted_foreground).child(updated)),
            )
            .child(
                compact_action("connect-terminal", "Connect terminal…").on_click(|_, _, cx| {
                    cx.defer(|cx| {
                        windows::show_settings_section(windows::SettingsSection::Connection, cx)
                    })
                }),
            )
    }

    pub(super) fn proxy_notice(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .p_3()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(cx.theme().muted_foreground)
                    .child("Activate is unavailable because the proxy is not running."),
            )
            .child(
                compact_action("proxy-info", "Connection info…").on_click(|_, _, cx| {
                    cx.defer(|cx| {
                        windows::show_settings_section(windows::SettingsSection::Connection, cx)
                    })
                }),
            )
    }

    pub(super) fn settings_accounts(&self, cx: &Context<Self>) -> Div {
        panel(cx)
            .child(div().font_weight(FontWeight::MEDIUM).child("Accounts"))
            .child(div().text_color(cx.theme().muted_foreground).child(
                "Manage saved accounts in the menu-bar panel, or import your existing CLI sign-in.",
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        action("open-accounts", "Open accounts…")
                            .on_click(|_, _, cx| cx.defer(windows::show_panel)),
                    )
                    .child(
                        action("import", "Import from CLI…")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.work(
                                    "Importing…",
                                    "Account saved.",
                                    |s| s.import_current(),
                                    cx,
                                )
                            })),
                    ),
            )
    }

    pub(super) fn empty_accounts(&self, cx: &Context<Self>) -> Div {
        stack()
            .py_8()
            .gap_2()
            .child(div().text_lg().child("Add your first account"))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Sign in to an account or import your CLI credentials."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.add_account_menu("empty-add", cx))
                    .child(
                        compact_action("empty-import", "Import in Settings…").on_click(
                            |_, _, cx| {
                                cx.defer(|cx| {
                                    windows::show_settings_section(
                                        windows::SettingsSection::Accounts,
                                        cx,
                                    )
                                })
                            },
                        ),
                    ),
            )
    }

    pub(super) fn app_footer(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .justify_between()
            .gap_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("Switching accounts applies to the next request.")
            .child(format!(
                "Requests: {} · In progress: {} · Errors: {}",
                self.counters.0, self.counters.1, self.counters.2
            ))
    }

    pub(super) fn app_header(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                stack()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Connection, notifications and diagnostics"),
                    ),
            )
            .child(
                stack()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Appearance"),
                    )
                    .child(
                        gpui_kit::component::radio::RadioGroup::horizontal("appearance")
                            .flex_none()
                            .selected_index(Some(if cx.theme().is_dark() { 1 } else { 0 }))
                            .child(
                                gpui_kit::component::radio::Radio::new("appearance-light")
                                    .label("Light"),
                            )
                            .child(
                                gpui_kit::component::radio::Radio::new("appearance-dark")
                                    .label("Dark"),
                            )
                            .on_change(|index, window, cx| {
                                palette::change(
                                    if *index == 0 {
                                        ThemeMode::Light
                                    } else {
                                        ThemeMode::Dark
                                    },
                                    Some(window),
                                    cx,
                                );
                            }),
                    ),
            )
    }

    pub(super) fn service_status(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let checked = self.usage.values().filter_map(|v| v.checked).min();
        let refresh_text = if self.usage_loading {
            "Refreshing accounts…".into()
        } else {
            checked
                .map(|at| {
                    format!(
                        "Updated {} · refreshes every minute",
                        at.with_timezone(&chrono::Local).format("%H:%M")
                    )
                })
                .unwrap_or("Refreshes automatically every minute".into())
        };
        stack()
            .child(div().flex().items_center().justify_between().gap_3()
                .child(badge(if self.proxy.is_some() { "Proxy running" } else { "Proxy unavailable" },
                    if self.proxy.is_some() { theme.success } else { theme.danger }, theme.muted))
                .child(div().text_xs().text_color(theme.muted_foreground).child(refresh_text)))
            .child(div().flex().items_center().gap_3()
                .child(div().flex_1().min_w_0().text_xs().text_color(theme.muted_foreground).child(notifications::status(cx)))
                .child(action("test-notifications", "Test notification")
                    .disabled(!notifications::can_test(cx))
                    .on_click(|_, _, cx| notifications::show(SystemNotification {
                        tag: "silent-reset-test".into(),
                        title: "Notifications are working".into(),
                        body: "You will receive a notification here when an account's limits reset.".into(),
                        actions: Vec::new(),
                    }, cx))))
    }

    pub(super) fn panel_header(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("Accounts ({})", self.snapshot.accounts.len())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        icon_action(
                            "refresh",
                            gpui_kit::component::IconName::RotateCw,
                            "Refresh accounts",
                        )
                        .disabled(self.busy || self.usage_loading)
                        .loading(self.usage_loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.work(
                                "Refreshing…",
                                "Account list refreshed.",
                                |s| s.snapshot(),
                                cx,
                            )
                        })),
                    )
                    .when(!self.snapshot.accounts.is_empty(), |actions| {
                        actions.child(self.add_account_menu("add", cx))
                    })
                    .child(
                        icon_action(
                            "settings",
                            gpui_kit::component::IconName::Settings,
                            "Settings…",
                        )
                        .on_click(|_, _, cx| cx.defer(windows::show_settings)),
                    ),
            )
    }

    pub(super) fn operation_status(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded(theme.radius)
            .bg(theme.muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(if self.error {
                        theme.danger
                    } else {
                        theme.muted_foreground
                    })
                    .child(self.status.clone()),
            )
            .when_some(self.device_login.as_ref(), |d, details| {
                d.child(
                    stack()
                        .gap_2()
                        .child(details.url())
                        .child(format!("One-time code: {}", details.code()))
                        .child(
                            action(
                                "copy-device-login",
                                if self.login_copied {
                                    "Link and code copied"
                                } else {
                                    "Copy link and code"
                                },
                            )
                            .disabled(self.cancel.load(Ordering::Relaxed))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(details) = &this.device_login {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        details.copy_text(),
                                    ));
                                    this.login_copied = true;
                                    cx.notify();
                                }
                            })),
                        ),
                )
            })
            .when(self.login_pending, |d| {
                d.child(
                    action("cancel", "Cancel sign-in")
                        .disabled(self.cancel.load(Ordering::Relaxed))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cancel.store(true, Ordering::Relaxed);
                            this.login_progress = None;
                            this.device_login = None;
                            this.status = "Canceling sign-in…".into();
                            cx.notify();
                        })),
                )
            })
    }

    pub(super) fn settings_operation_status(&self, cx: &Context<Self>) -> Div {
        div().when(!self.status.is_empty(), |d| {
            d.child(self.operation_status(cx))
        })
    }

    fn begin_login(&mut self, mode: launcher::LoginMode, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.login_pending = true;
        let (progress, receiver) = std::sync::mpsc::channel();
        self.login_progress = Some(receiver);
        self.device_login = None;
        self.login_copied = false;
        self.work(
            match mode {
                launcher::LoginMode::Browser => "Complete sign-in in your browser.",
                launcher::LoginMode::Device => "Preparing a sign-in link and one-time code…",
            },
            "Account added.",
            move |store| launcher::login(&store, cancel, mode, progress),
            cx,
        );
    }
}
