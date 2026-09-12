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
pub const PANEL_WIDTH: f32 = 460.;
pub const PANEL_MAX_HEIGHT: f32 = 538.;
// A theme-specific scrim keeps text legible over contrasting desktop content.
pub const GLASS_TINT_LIGHT_ALPHA: f32 = 0.52;
pub const GLASS_TINT_DARK_ALPHA: f32 = 0.60;
pub const GLASS_CARD_ALPHA: f32 = 0.22;
pub const PANEL_ENTER: Duration = Duration::from_millis(250);
pub const PANEL_EXIT: Duration = Duration::from_millis(150);

pub fn panel_transition(open: bool) -> gpui_kit::base::Transition {
    gpui_kit::base::Transition::new(if open { PANEL_ENTER } else { PANEL_EXIT }).easing(
        gpui_kit::base::Easing::cubic_bezier(0.22, 1., 0.36, 1.).expect("valid product easing"),
    )
}

/// Accordion expand/collapse from transitions.dev, mapped to native GPUI.
pub const DISCLOSURE_DURATION: Duration = Duration::from_millis(250);

pub fn disclosure_transition() -> gpui_kit::base::Transition {
    gpui_kit::base::Transition::new(DISCLOSURE_DURATION).easing(
        gpui_kit::base::Easing::cubic_bezier(0.22, 1., 0.36, 1.).expect("valid product easing"),
    )
}
