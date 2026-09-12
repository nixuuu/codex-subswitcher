//! Page composition. Components own presentation; Switcher owns application state.
use super::*;
use gpui_kit::component::Sizable;
mod account;
mod chrome;
mod components;
mod connection;
mod limits;
pub(crate) mod tokens;
use components::*;

impl Render for Switcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self
            .snapshot
            .accounts
            .iter()
            .map(|a| self.account_row(a, cx))
            .collect::<Vec<_>>();
        let scroll = self.page_scroll.clone();
        div()
            .size_full()
            .relative()
            .bg(cx.theme().background.opacity(if cx.theme().is_dark() {
                tokens::GLASS_TINT_DARK_ALPHA
            } else {
                tokens::GLASS_TINT_LIGHT_ALPHA
            }))
            .text_color(cx.theme().foreground)
            .font_family(tokens::FONT_FAMILY)
            .text_size(px(tokens::TEXT_BODY))
            .flex()
            .flex_col()
            .child(
                stack()
                    .flex_shrink_0()
                    .p(px(tokens::SPACE_SECTION))
                    .gap(px(tokens::SPACE_CONTENT))
                    .child(self.panel_header(cx))
                    .when(!self.status.is_empty() && (self.error || self.busy), |d| {
                        d.child(self.operation_status(cx))
                    }),
            )
            .child(
                div()
                    .id("page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.page_scroll)
                    .child(
                        stack()
                            .flex_shrink_0()
                            .px(px(tokens::SPACE_SECTION))
                            .pb(px(tokens::SPACE_SECTION))
                            .gap(px(tokens::SPACE_INLINE))
                            .when(rows.is_empty(), |d| d.child(self.empty_accounts(cx)))
                            .children(rows),
                    ),
            )
            .child(self.panel_footer(cx).flex_shrink_0())
            .child(
                canvas(
                    move |_, window, cx| {
                        window.defer(cx, move |window, cx| {
                            windows::fit_panel_to_content(&scroll, window, cx);
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

impl Switcher {
    pub(crate) fn settings_view(
        &self,
        scroll: &ScrollHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(tokens::FONT_FAMILY)
            .text_size(px(tokens::TEXT_BODY))
            .child(
                div()
                    .id("settings-page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .child(
                        stack()
                            .p(px(tokens::PAGE_PADDING))
                            .gap(px(tokens::SPACE_SECTION))
                            .child(self.app_header(cx))
                            .child(self.service_status(cx))
                            .child(self.settings_accounts(cx))
                            .when(!self.status.is_empty(), |d| {
                                d.child(self.operation_status(cx))
                            })
                            .child(self.connection_panel(cx))
                            .child(self.app_footer(cx)),
                    ),
            )
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
