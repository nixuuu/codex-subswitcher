//! Account identity, capacity, and reset detail components.
use super::*;

impl Switcher {
    fn reset_button(&self, account: &accounts::Account, cx: &Context<Self>) -> Button {
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let pending = data.is_some_and(|d| d.pending_reset);
        let next = data
            .and_then(|d| d.reset_details.as_ref())
            .and_then(|d| d.next(self.now));
        let credit_id = next.map(|c| c.id.clone());
        let expiry = next.map(|c| c.expiry_label()).unwrap_or_default();
        let fresh = view.is_some_and(|v| {
            v.error.is_none()
                && v.checked
                    .is_some_and(|t| (chrono::Utc::now() - t).num_seconds() < 120)
        });
        #[cfg(debug_assertions)]
        let fresh = fresh || demo_mode();
        let id = account.id.clone();
        let account_label = account.display_label();
        action(SharedString::from(format!("reset-{id}")), if pending {"Retry reset"} else {"Use reset"})
            .disabled(self.busy || self.usage_loading || !self.expanded_accounts.contains(&account.id) || (!pending && (!fresh || credit_id.is_none())))
            .on_click(cx.listener(move |_,_,window,cx| {
                let id=id.clone(); let account_label=account_label.clone(); let credit_id=credit_id.clone(); let expiry=expiry.clone(); let entity=cx.entity().downgrade();
                window.open_alert_dialog(cx,move |dialog,_,_| {
                    let id=id.clone(); let credit_id=credit_id.clone(); let entity=entity.clone();
                    dialog.title(if pending {"Retry reset?"} else {"Use a limit reset?"})
                        .button_props(dialog_actions(if pending {"Retry"}else{"Use 1 reset"}, true))
                        .child(if pending {format!("Check or complete the previous reset for {account_label}, using the same reset credit and operation ID.")}else{format!("{account_label}. {expiry}. This uses the available reset credit that expires first to reset Codex limits (weekly and 5h, where available). This cannot be undone.")})
                        .on_ok(move |_,_,cx| {let id=id.clone();let credit_id=credit_id.clone();let _=entity.update(cx,|this,cx|this.reset_account(id,credit_id,cx));true})
                });
            }))
    }

