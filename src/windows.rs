//! The menu-bar panel and settings share one long-lived application model.
use crate::{Switcher, dock, tray, ui};
use gpui_kit::{
    component::{Root, Theme, WindowExt},
    *,
};

pub struct Windows {
    state: Entity<Switcher>,
    panel: Option<WindowHandle<Root>>,
    panel_host: Option<Entity<PanelHost>>,
    settings: Option<WindowHandle<Root>>,
    settings_host: Option<Entity<SettingsHost>>,
    dismissed_at: Option<std::time::Instant>,
}
impl Global for Windows {}

pub fn install(state: Entity<Switcher>, cx: &mut App) {
    cx.set_global(Windows {
        state,
        panel: None,
        panel_host: None,
        settings: None,
        settings_host: None,
        dismissed_at: None,
    });
}

pub fn close_panel(cx: &mut App) {
    let handle = cx.global::<Windows>().panel;
    let host = cx.global::<Windows>().panel_host.clone();
    if let (Some(handle), Some(host)) = (handle, host) {
        // Updating WindowHandle<Root> borrows Root for the whole callback.
        // host.close() clears dialogs through Root::update, so use the window
        // handle without borrowing its root entity first.
        let handle: AnyWindowHandle = handle.into();
        let _ = handle.update(cx, |_, window, cx| {
            host.update(cx, |host, cx| host.close(window, cx));
        });
    }
}

pub fn toggle_panel(cx: &mut App) {
    // AppKit can deactivate the panel before delivering the status-item click.
    if cx
        .global::<Windows>()
        .dismissed_at
        .is_some_and(|at| at.elapsed().as_millis() < 250)
    {
        return;
    }
    if cx
        .global::<Windows>()
        .panel_host
        .as_ref()
        .is_some_and(|host| host.read(cx).present)
    {
        close_panel(cx);
    } else {
        show_panel(cx);
    }
}

/// Clamp the panel to the usable display, including off-origin secondary monitors.
fn panel_bounds(
    anchor: Bounds<Pixels>,
    visible: Bounds<Pixels>,
    width: f32,
    height: f32,
) -> Bounds<Pixels> {
    // Margins and tray anchoring are physical AppKit geometry.
    let margin = px(8.);
    let width = px(width).min(visible.size.width - margin * 2.);
    let height = px(height).min(visible.size.height - margin * 2.);
    let x = (anchor.center().x - width / 2.)
        .max(visible.left() + margin)
        .min(visible.right() - width - margin);
    let y = (anchor.bottom() + px(6.))
        .max(visible.top() + margin)
        .min(visible.bottom() - height - margin);
    Bounds::new(point(x, y), size(width, height))
}

