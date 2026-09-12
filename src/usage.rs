//! Subscription windows reported by the Codex usage endpoint, never inferred from plan names.
use anyhow::{Result, ensure};
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Usage {
    pub rate_limit: Option<Limits>,
    pub rate_limit_reset_credits: Option<ResetCredits>,
    #[serde(skip)]
    pub pending_reset: bool,
    #[serde(skip)]
    pub reset_details: Option<crate::resets::Credits>,
    #[serde(skip)]
    pub reset_details_error: Option<String>,
}
#[derive(Clone, Deserialize)]
pub struct ResetCredits {
    pub available_count: u32,
}
#[derive(Clone, Deserialize)]
pub struct Limits {
    pub primary_window: Option<Window>,
    pub secondary_window: Option<Window>,
}
#[derive(Clone, Deserialize)]
pub struct Window {
    pub used_percent: f32,
    pub limit_window_seconds: i64,
    pub reset_at: Option<i64>,
}
impl Usage {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        ensure!(value.get("rate_limit").is_some(), "Missing limit data.");
        let usage: Self = serde_json::from_value(value)?;
        for w in usage.windows() {
            ensure!(
                w.used_percent.is_finite()
                    && (0.0..=100.0).contains(&w.used_percent)
                    && w.limit_window_seconds > 0,
                "Invalid limit data."
            );
            ensure!(
                w.reset_at
                    .is_none_or(|t| DateTime::from_timestamp(t, 0).is_some()),
                "Invalid reset date."
            );
        }
        Ok(usage)
    }
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.rate_limit
            .iter()
            .flat_map(|r| r.primary_window.iter().chain(r.secondary_window.iter()))
    }

    /// Notify when remaining capacity returns to exactly 100%. Compare the
    /// reported usage to zero: subtracting tiny usage from 100 can round to 100.
    /// Reset deadlines and primary/secondary positions are not reset evidence.
    pub fn silent_resets_since(&self, previous: &Self) -> Vec<String> {
        if self.pending_reset || previous.pending_reset {
            return Vec::new();
        }
        self.windows()
            .filter(|window| {
                window.used_percent == 0.0
                    && previous.windows().any(|old| {
                        old.limit_window_seconds == window.limit_window_seconds
                            && old.used_percent > 0.0
                    })
            })
            .map(Window::label)
            .collect()
    }
}
impl Window {
    pub fn remaining_percent(&self) -> f32 {
        100.0 - self.used_percent
    }

