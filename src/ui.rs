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
        let wide = window.viewport_size().width >= px(tokens::WIDE_LAYOUT);
        let rows = self
            .snapshot
            .accounts
            .iter()
            .map(|a| self.account_row(a, wide, cx))
            .collect::<Vec<_>>();
        div()
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(tokens::FONT_FAMILY)
            .text_size(px(tokens::TEXT_BODY))
            .child(
                div()
                    .id("page")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.page_scroll)
                    .child(
                        stack()
                            .p(px(tokens::PAGE_PADDING))
                            .gap(px(tokens::SPACE_SECTION))
                            .child(self.app_header(cx))
                            .child(self.service_status(cx))
                            .child(self.account_toolbar(cx))
                            .when(!self.status.is_empty(), |d| {
                                d.child(self.operation_status(cx))
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .when(rows.is_empty(), |d| d.child(self.empty_accounts(cx)))
                                    .children(rows),
                            )
                            .child(self.connection_panel(cx))
                            .child(self.app_footer(cx)),
                    ),
            )
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