pub fn show_panel(cx: &mut App) {
    let Some(windows) = cx.try_global::<Windows>() else {
        return;
    };
    if let Some(handle) = windows.panel
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        if let Some(host) = cx.global::<Windows>().panel_host.clone() {
            host.update(cx, |host, cx| {
                host.present = true;
                cx.notify();
            });
        }
        return;
    }
    let state = cx.global::<Windows>().state.clone();
    let anchor = tray::anchor(cx);
    let displays = cx.displays();
    let display = anchor
        .and_then(|a| displays.iter().find(|d| d.bounds().contains(&a.center())))
        .or_else(|| displays.first());
    let Some(display) = display else {
        return;
    };
    let visible = display.visible_bounds();
    let anchor = anchor.unwrap_or_else(|| {
        Bounds::new(
            point(visible.right() - px(40.), visible.top()),
            size(px(1.), px(1.)),
        )
    });
    // The first layout measures content and replaces this provisional height.
    let scale = Theme::global(cx).font_size.as_f32() / ui::tokens::BASE_FONT_SIZE;
    let bounds = panel_bounds(
        anchor,
        visible,
        ui::tokens::PANEL_WIDTH * scale,
        232. * scale,
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: Some(display.id()),
        titlebar: None,
        kind: WindowKind::PopUp,
        window_background: WindowBackgroundAppearance::Blurred,
        show: false,
        focus: false,
        inactive_frame_interval: None,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    // PopUp is an AppKit nonactivating panel. Activating the application here
    // races its key-window handoff when another app (e.g. Ghostty) has focus.
    match cx.open_window(options, |window, cx| {
        crate::panel_effects::opacity(window, if cx.reduce_motion() { 1. } else { 0. }, false);
        let view = cx.new(|cx| PanelHost::new(state, window, cx));
        cx.global_mut::<Windows>().panel_host = Some(view.clone());
        cx.new(|cx| {
            Root::new(view, window, cx)
                .bg(transparent_black())
                .bordered(false)
        })
    }) {
        Ok(handle) => {
            cx.global_mut::<Windows>().panel = Some(handle);
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Err(error) => eprintln!("Could not open the accounts panel: {error}"),
    }
}

/// Measure the list after layout, independently of its scroll viewport.
pub(crate) fn fit_panel_to_content(scroll: &ScrollHandle, window: &mut Window, cx: &App) {
    let Some(content) = scroll.bounds_for_item(0) else {
        return;
    };
    let bounds = window.bounds();
    let available = window
        .display(cx)
        .map(|display| display.visible_bounds().bottom() - bounds.top() - px(8.))
        .unwrap_or(px(ui::tokens::PANEL_MAX_HEIGHT));
    let chrome = bounds.size.height - scroll.bounds().size.height;
    let scale = window.rem_size().as_f32() / ui::tokens::BASE_FONT_SIZE;
    let height = fitted_panel_height(chrome + content.size.height, available, scale);
    let width = px(ui::tokens::PANEL_WIDTH * scale).min(
        window
            .display(cx)
            .map(|display| display.visible_bounds().size.width - px(16.))
            .unwrap_or(px(ui::tokens::PANEL_WIDTH * scale)),
    );
    if (height - bounds.size.height).abs() >= px(1.) || (width - bounds.size.width).abs() >= px(1.)
    {
        window.resize(size(width, height));
    }
}

fn fitted_panel_height(content: Pixels, available: Pixels, scale: f32) -> Pixels {
    content
        .ceil()
        .min(px(ui::tokens::PANEL_MAX_HEIGHT * scale))
        .min(available)
        .max(px(1.))
}

struct PanelHost {
    state: Entity<Switcher>,
    focus: FocusHandle,
    activation: PanelActivation,
    present: bool,
    opacity: f32,
    _activation: Subscription,
}

/// Initial/duplicate inactive notifications are not a loss of panel focus.
#[derive(Default)]
struct PanelActivation {
    active: bool,
}
impl PanelActivation {
    fn changed(&mut self, active: bool) -> bool {
        let lost_focus = self.active && !active;
        self.active = active;
        lost_focus
    }
}

impl PanelHost {
    fn new(state: Entity<Switcher>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let initial_activation = PanelActivation {
            active: window.is_window_active(),
        };
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if this.activation.changed(window.is_window_active()) {
                cx.global_mut::<Windows>().dismissed_at = Some(std::time::Instant::now());
                this.close(window, cx);
            }
        });
        Self {
            state,
            focus,
            activation: initial_activation,
            present: true,
            opacity: if cx.reduce_motion() { 1. } else { 0. },
            _activation: activation,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.present {
            return;
        }
        self.present = false;
        // Sensitive dialog content must not linger through the exit animation.
        window.close_all_dialogs(cx);
        crate::panel_effects::opacity(window, self.opacity, true);
        cx.notify();
    }
}
impl Render for PanelHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let motion = gpui_kit::base::Presence::new("panel-presence", self.present)
            .transition(ui::tokens::panel_transition(self.present))
            .sample(window, cx);
        self.opacity = motion.progress;
        crate::panel_effects::opacity(window, motion.progress, !self.present);
        if !motion.should_render() {
            window.defer(cx, |window, cx| {
                let closing = cx
                    .global::<Windows>()
                    .panel_host
                    .as_ref()
                    .is_some_and(|host| !host.read(cx).present);
                if closing
                    && cx.global::<Windows>().panel.is_some_and(|handle| {
                        handle.window_id() == window.window_handle().window_id()
                    })
                {
                    cx.global_mut::<Windows>().panel = None;
                    cx.global_mut::<Windows>().panel_host = None;
                    window.remove_window();
                }
            });
        }
        div()
            .id("accounts-panel")
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "escape" && !window.has_active_dialog(cx) {
                    cx.defer(close_panel);
                    cx.stop_propagation();
                }
            })
            .child(self.state.clone())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SettingsSection {
    Accounts,
    Connection,
}

pub fn show_settings(cx: &mut App) {
    open_settings(None, cx);
}

pub(crate) fn show_settings_section(section: SettingsSection, cx: &mut App) {
    open_settings(Some(section), cx);
}

