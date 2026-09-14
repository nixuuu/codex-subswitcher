//! Account identity, capacity, and reset detail components.
use super::*;

fn reset_disabled_reason(
    busy: bool,
    usage_loading: bool,
    pending: bool,
    fresh: bool,
    has_credit: bool,
) -> Option<&'static str> {
    if busy {
        Some("Another account operation is in progress")
    } else if usage_loading {
        Some("Refreshing limits before a reset can be used")
    } else if pending {
        None
    } else if !fresh {
        Some("Refresh limits to use a reset")
    } else if !has_credit {
        Some("No eligible reset credits")
    } else {
        None
    }
}

fn remove_disabled_reason(busy: bool, selected: bool) -> Option<&'static str> {
    if busy {
        Some("Another account operation is in progress")
    } else if selected {
        Some("Activate another account before removing this one")
    } else {
        None
    }
}

fn remove_dialog_title(account_label: &str) -> String {
    format!("Remove “{account_label}”?")
}

impl Switcher {
    fn reset_button(&self, account: &accounts::Account, fresh: bool, cx: &Context<Self>) -> Button {
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let pending = data.is_some_and(|d| d.pending_reset);
        let next = data
            .and_then(|d| d.reset_details.as_ref())
            .and_then(|d| d.next(self.now));
        let credit_id = next.map(|c| c.id.clone());
        let expiry = next.map(|c| c.expiry_label()).unwrap_or_default();
        let id = account.id.clone();
        let account_label = account.display_label();
        let reason = reset_disabled_reason(
            self.busy,
            self.usage_loading,
            pending,
            fresh,
            credit_id.is_some(),
        );
        action(SharedString::from(format!("reset-{id}")), if pending {"Retry reset…"} else {"Use reset…"})
            .disabled(reason.is_some() || !self.expanded_accounts.contains(&account.id))
            .loading(self.busy && pending)
            .on_click(cx.listener(move |_,_,window,cx| {
                let id=id.clone(); let account_label=account_label.clone(); let credit_id=credit_id.clone(); let expiry=expiry.clone(); let entity=cx.entity().downgrade();
                window.open_alert_dialog(cx,move |dialog,_,_| {
                    let id=id.clone(); let credit_id=credit_id.clone(); let entity=entity.clone();
                    dialog.title(if pending {format!("Retry reset for “{account_label}”?")} else {format!("Use a limit reset for “{account_label}”?")})
                        .button_props(destructive_dialog_actions(if pending {"Retry"}else{"Use 1 reset"}))
                        .child(if pending {format!("Check or complete the previous reset for {account_label}. Retrying preserves the previous operation and does not select another credit.")}else{format!("{account_label}. {expiry}. This uses the available reset credit that expires first to reset Codex limits (weekly and 5h, where available). This cannot be undone.")})
                        .on_ok(move |_,_,cx| {let id=id.clone();let credit_id=credit_id.clone();let _=entity.update(cx,|this,cx|this.reset_account(id,credit_id,cx));true})
                });
            }))
    }

    fn account_details(&self, account: &accounts::Account, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let fresh = view.is_some_and(|v| {
            v.error.is_none()
                && v.checked
                    .is_some_and(|t| (chrono::Utc::now() - t).num_seconds() < 120)
        });
        #[cfg(debug_assertions)]
        let fresh = fresh || demo_mode();
        let has_weekly_budget = data.is_some_and(|data| {
            data.windows()
                .any(|limit| limit.weekly_budget(self.now).is_some())
        });
        let reset_reason = reset_disabled_reason(
            self.busy,
            self.usage_loading,
            data.is_some_and(|d| d.pending_reset),
            fresh,
            data.and_then(|d| d.reset_details.as_ref())
                .and_then(|d| d.next(self.now))
                .is_some(),
        );
        let mut content = stack()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .when(has_weekly_budget, |details| {
                details.child(
                    div()
                        .pb_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(
                            "Weekly budget assumes even usage across the week; it is not a usage forecast.",
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_weight(FontWeight::MEDIUM).child("Limit resets"))
                    .child(self.reset_button(account, fresh, cx)),
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
                        .child("No eligible reset credits."),
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
        if let Some(reason) = reset_reason {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(reason),
            );
        }
        let id = account.id.clone();
        let selected = self.snapshot.current.as_ref().is_some_and(|a| a.id == id);
        let account_label = account.display_label();
        let remove_reason = remove_disabled_reason(self.busy, selected);
        content.child(div().flex().justify_between().items_center().pt_2().border_t_1().border_color(theme.border)
            .child(div().text_sm().text_color(theme.muted_foreground).when_some(remove_reason, |d, reason| d.child(reason)))
            .child(action(SharedString::from(format!("delete-{id}")), "Remove account…").disabled(remove_reason.is_some()||!self.expanded_accounts.contains(&id))
                .on_click(cx.listener(move |_,_,window,cx| {
                    let id=id.clone();let account_label=account_label.clone();let entity=cx.entity().downgrade();
                    window.open_alert_dialog(cx,move |dialog,_,_| {
                        let id=id.clone();let entity=entity.clone();
                        dialog.title(remove_dialog_title(&account_label)) .button_props(destructive_dialog_actions("Remove"))
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
        let activate_id = id.clone();
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
            } else if self.proxy.is_none() {
                "The proxy is unavailable; open connection info in Settings"
            } else if self.busy {
                "Wait for the current account operation to finish"
            } else {
                "Use this account for the next request"
            })
            .disabled(self.busy || selected || self.proxy.is_none())
            .on_click(cx.listener(move |this, _, _, cx| {
                let id = activate_id.clone();
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
        let mut meters = stack().min_w_0().gap_2();
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
            .gap_2()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(account.plan.clone())
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_xs()
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
            .p_2()
            .rounded(theme.radius_lg)
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
                d.child(div().pt_3().text_color(theme.danger).child(format!(
                    "{} Data may be outdated.",
                    view.and_then(|v| v.error.clone()).unwrap_or_default()
                )))
            })
            .child(Disclosure::new(
                format!("account-details-{}", account.id),
                expanded,
                div().pt_3().child(self.account_details(account, cx)),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::{remove_dialog_title, remove_disabled_reason, reset_disabled_reason};

    #[test]
    fn reset_reason_matches_pending_and_credit_rules() {
        assert_eq!(
            reset_disabled_reason(false, false, false, false, true),
            Some("Refresh limits to use a reset")
        );
        assert_eq!(
            reset_disabled_reason(false, false, false, true, false),
            Some("No eligible reset credits")
        );
        assert_eq!(
            reset_disabled_reason(false, false, true, false, false),
            None
        );
        assert_eq!(
            reset_disabled_reason(false, true, true, true, true),
            Some("Refreshing limits before a reset can be used")
        );
    }

    #[test]
    fn active_account_explains_remove_block() {
        assert_eq!(
            remove_disabled_reason(false, true),
            Some("Activate another account before removing this one")
        );
        assert_eq!(remove_disabled_reason(false, false), None);
    }

    #[test]
    fn remove_confirmation_identifies_the_selected_account() {
        assert_eq!(
            remove_dialog_title("Account a1b2c3d4"),
            "Remove “Account a1b2c3d4”?"
        );
        assert_eq!(
            remove_dialog_title("Account e5f6a7b8"),
            "Remove “Account e5f6a7b8”?"
        );
    }
}
