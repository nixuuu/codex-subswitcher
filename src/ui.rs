//! Compact account overview; one page scroll, with optional per-account detail disclosure.
use super::*;
use gpui_kit::component::Sizable;

fn badge(text: impl Into<SharedString>, color: Hsla, background: Hsla) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .text_color(color)
        .bg(background)
        .child(text.into())
}

impl Switcher {
    fn limit_meter(&self, id: &str, index: usize, limit: &usage::Window, cx: &App) -> Div {
        let theme = cx.theme();
        let reset = limit
            .reset_at
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
        let reset_text = match reset {
            Some(at) if at > self.now => format!(
                "Reset {}",
                at.with_timezone(&chrono::Local).format("%d.%m · %H:%M")
            ),
            Some(_) => "Oczekiwanie na reset".into(),
            None => "Termin niedostępny".into(),
        };
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(limit.label()),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{:.0}%", limit.used_percent)),
                    ),
            )
            .child(
                Progress::new(SharedString::from(format!("usage-{id}-{index}")))
                    .small()
                    .value(limit.used_percent)
                    .accessibility_label(format!(
                        "{}: {:.0}% użyte",
                        limit.label(),
                        limit.used_percent
                    ))
                    .color(if limit.used_percent >= 90. {
                        theme.danger
                    } else {
                        theme.primary
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(reset_text),
            )
    }

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
        let id = account.id.clone();
        let email = account.email.clone();
        Button::new(SharedString::from(format!("reset-{id}"))).small()
            .label(if pending {"Ponów restart"} else {"Użyj restartu"})
            .disabled(self.busy || self.usage_loading || (!pending && (!fresh || credit_id.is_none())))
            .on_click(cx.listener(move |_,_,window,cx| {
                let id=id.clone(); let email=email.clone(); let credit_id=credit_id.clone(); let expiry=expiry.clone(); let entity=cx.entity().downgrade();
                window.open_alert_dialog(cx,move |dialog,_,_| {
                    let id=id.clone(); let credit_id=credit_id.clone(); let entity=entity.clone();
                    dialog.title(if pending {"Ponowić restart?"} else {"Wykorzystać restart limitów?"})
                        .button_props(DialogButtonProps::default().ok_text(if pending {"Ponów"}else{"Użyj 1 restartu"}).show_cancel(true).cancel_text("Anuluj"))
                        .child(if pending {format!("Sprawdzi lub dokończy poprzedni restart konta {email}, używając tego samego restartu i identyfikatora operacji.")}else{format!("Konto: {email}. {expiry}. Wykorzysta 1 restart o najkrótszej ważności do resetu limitów Codex (weekly i 5h, jeśli występują). Tej operacji nie można cofnąć.")})
                        .on_ok(move |_,_,cx| {let id=id.clone();let credit_id=credit_id.clone();let _=entity.update(cx,|this,cx|this.reset_account(id,credit_id,cx));true})
                });
            }))
    }

    fn account_details(&self, account: &accounts::Account, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let view = self.usage.get(&account.id);
        let data = view.and_then(|v| v.data.as_ref());
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(theme.muted)
            .rounded_md()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Restarty limitów"),
                    )
                    .child(self.reset_button(account, cx)),
            );
        content = content.child(
            div()
                .text_color(theme.muted_foreground)
                .child(account.email.clone()),
        );
        if let Some(details) = data.and_then(|d| d.reset_details.as_ref()) {
            let next = details.next(self.now);
            let mut count = 0;
            for credit in details.credits.iter().filter(|c| c.available) {
                count += 1;
                let status = if !credit.eligible(self.now) {
                    "Wygasł"
                } else if next.is_some_and(|n| n.id == credit.id) {
                    "Następny do użycia"
                } else {
                    "Dostępny"
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
                        .child("Brak dostępnych restartów."),
                );
            }
        } else {
            content = content.child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Daty ważności są obecnie niedostępne."),
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
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Profil {}",&account.id[..8])))
            .child(Button::new(SharedString::from(format!("delete-{id}"))).label("Usuń konto").small().ghost().disabled(self.busy||selected)
                .on_click(cx.listener(move |_,_,window,cx| {
                    let id=id.clone();let entity=cx.entity().downgrade();
                    window.open_alert_dialog(cx,move |dialog,_,_| {
                        let id=id.clone();let entity=entity.clone();
                        dialog.title("Usunąć zapisane konto?").button_props(DialogButtonProps::default().ok_text("Usuń").show_cancel(true).cancel_text("Anuluj"))
                            .child("Usunięcie profilu nie anuluje subskrypcji. Ponowne dodanie wymaga logowania.")
                            .on_ok(move |_,_,cx| {let id=id.clone();let _=entity.update(cx,|this,cx|this.work("Usuwanie…","Usunięto zapisany profil.",move |store|store.remove(&id),cx));true})
                    });
                }))))
    }

    fn account_row(
        &self,
        account: &accounts::Account,
        wide: bool,
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
        let identity = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .when(wide, |d| d.w(px(210.)).flex_shrink_0())
            .when(!wide, |d| d.flex_1())
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .child(account.email.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(account.plan.clone()),
                    )
                    .when(selected, |d| {
                        d.child(badge("Aktywne", theme.success, theme.accent))
                    }),
            );
        let actions = div()
            .flex()
            .flex_col()
            .items_end()
            .gap_1()
            .flex_shrink_0()
            .child(
                Button::new(SharedString::from(format!("switch-{id}")))
                    .small()
                    .label(if selected { "Wybrane" } else { "Przełącz" })
                    .disabled(self.busy || selected || self.proxy.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.work(
                            "Przełączanie…",
                            "Konto przełączone. Następne żądanie użyje tego konta.",
                            move |store| store.switch(&id),
                            cx,
                        );
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("details-{toggle_id}")))
                    .small()
                    .ghost()
                    .label(if expanded { "Zwiń" } else { "Szczegóły" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.expanded_accounts.remove(&toggle_id) {
                            this.expanded_accounts.insert(toggle_id.clone());
                        }
                        cx.notify();
                    })),
            );
        let mut meters = div().flex_1().min_w_0().flex().gap_4();
        if let Some(data) = data {
            for (index, limit) in data.windows().enumerate() {
                meters = meters.child(self.limit_meter(&account.id, index, limit, cx));
            }
            if data.windows().next().is_none() {
                meters = meters.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child("Brak udostępnionych okien limitów"),
                );
            }
        } else {
            meters = meters.child(div().text_color(theme.muted_foreground).child(
                if self.usage_loading {
                    "Pobieranie limitów…"
                } else {
                    "Limity niedostępne"
                },
            ));
        }
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
        let next = data
            .and_then(|d| d.reset_details.as_ref())
            .and_then(|d| d.next(self.now));
        let expiration = next
            .map(|c| {
                c.expires_at
                    .map(|at| {
                        format!(
                            "Najbliższy: {}",
                            at.with_timezone(&chrono::Local).format("%d.%m · %H:%M")
                        )
                    })
                    .unwrap_or("Bez daty wygaśnięcia".into())
            })
            .unwrap_or_else(|| {
                if count == Some(0) {
                    "Brak do wykorzystania".into()
                } else {
                    "Daty niedostępne".into()
                }
            });
        let resets = div()
            .w(px(174.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(badge(
                        count.map(|n| n.to_string()).unwrap_or("—".into()),
                        theme.foreground,
                        theme.muted,
                    ))
                    .child(div().text_color(theme.muted_foreground).child(match count {
                        Some(1) => "restart",
                        Some(n)
                            if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) =>
                        {
                            "restarty"
                        }
                        _ => "restartów",
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(expiration),
            )
            .when(data.is_some_and(|d| d.pending_reset), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("Restart do potwierdzenia"),
                )
            })
            .when(data.is_some_and(|d| d.reset_details_error.is_some()), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child("Nieaktualne dane restartów"),
                )
            });
        let body = if wide {
            div()
                .flex()
                .items_center()
                .gap_5()
                .child(identity)
                .child(meters)
                .child(resets)
                .child(actions)
        } else {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(identity)
                        .child(actions),
                )
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_5()
                        .child(meters)
                        .child(resets),
                )
        };
        div()
            .id(SharedString::from(format!("account-{}", account.id)))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .py_3()
            .gap_3()
            .border_b_1()
            .border_color(theme.border)
            .child(body)
            .when(view.is_some_and(|v| v.error.is_some()), |d| {
                d.child(div().text_color(theme.danger).child(format!(
                    "{} Dane mogą być nieaktualne.",
                    view.and_then(|v| v.error.clone()).unwrap_or_default()
                )))
            })
            .when(expanded, |d| d.child(self.account_details(account, cx)))
    }
}