fn open_settings(section: Option<SettingsSection>, cx: &mut App) {
    let Some((state, settings, settings_host)) = cx.try_global::<Windows>().map(|windows| {
        (
            windows.state.clone(),
            windows.settings,
            windows.settings_host.clone(),
        )
    }) else {
        return;
    };
    if matches!(section, Some(SettingsSection::Connection)) {
        state.update(cx, |state, cx| {
            state.show_connection = true;
            cx.notify();
        });
    }
    if let Some(handle) = settings
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        if let (Some(host), Some(section)) = (settings_host, section) {
            host.update(cx, |host, cx| host.request(section, cx));
        }
        dock::show();
        cx.activate(true);
        close_panel(cx);
        return;
    }
    let scale = Theme::global(cx).font_size.as_f32() / ui::tokens::BASE_FONT_SIZE;
    let bounds = Bounds::centered(None, size(px(740. * scale), px(660. * scale)), cx);
    dock::show();
    cx.activate(true);
    match cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(680. * scale), px(560. * scale))),
            titlebar: Some(TitlebarOptions {
                title: Some("Codex Sub Switcher — Settings".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            window.on_window_should_close(cx, |_, cx| {
                cx.global_mut::<Windows>().settings = None;
                cx.global_mut::<Windows>().settings_host = None;
                if tray::available(cx) {
                    dock::hide();
                }
                true
            });
            let view = cx.new(|cx| {
                let subscription = cx.observe(&state, |_, _, cx| cx.notify());
                SettingsHost {
                    state,
                    scroll: ScrollHandle::new(),
                    requested_section: section,
                    _subscription: subscription,
                }
            });
            cx.global_mut::<Windows>().settings_host = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx))
        },
    ) {
        Ok(handle) => {
            cx.global_mut::<Windows>().settings = Some(handle);
            close_panel(cx);
        }
        Err(error) => eprintln!("Could not open settings: {error}"),
    }
}

struct SettingsHost {
    state: Entity<Switcher>,
    scroll: ScrollHandle,
    requested_section: Option<SettingsSection>,
    _subscription: Subscription,
}
impl SettingsHost {
    fn request(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.requested_section = Some(section);
        cx.notify();
    }
}
impl Render for SettingsHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(section) = self.requested_section.take() {
            self.scroll.scroll_to_top_of_item(match section {
                SettingsSection::Accounts => 2,
                SettingsSection::Connection => 4,
            });
        }
        self.state.update(cx, |state, cx| {
            state.settings_view(&self.scroll, window, cx)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PanelActivation, fitted_panel_height, panel_bounds};
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn panel_fits_content_until_height_or_display_limit() {
        assert_eq!(fitted_panel_height(px(180.), px(900.), 1.), px(180.));
        assert_eq!(fitted_panel_height(px(410.), px(900.), 1.), px(410.));
        assert_eq!(fitted_panel_height(px(700.), px(900.), 1.), px(538.));
        assert_eq!(fitted_panel_height(px(700.), px(320.), 1.), px(320.));
        assert_eq!(fitted_panel_height(px(180.), px(320.), 1.), px(180.));
        assert_eq!(
            fitted_panel_height(px(900.), px(1200.), 18. / 14.),
            px(538. * 18. / 14.)
        );
    }

    #[test]
    fn opening_from_another_app_ignores_initial_inactive_notifications() {
        let mut panel = PanelActivation::default();
        assert!(!panel.changed(false));
        assert!(!panel.changed(false));
        assert!(!panel.changed(true));
        assert!(!panel.changed(true));
        assert!(panel.changed(false));
        assert!(!panel.changed(false));
    }

    #[test]
    fn panel_already_active_at_creation_dismisses_on_first_blur() {
        let mut panel = PanelActivation { active: true };
        assert!(panel.changed(false));
    }

    #[test]
    fn panel_stays_on_secondary_display_at_either_edge() {
        let visible = Bounds::new(point(px(-1440.), px(24.)), size(px(1440.), px(876.)));
        for x in [-1430., -20.] {
            let anchor = Bounds::new(point(px(x), px(0.)), size(px(20.), px(24.)));
            let panel = panel_bounds(anchor, visible, 460., 640.);
            assert!(panel.left() >= visible.left());
            assert!(panel.right() <= visible.right());
            assert!(panel.top() >= visible.top());
            assert!(panel.bottom() <= visible.bottom());
        }
    }

    #[test]
    fn panel_shrinks_to_short_display() {
        let visible = Bounds::new(point(px(0.), px(24.)), size(px(800.), px(500.)));
        let panel = panel_bounds(
            Bounds::new(point(px(700.), px(0.)), size(px(40.), px(24.))),
            visible,
            460.,
            640.,
        );
        assert_eq!(panel.size.height, px(484.));
        assert!(panel.bottom() <= visible.bottom());
    }
}
