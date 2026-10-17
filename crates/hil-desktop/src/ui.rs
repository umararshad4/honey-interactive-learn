//! Small presentational helpers shared by the views.

use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme, IconName, Sizable as _, v_flex};
use gpui_kit::*;

/// Small uppercase label above a group of controls.
pub fn caption(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(text.into().to_uppercase())
}

pub fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub fn heading(text: impl Into<SharedString>) -> Div {
    div().text_base().font_semibold().child(text.into())
}

/// Bordered block used for cards, snippets and Q&A items.
pub fn card(cx: &App) -> Div {
    v_flex()
        .gap_2()
        .p_4()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

/// Monospace block that scrolls sideways instead of wrapping.
pub fn mono_block(id: impl Into<ElementId>, text: &str, cx: &App) -> impl IntoElement {
    div()
        .id(id.into())
        .w_full()
        .overflow_x_scroll()
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        .font_family(cx.theme().mono_font_family.clone())
        .text_sm()
        .child(div().whitespace_nowrap().children(text.lines().map(|line| {
            div().child(if line.is_empty() {
                " ".to_string()
            } else {
                line.to_string()
            })
        })))
}

/// Ghost button that copies `text` to the clipboard.
pub fn copy_button(id: impl Into<ElementId>, label: &'static str, text: String) -> Button {
    Button::new(id)
        .icon(IconName::Copy)
        .label(label)
        .ghost()
        .small()
        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

pub fn empty_note(text: &'static str, cx: &App) -> Div {
    div().py_6().child(muted(text, cx))
}

/// Picker buttons: filled when chosen, outlined otherwise.
pub trait ToggleStyle {
    fn toggled_style(self, on: bool) -> Self;
}

impl ToggleStyle for Button {
    fn toggled_style(self, on: bool) -> Self {
        if on { self.primary() } else { self.outline() }
    }
}