impl Render for Switcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bg = theme.background;
        let fg = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let danger = theme.danger;
        let wide = window.viewport_size().width >= px(900.);
        let rows = self
            .snapshot
            .accounts
            .iter()
            .map(|a| self.account_row(a, wide, cx))
            .collect::<Vec<_>>();
        let checked = self.usage.values().filter_map(|v| v.checked).min();
        let refresh_text = if self.usage_loading {
            "Odświeżanie kont…".into()
        } else {
            checked
                .map(|at| {
                    format!(
                        "Odczyt {} · automatycznie co minutę",
                        at.with_timezone(&chrono::Local).format("%H:%M")
                    )
                })
                .unwrap_or("Automatyczne odświeżanie co minutę".into())
        };
        div().size_full().relative().bg(bg).text_color(fg).font_family(".AppleSystemUIFont").text_sm()
            .child(div().id("page").size_full().overflow_y_scroll().track_scroll(&self.page_scroll)
                .child(div().flex().flex_col().p_5().gap_4()
                    .child(div().flex().items_center().justify_between().gap_4()
                        .child(div().flex().flex_col().gap_1().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Codex Sub Switcher"))
                            .child(div().text_color(muted).child("Konta i limity subskrypcji")))
                        .child(Button::new("theme").small().ghost().label("Zmień motyw").on_click(|_,window,cx|{let mode=if cx.theme().is_dark(){ThemeMode::Light}else{ThemeMode::Dark};palette::change(mode,Some(window),cx);})))
                    .child(div().flex().items_center().justify_between().gap_3()
                        .child(badge(if self.proxy.is_some(){"Proxy aktywne"}else{"Proxy niedostępne"},if self.proxy.is_some(){theme.success}else{danger},theme.muted))
                        .child(div().text_xs().text_color(muted).child(refresh_text)))
                    .child(div().flex().items_center().justify_between().gap_3().pt_2()
                        .child(div().font_weight(FontWeight::MEDIUM).child(format!("Konta ({})",self.snapshot.accounts.len())))
                        .child(div().flex().items_center().gap_2()
                            .child(Button::new("refresh").small().ghost().label("Odśwież").disabled(self.busy||self.usage_loading).on_click(cx.listener(|this,_,_,cx|this.work("Odczytywanie…","Lista kont odświeżona.",|s|s.snapshot(),cx))))
                            .child(Button::new("import").small().label("Importuj z CLI").disabled(self.busy).on_click(cx.listener(|this,_,_,cx|this.work("Importowanie…","Konto zapisane.",|s|s.import_current(),cx))))
                            .child(Button::new("add").primary().label("Dodaj konto").disabled(self.busy).on_click(cx.listener(|this,_,_,cx|{this.cancel=Arc::new(AtomicBool::new(false));let cancel=this.cancel.clone();this.login_pending=true;this.work("Dokończ logowanie w przeglądarce.","Konto dodane.",move |s|launcher::login(&s,cancel),cx);})))) )
                    .when(!self.status.is_empty(),|d|d.child(div().flex().items_center().gap_3().p_3().rounded_md().bg(theme.muted)
                        .child(div().flex_1().min_w_0().text_color(if self.error{danger}else{muted}).child(self.status.clone()))
                        .when(self.login_pending,|d|d.child(Button::new("cancel").small().label("Anuluj logowanie").on_click(cx.listener(|this,_,_,cx|{this.cancel.store(true,Ordering::Relaxed);this.status="Anulowanie logowania…".into();cx.notify();}))))))
                    .child(div().flex().flex_col()
                        .when(rows.is_empty(),|d|d.child(div().py_8().flex().flex_col().gap_2().child(div().text_lg().child("Dodaj pierwsze konto")).child(div().text_color(muted).child("Zaloguj się na konto lub zaimportuj logowanie z CLI."))))
                        .children(rows))
                    .child(div().flex().flex_col().gap_3().p_4().rounded_lg().border_1().border_color(border)
                    .child(div().flex().items_center().justify_between().child(div().text_base().child("Połącz terminal z proxy")).child(Button::new("connection-details").ghost().label(if self.show_connection {"Zwiń"} else {"Pokaż instrukcję"}).on_click(cx.listener(|this,_,_,cx| {this.show_connection = !this.show_connection; cx.notify();}))))
                    .when(self.show_connection, |d| d
                    .child(div().text_color(muted).child("Skopiuj polecenie i uruchom je w katalogu projektu. Możesz dopisać resume, aby wznowić rozmowę."))
                    .child(div().flex().gap_3().items_center().child(div().flex_1().min_w_0().text_color(muted).child("codex-switch [resume]"))
                        .child(Button::new("copy").label("Kopiuj polecenie").disabled(self.proxy.is_none()||self.command.is_empty()).on_click(cx.listener(|this,_,_,cx|{cx.write_to_clipboard(ClipboardItem::new_string(this.command.to_string()));this.status="Polecenie skopiowane. Wklej je w terminalu w katalogu projektu.".into();this.error=false;cx.notify();}))))
                    .child(div().flex().items_center().justify_between().gap_3()
                        .child(div().text_color(muted).child(if self.snapshot.config_enabled {"Zwykłe codex korzysta z proxy"} else {"Opcjonalnie: włącz proxy dla zwykłego polecenia codex"}))
                        .child(Button::new("config").label(if self.snapshot.config_backup {"Przywróć config.toml"}else{"Włącz w config.toml"}).disabled(self.busy||self.proxy.is_none()).on_click(cx.listener(|this,_,window,cx| {
                            let restore=this.snapshot.config_backup;
                            let port=this.proxy.as_ref().unwrap().port;
                            let path=this.store.codex_home.join("config.toml").display().to_string();
                            let entity=cx.entity().downgrade();
                            let description=if restore {format!("Przywróci poprzedniego dostawcę modelu w {path} i usunie wpis proxy. Pozostałe zmiany w konfiguracji zostaną zachowane.")} else {format!("Zmieni domyślnego dostawcę modelu w {path} na lokalne proxy (127.0.0.1:{port}). Utworzy kopię konfiguracji. Nowe sesje zwykłego codex będą wymagały uruchomionego switchera. Profile i konfiguracja projektu mogą nadpisać ten wybór.")};
                            window.open_alert_dialog(cx,move |dialog,_,_| {
                                let entity=entity.clone();
                                dialog.title(if restore {"Przywrócić konfigurację?"}else{"Włączyć proxy domyślnie?"})
                                    .button_props(DialogButtonProps::default().ok_text(if restore {"Przywróć"}else{"Włącz proxy"}).show_cancel(true).cancel_text("Anuluj"))
                                    .child(description.clone()).on_ok(move |_,_,cx| {
                                        let _=entity.update(cx,|this,cx|this.work("Zapisywanie konfiguracji…","Konfiguracja zapisana. Zmiana obejmuje nowo uruchamiane sesje CLI.",move |store| {
                                            if restore {config::restore(&store)?;}else{config::enable(&store,port,&std::env::current_exe()?)?;}store.snapshot()
                                        },cx)); true
                                    })
                            });
                        }))))
                    .child(div().text_xs().text_color(muted).child("CLI uruchomiony wcześniej zwykłym poleceniem codex nie korzysta z proxy. Konto wyświetlane przez /status może pochodzić ze zwykłego logowania CLI."))))
                    .child(div().flex().justify_between().gap_3().text_xs().text_color(muted)
                        .child("Przełączenie konta dotyczy kolejnego żądania.")
                        .child(format!("{} żądań · {} w toku · {} błędów",self.counters.0,self.counters.1,self.counters.2)))
                ))
            .children(Root::render_sheet_layer(window,cx)).children(Root::render_dialog_layer(window,cx)).children(Root::render_notification_layer(window,cx))
    }
}
