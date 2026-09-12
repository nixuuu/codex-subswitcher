//! Page header, account actions, and operational feedback.
use super::*;

impl Switcher {
    pub(super) fn empty_accounts(&self, cx: &Context<Self>) -> Div {
        stack()
            .py_8()
            .gap(px(tokens::SPACE_INLINE))
            .child(div().text_lg().child("Add your first account"))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Sign in to an account or import your CLI credentials."),
            )
    }

    pub(super) fn app_footer(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .justify_between()
            .gap(px(tokens::SPACE_CONTENT))
            .text_size(px(tokens::TEXT_CAPTION))
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
            .gap(px(tokens::SPACE_SECTION))
            .child(
                stack()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(tokens::TEXT_HEADING))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Codex Sub Switcher"),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Accounts and subscription limits"),
                    ),
            )
            .child(action("theme", "Toggle theme").on_click(|_, window, cx| {
                let mode = if cx.theme().is_dark() {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                palette::change(mode, Some(window), cx);
            }))
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
            .child(div().flex().items_center().justify_between().gap(px(tokens::SPACE_CONTENT))
                .child(badge(if self.proxy.is_some() { "Proxy active" } else { "Proxy unavailable" },
                    if self.proxy.is_some() { theme.success } else { theme.danger }, theme.muted))
                .child(div().text_xs().text_color(theme.muted_foreground).child(refresh_text)))
            .child(div().flex().items_center().gap(px(tokens::SPACE_CONTENT))
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

    pub(super) fn account_toolbar(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(tokens::SPACE_CONTENT))
            .pt(px(tokens::SPACE_INLINE))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("Accounts ({})", self.snapshot.accounts.len())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(tokens::SPACE_INLINE))
                    .child(
                        action("refresh", "Refresh")
                            .disabled(self.busy || self.usage_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.work(
                                    "Refreshing…",
                                    "Account list refreshed.",
                                    |s| s.snapshot(),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        action("import", "Import from CLI")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.work(
                                    "Importing…",
                                    "Account saved.",
                                    |s| s.import_current(),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        primary_action("add", "Add account")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel = Arc::new(AtomicBool::new(false));
                                let cancel = this.cancel.clone();
                                this.login_pending = true;
                                this.work(
                                    "Complete sign-in in your browser.",
                                    "Account added.",
                                    move |s| launcher::login(&s, cancel),
                                    cx,
                                );
                            })),
                    ),
            )
    }

    pub(super) fn operation_status(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .flex()
            .items_center()
            .gap(px(tokens::SPACE_CONTENT))
            .p(px(tokens::SPACE_CONTENT))
            .rounded(px(tokens::RADIUS))
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
            .when(self.login_pending, |d| {
                d.child(action("cancel", "Cancel sign-in").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cancel.store(true, Ordering::Relaxed);
                        this.status = "Canceling sign-in…".into();
                        cx.notify();
                    },
                )))
            })
    }
}
