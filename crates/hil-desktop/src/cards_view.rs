//! Flashcards: one card at a time, click to flip.

use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use hil_core::model::Lesson;

use crate::app::HilApp;
use crate::ui;

impl HilApp {
    pub(crate) fn render_cards(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let cards = &lesson.content.flashcards;
        let total = cards.len();
        let Some(card) = cards.get(self.view.card.min(total.saturating_sub(1))) else {
            return ui::empty_note("This lesson has no flashcards.", cx).into_any_element();
        };
        let index = self.view.card.min(total - 1);
        let flipped = self.view.flipped;
        let theme = cx.theme();
        let face = v_flex()
            .id("flashcard")
            .w_full()
            .min_h(px(260.))
            .items_center()
            .justify_center()
            .gap_3()
            .p_8()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(if flipped { theme.primary } else { theme.border })
            .bg(if flipped {
                theme.accent
            } else {
                theme.background
            })
            .cursor_pointer()
            .hover(|s| s.border_color(theme.ring))
            .on_click(cx.listener(|this, _, _, cx| {
                this.view.flipped = !this.view.flipped;
                cx.notify();
            }))
            .child(ui::caption(if flipped { "Answer" } else { "Question" }, cx))
            .child(div().text_xl().text_center().child(if flipped {
                card.back.clone()
            } else {
                card.front.clone()
            }))
            .child(ui::muted("Click the card to flip it", cx));
        v_flex()
            .gap_4()
            .child(face)
            .child(
                h_flex()
                    .gap_3()
                    .justify_center()
                    .child(
                        Button::new("card-prev")
                            .icon(IconName::ChevronLeft)
                            .label("Previous")
                            .small()
                            .outline()
                            .disabled(index == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.view.card = index.saturating_sub(1);
                                this.view.flipped = false;
                                cx.notify();
                            })),
                    )
                    .child(div().text_sm().child(format!("{} / {}", index + 1, total)))
                    .child(
                        Button::new("card-next")
                            .icon(IconName::ChevronRight)
                            .label("Next")
                            .small()
                            .outline()
                            .disabled(index + 1 >= total)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.view.card = (index + 1).min(total - 1);
                                this.view.flipped = false;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
