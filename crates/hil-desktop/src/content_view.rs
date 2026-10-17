//! Read-only lesson tabs: overview, diagram, code, examples, glossary, Q&A.

use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hil_core::model::Lesson;

use crate::app::HilApp;
use crate::ui;

fn markdown(id: String, text: impl Into<SharedString>) -> TextView {
    TextView::markdown(SharedString::from(id), text)
}

/// TL;DR, key points and sections as one Markdown document.
pub fn overview(lesson: &Lesson) -> impl IntoElement {
    let c = &lesson.content;
    let mut md = String::new();
    if !c.tldr.is_empty() {
        md.push_str(&format!("### TL;DR\n\n{}\n\n", c.tldr));
    }
    if !c.key_points.is_empty() {
        md.push_str("### Key points\n\n");
        for point in &c.key_points {
            md.push_str(&format!("- {point}\n"));
        }
        md.push('\n');
    }
    for section in &c.sections {
        md.push_str(&format!("### {}\n\n{}\n\n", section.heading, section.body));
    }
    if md.is_empty() {
        md.push_str("_This lesson has no summary._");
    }
    markdown(format!("overview-{}", lesson.id), md)
}

pub fn diagram(lesson: &Lesson, cx: &App) -> impl IntoElement {
    let d = &lesson.content.diagram;
    if d.ascii.trim().is_empty() && d.mermaid.trim().is_empty() {
        return ui::empty_note("This lesson has no diagram.", cx).into_any_element();
    }
    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_2()
                .justify_between()
                .child(ui::heading(if d.title.is_empty() { "Diagram".into() } else { d.title.clone() }))
                .when(!d.mermaid.trim().is_empty(), |el| {
                    el.child(ui::copy_button("copy-mermaid", "Copy Mermaid", d.mermaid.clone()))
                }),
        )
        .when(!d.ascii.trim().is_empty(), |el| {
            el.child(ui::mono_block("diagram-ascii", &d.ascii, cx))
        })
        .when(!d.mermaid.trim().is_empty(), |el| {
            el.child(ui::muted(
                "Paste the Mermaid source into GitHub, Obsidian or mermaid.live for a rendered chart.",
                cx,
            ))
        })
        .into_any_element()
}

pub fn code(lesson: &Lesson, cx: &App) -> impl IntoElement {
    let snippets = &lesson.content.code_snippets;
    if snippets.is_empty() {
        return ui::empty_note("This lesson has no code snippets.", cx).into_any_element();
    }
    v_flex()
        .gap_4()
        .children(snippets.iter().enumerate().map(|(i, s)| {
            ui::card(cx)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(ui::heading(s.title.clone())))
                        .when(!s.language.is_empty(), |el| {
                            el.child(Tag::secondary().small().child(s.language.clone()))
                        })
                        .child(ui::copy_button(("copy-code", i), "Copy", s.code.clone())),
                )
                .child(ui::mono_block(("code", i), &s.code, cx))
                .when(!s.explanation.is_empty(), |el| {
                    el.child(markdown(
                        format!("code-{}-{i}", lesson.id),
                        s.explanation.clone(),
                    ))
                })
        }))
        .into_any_element()
}

pub fn examples(lesson: &Lesson, cx: &App) -> impl IntoElement {
    let examples = &lesson.content.examples;
    if examples.is_empty() {
        return ui::empty_note("This lesson has no examples.", cx).into_any_element();
    }
    v_flex()
        .gap_4()
        .children(examples.iter().enumerate().map(|(i, e)| {
            ui::card(cx)
                .child(ui::heading(e.title.clone()))
                .child(markdown(
                    format!("example-{}-{i}", lesson.id),
                    e.description.clone(),
                ))
        }))
        .into_any_element()
}

pub fn glossary(lesson: &Lesson, cx: &App) -> impl IntoElement {
    let terms = &lesson.content.glossary;
    if terms.is_empty() {
        return ui::empty_note("This lesson has no glossary.", cx).into_any_element();
    }
    let theme = cx.theme();
    v_flex()
        .children(terms.iter().map(|t| {
            h_flex()
                .items_start()
                .gap_4()
                .py_3()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .w(px(200.))
                        .flex_shrink_0()
                        .font_semibold()
                        .child(t.term.clone()),
                )
                .child(div().flex_1().min_w_0().child(t.definition.clone()))
        }))
        .into_any_element()
}

impl HilApp {
    pub(crate) fn render_questions(
        &self,
        lesson: &Lesson,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let questions = &lesson.content.questions;
        if questions.is_empty() {
            return ui::empty_note("This lesson has no open questions.", cx).into_any_element();
        }
        v_flex()
            .gap_4()
            .children(questions.iter().enumerate().map(|(i, q)| {
                let open = self.view.revealed.contains(&i);
                ui::card(cx)
                    .child(ui::heading(format!("{}. {}", i + 1, q.question)))
                    .child(
                        h_flex().child(
                            Button::new(("reveal", i))
                                .label(if open { "Hide answer" } else { "Show answer" })
                                .small()
                                .outline()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.view.revealed.remove(&i) {
                                        this.view.revealed.insert(i);
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
                    .when(open, |el| {
                        el.child(markdown(
                            format!("answer-{}-{i}", lesson.id),
                            q.answer.clone(),
                        ))
                    })
            }))
            .into_any_element()
    }
}
