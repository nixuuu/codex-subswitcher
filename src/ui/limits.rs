//! Reusable capacity meter; no application-state or request ownership.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BudgetStatus {
    Below,
    On,
    Within,
}

impl BudgetStatus {
    fn from_difference(difference: f32) -> Self {
        if difference < -1.0 {
            Self::Below
        } else if difference > 1.0 {
            Self::Within
        } else {
            Self::On
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Below => "Faster than weekly budget",
            Self::On => "On pace with weekly budget",
            Self::Within => "Slower than weekly budget",
        }
    }
}

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
    let reset = limit
        .reset_at
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0));
    let color = if remaining <= 10.0 {
        theme.danger
    } else {
        theme.success
    };
    let reset_text = limit
        .reset_at
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|at| {
            if at > now {
                at.with_timezone(&chrono::Local)
                    .format("%b %-d · %H:%M")
                    .to_string()
            } else {
                "Waiting for reset".into()
            }
        })
        .unwrap_or_else(|| "Reset time unavailable".into());

    stack()
        .min_w_0()
        .gap_0p5()
        .text_xs()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w_12()
                        .flex_shrink_0()
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
                        .w_16()
                        .flex_shrink_0()
                        .text_right()
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
                            .left(relative(day as f32 / 7.0))
                            .top_0()
                            // Physical hairline separates days on the capacity scale.
                            .w(px(1.0))
                            .h_full()
                            .bg(theme.foreground.opacity(0.55))
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(relative(expected / 100.0))
                            .top_0()
                            // Distinct physical marker for the current linear budget.
                            .w(px(2.0))
                            .h_2()
                            .bg(theme.foreground),
                    )
                }),
        )
        .when_some(budget.zip(reset), |view, (expected, reset)| {
            let difference = remaining - expected;
            view.child(
                div()
                    .relative()
                    .h_5()
                    .text_color(theme.muted_foreground)
                    .child(div().absolute().left_0().child("Reset"))
                    .children((1..7).map(|day| {
                        div()
                            .absolute()
                            // Center each label on its tick using the same fractional grid.
                            .left(relative((day as f32 - 0.5) / 7.0))
                            .w(relative(1.0 / 7.0))
                            .text_center()
                            .child(
                                (reset - chrono::Duration::days(day))
                                    .with_timezone(&chrono::Local)
                                    .format("%a")
                                    .to_string(),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_x_2()
                    .child(
                        div()
                            .text_color(if difference < -1.0 {
                                theme.warning
                            } else {
                                theme.foreground
                            })
                            .child(BudgetStatus::from_difference(difference).label()),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(format!("Now: {expected:.0}% · {difference:+.0} pp")),
                    ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::BudgetStatus;

    #[test]
    fn weekly_budget_status_keeps_one_point_neutral_band() {
        assert_eq!(BudgetStatus::from_difference(-1.01), BudgetStatus::Below);
        assert_eq!(BudgetStatus::from_difference(-1.0), BudgetStatus::On);
        assert_eq!(BudgetStatus::from_difference(1.0), BudgetStatus::On);
        assert_eq!(BudgetStatus::from_difference(1.01), BudgetStatus::Within);
    }
}
