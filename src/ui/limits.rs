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
    let budget = limit.weekly_budget(now);
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
            div()
                .relative()
                .child(
                    Progress::new(SharedString::from(format!("usage-{account_id}-{index}")))
                        .small()
                        .value(remaining)
                        .accessibility_label(format!(
                            "{}: {:.0}% remaining",
                            limit.label(),
                            remaining
                        ))
                        .color(color),
                )
                .when_some(budget, |bar, expected| {
                    bar.children((1..7).map(|day| {
                        div()
                            .absolute()
                            .left(relative(day as f32 / 7.))
                            .top_0()
                            .w(px(1.))
                            .h_full()
                            .bg(theme.foreground.opacity(0.55))
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(relative(expected / 100.))
                            .top(px(-2.))
                            .w(px(2.))
                            .h(px(10.))
                            .bg(theme.foreground),
                    )
                }),
        )
        .when_some(budget.zip(reset), |view, (expected, reset)| {
            let difference = remaining - expected;
            let status = if difference < -1. {
                format!("Fast usage · {:.0} pp below budget", -difference)
            } else if difference > 1. {
                format!("Within budget · {:.0} pp spare", difference)
            } else {
                "On budget".into()
            };
            view.child(
                div()
                    .relative()
                    .h(px(18.))
                    .text_color(theme.muted_foreground)
                    .child(div().absolute().left_0().child("Reset"))
                    .children((1..7).map(|day| {
                        let at = reset - chrono::Duration::days(day);
                        div()
                            .absolute()
                            .left(relative(day as f32 / 7.))
                            .ml(px(-11.))
                            .child(at.with_timezone(&chrono::Local).format("%a").to_string())
                    })),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(tokens::SPACE_INLINE))
                    .child(div().text_color(theme.foreground).child(status))
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(format!("Now: {expected:.0}% budget")),
                    ),
            )
        })
}
