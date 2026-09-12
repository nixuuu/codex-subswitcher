//! Reusable capacity meter; no application-state or request ownership.
use super::*;

pub(super) fn meter(
    account_id: &str,
    index: usize,
    limit: &usage::Window,
    now: chrono::DateTime<chrono::Utc>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let remaining = limit.remaining_percent();
    let color = if remaining <= 10.0 {
        theme.danger
    } else {
        theme.success
    };
    let reset = limit
        .reset_at
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    let reset_text = match reset {
        Some(at) if at > now => format!(
            "{}",
            at.with_timezone(&chrono::Local).format("%b %-d · %H:%M")
        ),
        Some(_) => "Waiting for reset".into(),
        None => "Reset time unavailable".into(),
    };
    stack()
        .min_w_0()
        .gap_0p5()
        .text_size(px(tokens::TEXT_CAPTION))
        .child(
            div()
                .flex()
                .justify_between()
                .gap(px(tokens::SPACE_INLINE))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(limit.label()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.muted_foreground)
                        .child(reset_text),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(format!("{remaining:.0}% left")),
                ),
        )
        .child(
            Progress::new(SharedString::from(format!("usage-{account_id}-{index}")))
                .small()
                .value(remaining)
                .accessibility_label(format!("{}: {:.0}% remaining", limit.label(), remaining))
                .color(color),
        )
}
