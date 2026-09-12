//! Small style compositions; callers retain layout and event ownership.
use super::*;
use gpui_kit::component::{IconName, Size, button::ButtonVariant};

pub(super) fn action(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .secondary()
        .with_size(Size::Size(px(tokens::CONTROL_HEIGHT)))
        .h(px(tokens::CONTROL_HEIGHT))
        .px(px(tokens::CONTROL_PADDING))
        .text_size(px(tokens::TEXT_BODY))
        .rounded(px(tokens::RADIUS))
}

pub(super) fn primary_action(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    action(id, label).primary()
}

pub(super) fn icon_action(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .icon(icon)
        .ghost()
        .with_size(Size::Size(px(tokens::ICON_CONTROL_HEIGHT)))
        .rounded(px(tokens::RADIUS))
        .tooltip(label.clone())
        .accessibility_label(label)
}

pub(super) fn dialog_actions(ok: impl Into<SharedString>, cancel: bool) -> DialogButtonProps {
    DialogButtonProps::default()
        .ok_text(ok)
        .cancel_text("Cancel")
        .cancel_variant(ButtonVariant::Secondary)
        .show_cancel(cancel)
}

pub(super) fn badge(text: impl Into<SharedString>, color: Hsla, background: Hsla) -> Div {
    div()
        .px(px(tokens::SPACE_INLINE))
        .py_0p5()
        .rounded(px(tokens::RADIUS))
        .text_size(px(tokens::TEXT_CAPTION))
        .text_color(color)
        .bg(background)
        .child(text.into())
}

pub(super) fn stack() -> Div {
    div().flex().flex_col().gap(px(tokens::SPACE_CONTENT))
}

pub(super) fn panel(cx: &App) -> Div {
    stack()
        .p(px(tokens::SPACE_SECTION))
        .rounded(px(tokens::PANEL_RADIUS))
        .border_1()
        .border_color(cx.theme().border)
}

/// Stable keyed progress supports reversals, dynamic height, and Reduce Motion.
/// Padding belongs to the content, so the closed region occupies no space.
#[derive(IntoElement)]
pub(super) struct Disclosure {
    id: SharedString,
    open: bool,
    content: AnyElement,
}

impl Disclosure {
    pub(super) fn new(id: impl Into<SharedString>, open: bool, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            open,
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let progress = gpui_kit::base::transition(
            (ElementId::from(self.id.clone()), "disclosure"),
            if self.open { 1.0_f32 } else { 0.0_f32 },
            tokens::disclosure_transition(),
            window,
            cx,
        );
        gpui_kit::base::Collapsible::new()
            .open(self.open)
            .reveal(self.id, progress)
            .content(self.content)
            .flex()
            .flex_col()
    }
}
