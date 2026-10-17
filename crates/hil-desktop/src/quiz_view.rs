//! Multiple-choice quiz driven by `hil_core::quiz::QuizSession`.

use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, IconName, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hil_core::markdown::option_letter;
use hil_core::model::Lesson;
use hil_core::quiz::QuizSession;

use crate::app::HilApp;
use crate::ui;

impl HilApp {
    pub(crate) fn render_quiz(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let quiz = &self.view.quiz;
        if quiz.is_empty() {
            return ui::empty_note("This lesson has no quiz.", cx).into_any_element();
        }
        if quiz.is_finished() {
            return self.render_score(lesson, cx).into_any_element();
        }
        let Some(q) = quiz.current() else {
            return div().into_any_element();
        };
        let chosen = quiz.current_answer();
        let theme = cx.theme();
        let options = q.options.iter().enumerate().map(|(i, text)| {
            let (border, bg) = match chosen {
                Some(_) if i == q.answer_index => (theme.success, theme.success.opacity(0.15)),
                Some(c) if c == i => (theme.danger, theme.danger.opacity(0.15)),
                _ => (theme.border, theme.background),
            };
            h_flex()
                .id(("option", i))
                .w_full()
                .gap_3()
                .items_start()
                .p_3()
                .rounded(theme.radius)
                .border_1()
                .border_color(border)
                .bg(bg)
                .when(chosen.is_none(), |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.view.quiz.answer(i);
                            cx.notify();
                        }))
                })
                .child(
                    div()
                        .font_semibold()
                        .child(format!("{}.", option_letter(i))),
                )
                .child(div().flex_1().min_w_0().child(text.clone()))
        });
        let last = quiz.current_index() + 1 == quiz.len();
        v_flex()
            .gap_4()
            .child(ui::caption(
                format!("Question {} of {}", quiz.current_index() + 1, quiz.len()),
                cx,
            ))
            .child(div().text_lg().font_semibold().child(TextView::markdown(
                SharedString::from(format!("quiz-{}-{}", lesson.id, quiz.current_index())),
                q.question.clone(),
            )))
            .child(v_flex().gap_2().children(options))
            .when_some(chosen, |el, c| {
                let correct = c == q.answer_index;
                let verdict = if correct {
                    "Correct!".to_string()
                } else {
                    format!(
                        "Not quite: the answer is {}.",
                        option_letter(q.answer_index)
                    )
                };
                el.child(
                    ui::card(cx)
                        .child(
                            div()
                                .font_semibold()
                                .text_color(if correct { theme.success } else { theme.danger })
                                .child(verdict),
                        )
                        .when(!q.explanation.is_empty(), |el| {
                            el.child(q.explanation.clone())
                        }),
                )
                .child(
                    h_flex().child(
                        Button::new("quiz-next")
                            .primary()
                            .icon(IconName::ArrowRight)
                            .label(if last { "See score" } else { "Next" })
                            .on_click(cx.listener(|this, _, _, cx| this.next_question(cx))),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_score(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let quiz = &self.view.quiz;
        let (score, total) = (quiz.score(), quiz.len());
        let message = match score * 100 / total.max(1) {
            100 => "Perfect score!",
            70.. => "Nice work.",
            40.. => "Getting there; review the missed ones and retry.",
            _ => "Worth another read of the overview before retrying.",
        };
        let best = lesson.best_score().map(|b| {
            format!(
                "Best so far: {} / {} ({} attempts)",
                b.score,
                b.total,
                lesson.attempts.len()
            )
        });
        ui::card(cx)
            .items_center()
            .gap_3()
            .p_8()
            .child(ui::caption("Quiz complete", cx))
            .child(
                div()
                    .text_3xl()
                    .font_bold()
                    .child(format!("{score} / {total}")),
            )
            .child(div().child(message))
            .when_some(best, |el, b| el.child(ui::muted(b, cx)))
            .child(
                Button::new("quiz-retry")
                    .primary()
                    .icon(IconName::RotateCw)
                    .label("Retry")
                    .on_click(cx.listener(|this, _, _, cx| this.retry_quiz(cx))),
            )
    }

    fn next_question(&mut self, cx: &mut Context<Self>) {
        if !self.view.quiz.advance() && !self.view.quiz_saved {
            let attempt = self.view.quiz.attempt();
            if let Some(lesson) = self.selected_lesson_mut() {
                lesson.attempts.push(attempt);
            }
            self.save_selected();
            self.view.quiz_saved = true;
        }
        cx.notify();
    }

    fn retry_quiz(&mut self, cx: &mut Context<Self>) {
        let questions = self
            .selected_lesson()
            .map(|l| l.content.quiz.clone())
            .unwrap_or_default();
        self.view.quiz = QuizSession::new(questions);
        self.view.quiz_saved = false;
        cx.notify();
    }
}