    pub fn label(&self) -> String {
        match self.limit_window_seconds {
            18000 => "5h".into(),
            604800 => "Weekly".into(),
            86400 => "1 day".into(),
            s if s % 86400 == 0 => format!("{} days", s / 86400),
            s if s % 3600 == 0 => format!("{}h", s / 3600),
            s => format!("{} min", s / 60),
        }
    }
    pub fn reset_label(&self, now: DateTime<Utc>) -> String {
        let Some(at) = self.reset_at.and_then(|t| DateTime::from_timestamp(t, 0)) else {
            return "Reset time unavailable".into();
        };
        let remaining = (at - now).num_seconds();
        let date = at
            .with_timezone(&Local)
            .format("%b %-d %H:%M %Z")
            .to_string();
        if remaining <= 0 {
            return format!("Reset {date} · waiting for updated data");
        }
        let minutes = (remaining + 59) / 60;
        let relative = if minutes >= 1440 {
            format!("{}d {}h", minutes / 1440, minutes % 1440 / 60)
        } else {
            format!("{}h {}min", minutes / 60, minutes % 60)
        };
        format!("Resets in {relative} · {date}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn window(seconds: i64) -> serde_json::Value {
        json!({"used_percent":42.5,"limit_window_seconds":seconds,"reset_at":2000000000})
    }
    #[test]
    fn weekly_only_primary_is_not_five_hours() {
        let u = Usage::parse(
            &serde_json::to_vec(
                &json!({"rate_limit":{"primary_window":window(604800),"secondary_window":null}}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(u.windows().count(), 1);
        assert_eq!(u.windows().next().unwrap().label(), "Weekly");
    }
    #[test]
    fn both_windows_and_absent_limits() {
        let u = Usage::parse(&serde_json::to_vec(&json!({"rate_limit":{"primary_window":window(18000),"secondary_window":window(604800)}})).unwrap()).unwrap();
        assert_eq!(
            u.windows().map(Window::label).collect::<Vec<_>>(),
            ["5h", "Weekly"]
        );
        assert_eq!(
            Usage::parse(br#"{"rate_limit":null}"#)
                .unwrap()
                .windows()
                .count(),
            0
        );
        assert!(Usage::parse(br#"{}"#).is_err());
    }
    #[test]
    fn malformed_data_is_not_zero_usage() {
        for value in [
            json!({"limit_window_seconds":18000}),
            json!({"used_percent":-1,"limit_window_seconds":18000}),
            json!({"used_percent":101,"limit_window_seconds":18000}),
        ] {
            assert!(
                Usage::parse(
                    &serde_json::to_vec(&json!({"rate_limit":{"primary_window":value}})).unwrap()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn expired_window_is_not_assumed_reset() {
        let w = Window {
            used_percent: 100.,
            limit_window_seconds: 18000,
            reset_at: Some(1000),
        };
        assert!(
            w.reset_label(DateTime::from_timestamp(1001, 0).unwrap())
                .contains("waiting")
        );
    }

    fn reported(windows: &[(i64, f32)]) -> Usage {
        let window = |index: usize| {
            windows
                .get(index)
                .map(|&(seconds, used)| json!({"limit_window_seconds":seconds,"used_percent":used}))
        };
        Usage::parse(
            &serde_json::to_vec(&json!({"rate_limit":{
                "primary_window":window(0), "secondary_window":window(1)
            }}))
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn silent_reset_matches_window_duration_even_when_positions_change() {
        let previous = reported(&[(18000, 32.0), (604800, 89.0)]);
        let current = reported(&[(604800, 0.0), (18000, 0.0)]);
        assert_eq!(current.silent_resets_since(&previous), ["Weekly", "5h"]);
        assert_eq!(
            reported(&[(604800, 0.0)]).silent_resets_since(&previous),
            ["Weekly"]
        );
    }

    #[test]
    fn silent_reset_requires_reported_positive_to_exact_zero_transition() {
        let previous = reported(&[(18000, 32.0)]);
        for current in [
            reported(&[]),
            reported(&[(604800, 0.0)]),
            reported(&[(18000, 0.01)]),
            reported(&[(18000, f32::EPSILON)]),
            reported(&[(18000, 32.0)]),
            reported(&[(18000, 90.0)]),
        ] {
            assert!(current.silent_resets_since(&previous).is_empty());
        }
        let zero = reported(&[(18000, 0.0)]);
        assert!(zero.silent_resets_since(&zero).is_empty());
        assert!(zero.silent_resets_since(&reported(&[])).is_empty());
        assert_eq!(
            zero.silent_resets_since(&reported(&[(18000, 0.01)])),
            ["5h"]
        );
    }

    #[test]
    fn pending_manual_reset_is_not_a_silent_reset() {
        let mut previous = reported(&[(18000, 32.0)]);
        let mut current = reported(&[(18000, 0.0)]);
        previous.pending_reset = true;
        assert!(current.silent_resets_since(&previous).is_empty());
        previous.pending_reset = false;
        current.pending_reset = true;
        assert!(current.silent_resets_since(&previous).is_empty());
    }

    #[test]
    fn remaining_capacity_is_the_inverse_of_reported_usage() {
        for (used, remaining) in [(0.0, 100.0), (9.0, 91.0), (90.0, 10.0), (100.0, 0.0)] {
            let data = reported(&[(18000, used)]);
            assert_eq!(
                data.windows().next().unwrap().remaining_percent(),
                remaining
            );
        }
        let partially_used = reported(&[(18000, f32::EPSILON)]);
        assert_eq!(
            reported(&[(18000, 0.0)]).silent_resets_since(&partially_used),
            ["5h"]
        );
    }
}
