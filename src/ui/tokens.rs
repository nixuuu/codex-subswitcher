//! Product geometry and motion. Colors remain in `palette` / the active theme.
use std::time::Duration;

pub const FONT_FAMILY: &str = ".AppleSystemUIFont";
pub const TEXT_BODY: f32 = 14.;
pub const TEXT_CAPTION: f32 = 12.;
pub const TEXT_HEADING: f32 = 20.;
pub const CONTROL_HEIGHT: f32 = 32.;
pub const ICON_CONTROL_HEIGHT: f32 = 24.;
pub const CONTROL_PADDING: f32 = 12.;
pub const RADIUS: f32 = 6.;
pub const PANEL_RADIUS: f32 = 8.;
pub const SPACE_INLINE: f32 = 8.;
pub const SPACE_CONTENT: f32 = 12.;
pub const SPACE_SECTION: f32 = 16.;
pub const PAGE_PADDING: f32 = 20.;
pub const ACCOUNT_IDENTITY_WIDTH: f32 = 210.;
pub const RESET_SUMMARY_WIDTH: f32 = 174.;
pub const WIDE_LAYOUT: f32 = 900.;

/// Accordion expand/collapse from transitions.dev, mapped to native GPUI.
pub const DISCLOSURE_DURATION: Duration = Duration::from_millis(250);

pub fn disclosure_transition() -> gpui_kit::base::Transition {
    gpui_kit::base::Transition::new(DISCLOSURE_DURATION).easing(
        gpui_kit::base::Easing::cubic_bezier(0.22, 1., 0.36, 1.).expect("valid product easing"),
    )
}