    fn account_details(&self, account: &accounts::Account, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let mut content = stack()
            .p(px(tokens::SPACE_CONTENT))
            .bg(theme.muted)
            .rounded(px(tokens::RADIUS))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_weight(FontWeight::MEDIUM).child("Limit resets"))
                    .child(self.reset_button(account, cx)),
            );
        content = content.child(
            div()
                .text_color(theme.muted_foreground)
                .child(account.display_label()),
        );
        if let Some(details) = data.and_then(|d| d.reset_details.as_ref()) {
            let next = details.next(self.now);
            let mut count = 0;
            for credit in details.credits.iter().filter(|c| c.available) {
                count += 1;
                let status = if !credit.eligible(self.now) {
                    "Expired"
                } else if next.is_some_and(|n| n.id == credit.id) {
                    "Next to use"
                } else {
                    "Available"
                };
                content = content.child(
                    div()
                        .id(SharedString::from(format!(
                            "credit-{}-{}",
                            account.id, credit.id
                        )))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(credit.expiry_label())
                        .child(badge(
                            status,
                            if credit.eligible(self.now) {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            },
                            theme.background,
                        )),
                );
            }
            if count == 0 {
                content = content.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("No resets available."),
                );
            }
        } else {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Expiration dates are currently unavailable."),
            );
        }
        if let Some(error) = data.and_then(|d| d.reset_details_error.as_ref()) {
            content = content.child(div().text_color(theme.danger).child(error.clone()));
        }
        if let Some(data) = data {
            for limit in data.windows() {
                content = content.child(div().text_color(theme.muted_foreground).child(format!(
                    "{} · {}",
                    limit.label(),
                    limit.reset_label(self.now)
                )));
            }
        }
        let id = account.id.clone();
        let selected = self.snapshot.current.as_ref().is_some_and(|a| a.id == id);
        content.child(div().flex().justify_between().items_center().pt_2().border_t_1().border_color(theme.border)
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Profile {}",&account.id[..8])))
            .child(action(SharedString::from(format!("delete-{id}")), "Remove account").disabled(self.busy||selected||!self.expanded_accounts.contains(&id))
                .on_click(cx.listener(move |_,_,window,cx| {
                    let id=id.clone();let entity=cx.entity().downgrade();
                    window.open_alert_dialog(cx,move |dialog,_,_| {
                        let id=id.clone();let entity=entity.clone();
                        dialog.title("Remove saved account?").button_props(dialog_actions("Remove", true))
                            .child("Removing this profile does not cancel your subscription. You will need to sign in to add it again.")
                            .on_ok(move |_,_,cx| {let id=id.clone();let _=entity.update(cx,|this,cx|this.work("Removing…","Saved profile removed.",move |store|store.remove(&id),cx));true})
                    });
                }))))
    }

    pub(super) fn account_row(
        &self,
        account: &accounts::Account,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self
            .snapshot
            .current
            .as_ref()
            .is_some_and(|a| a.id == account.id);
        let expanded = self.expanded_accounts.contains(&account.id);
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let id = account.id.clone();
        let toggle_id = id.clone();
        let count = data.and_then(|d| {
            d.reset_details
                .as_ref()
                .map(|d| d.available_count)
                .or_else(|| {
                    d.rate_limit_reset_credits
                        .as_ref()
                        .map(|c| c.available_count)
                })
        });
        let email = account.email.clone();
        let account_label = account.display_label();
        let reveal_email = icon_action(
            SharedString::from(format!("show-email-{id}")),
            gpui_kit::component::IconName::Eye,
            "Show email",
        )
        .text_color(theme.muted_foreground)
        .accessibility_label(format!("Show email for {account_label}"))
        .on_click(cx.listener(move |_, _, window, cx| {
            let email = email.clone();
            let account_label = account_label.clone();
            window.open_alert_dialog(cx, move |dialog, _, _| {
                dialog
                    .title(account_label.clone())
                    .button_props(dialog_actions("Hide email", false))
                    .child(email.clone())
                    .on_ok(|_, _, _| true)
            });
        }));
        let identity = div().min_w_0().flex().flex_1().child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(account.display_label()),
                )
                .child(reveal_email),
        );
        let actions = div().flex().items_end().gap_1().flex_shrink_0().child(
            compact_action(
                SharedString::from(format!("switch-{id}")),
                if selected { "Active" } else { "Activate" },
            )
            .tooltip(if selected {
                "This account is active"
            } else {
                "Use this account for the next request"
            })
            .disabled(self.busy || selected || self.proxy.is_none())
            .on_click(cx.listener(move |this, _, _, cx| {
                let id = id.clone();
                this.work(
                    "Switching…",
                    "Account switched. The next request will use this account.",
                    move |store| store.switch(&id),
                    cx,
                );
            })),
        );
        let details = icon_action(
            SharedString::from(format!("details-{toggle_id}")),
            if expanded {
                gpui_kit::component::IconName::ChevronUp
            } else {
                gpui_kit::component::IconName::ChevronDown
            },
            if expanded {
                "Hide account details"
            } else {
                "Account details and limit resets"
            },
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.expanded_accounts.remove(&toggle_id) {
                this.expanded_accounts.insert(toggle_id.clone());
            }
            cx.notify();
        }));
        let mut meters = stack().min_w_0().gap(px(tokens::SPACE_INLINE));
        if let Some(data) = data {
            for (index, limit) in data.windows().enumerate() {
                meters = meters.child(limits::meter(&account.id, index, limit, self.now, cx));
            }
            if data.windows().next().is_none() {
                meters = meters.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("No limit windows reported"),
                );
            }
        } else {
            meters = meters.child(div().text_color(theme.muted_foreground).child(
                if self.usage_loading {
                    "Loading limits…"
                } else {
                    "Limits unavailable"
                },
            ));
        }
        let resets = div()
            .min_w_0()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(tokens::SPACE_INLINE))
            .text_size(px(tokens::TEXT_CAPTION))
            .text_color(theme.muted_foreground)
            .child(account.plan.clone())
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_size(px(tokens::TEXT_CAPTION))
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "· {} {}",
                            count.map(|n| n.to_string()).unwrap_or("—".into()),
                            if count == Some(1) { "reset" } else { "resets" }
                        )),
                ),
            )
            .when(data.is_some_and(|d| d.pending_reset), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("Reset needs confirmation"),
                )
            })
            .when(data.is_some_and(|d| d.reset_details_error.is_some()), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("Reset data is outdated"),
                )
            });
        let body = stack()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(identity)
                    .child(actions)
                    .child(details),
            )
            .child(resets)
            .child(meters);
        div()
            .id(SharedString::from(format!("account-{}", account.id)))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p(px(tokens::SPACE_INLINE))
            .rounded(px(tokens::PANEL_RADIUS))
            .bg((if selected { theme.accent } else { theme.muted })
                .opacity(tokens::GLASS_CARD_ALPHA))
            .border_1()
            .border_color(if selected {
                theme.primary.opacity(0.35)
            } else {
                theme.border
            })
            .child(body)
            .when(view.is_some_and(|v| v.error.is_some()), |d| {
                d.child(
                    div()
                        .pt(px(tokens::SPACE_CONTENT))
                        .text_color(theme.danger)
                        .child(format!(
                            "{} Data may be outdated.",
                            view.and_then(|v| v.error.clone()).unwrap_or_default()
                        )),
                )
            })
            .child(Disclosure::new(
                format!("account-details-{}", account.id),
                expanded,
                div()
                    .pt(px(tokens::SPACE_CONTENT))
                    .child(self.account_details(account, cx)),
            ))
    }
}
