//! Terminal connection instructions and the existing config confirmation flow.
use super::*;

impl Switcher {
    pub(super) fn connection_panel(&self, cx: &Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let content = stack().pt_3()
            .child(div().text_color(muted).child("These options connect new sessions in every terminal that uses this proxy. Account changes apply from the next request; requests already in progress keep their account."))
            .child(div().text_color(muted).child("Copy the command and run it in your project directory. Add resume to continue a conversation."))
            .child(div().flex().gap_3().items_center()
                .child(div().flex_1().min_w_0().text_color(muted).child("codex-switch [resume]"))
                .child(action("copy", "Copy command")
                    .disabled(self.proxy.is_none() || self.command.is_empty() || !self.show_connection)
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(this.command.to_string()));
                        this.status = "Command copied. Paste it into a terminal in your project directory.".into();
                        this.error = false;
                        cx.notify();
                    }))))
            .child(div().flex().items_center().justify_between().gap_3()
                .child(div().flex_1().min_w_0().text_color(muted).child(if self.snapshot.config_enabled {
                    "The codex command uses the proxy"
                } else { "Optional: enable the proxy for the codex command" }))
                .child(action("config", if self.snapshot.config_backup { "Restore config.toml…" } else { "Enable in config.toml…" })
                    .disabled(self.busy || self.proxy.is_none() || !self.show_connection)
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_config(window, cx)))))
            .child(div().text_xs().text_color(muted).child("CLI sessions started earlier with the codex command do not use the proxy. The account shown by /status may come from the CLI's own credentials."));
        panel(cx)
            .gap_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .text_base()
                            .child("Connect your terminal to the proxy"),
                    )
                    .child(
                        action(
                            "connection-details",
                            if self.show_connection {
                                "Collapse"
                            } else {
                                "Show instructions"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_connection = !this.show_connection;
                            cx.notify();
                        })),
                    ),
            )
            .child(Disclosure::new(
                "connection-reveal",
                self.show_connection,
                content,
            ))
    }

    fn confirm_config(&self, window: &mut Window, cx: &mut Context<Self>) {
        let restore = self.snapshot.config_backup;
        let Some(proxy) = self.proxy.as_ref() else {
            return;
        };
        let port = proxy.port;
        let path = self
            .store
            .codex_home
            .join("config.toml")
            .display()
            .to_string();
        let entity = cx.entity().downgrade();
        let description = if restore {
            format!(
                "Restore the previous model provider in {path} and remove the proxy entry. Other configuration changes will be preserved."
            )
        } else {
            format!(
                "Change the default model provider in {path} to the local proxy (127.0.0.1:{port}) and back up the configuration. New codex sessions will require the switcher to be running. Profiles and project configuration can override this setting."
            )
        };
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            dialog
                .title(if restore {
                    "Restore configuration?"
                } else {
                    "Enable the proxy by default?"
                })
                .button_props(dialog_actions(
                    if restore { "Restore" } else { "Enable proxy" },
                    true,
                ))
                .child(description.clone())
                .on_ok(move |_, _, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.work(
                            "Saving configuration…",
                            "Configuration saved. The change applies to new CLI sessions.",
                            move |store| {
                                if restore {
                                    config::restore(&store)?;
                                } else {
                                    config::enable(&store, port, &std::env::current_exe()?)?;
                                }
                                store.snapshot()
                            },
                            cx,
                        )
                    });
                    true
                })
        });
    }
}
