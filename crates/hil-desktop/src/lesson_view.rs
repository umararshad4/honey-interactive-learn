//! Main area: lesson header, restyle control and tab bar.

use std::collections::HashSet;

use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hil_core::model::{Depth, Lesson, Style};
use hil_core::quiz::QuizSession;

use crate::app::HilApp;
use crate::ui;
use crate::ui::ToggleStyle as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LessonTab {
    #[default]
    Overview,
    Diagram,
    Code,
    Examples,
    Flashcards,
    Quiz,
    Questions,
    Glossary,
}

impl LessonTab {
    pub const ALL: [LessonTab; 8] = [
        Self::Overview,
        Self::Diagram,
        Self::Code,
        Self::Examples,
        Self::Flashcards,
        Self::Quiz,
        Self::Questions,
        Self::Glossary,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Diagram => "Diagram",
            Self::Code => "Code",
            Self::Examples => "Examples",
            Self::Flashcards => "Flashcards",
            Self::Quiz => "Quiz",
            Self::Questions => "Q&A",
            Self::Glossary => "Glossary",
        }
    }

    /// Parses `quiz`, `Q&A`, `qa`, `cards`... (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "cards" => Some(Self::Flashcards),
            "qa" | "questions" => Some(Self::Questions),
            _ => Self::ALL
                .into_iter()
                .find(|t| t.label().to_ascii_lowercase() == s),
        }
    }
}

/// Per-lesson UI state, reset when another lesson is selected.
pub struct ViewState {
    pub tab: LessonTab,
    pub restyle_open: bool,
    pub restyle_style: Style,
    pub restyle_depth: Depth,
    pub quiz: QuizSession,
    pub quiz_saved: bool,
    pub card: usize,
    pub flipped: bool,
    pub revealed: HashSet<usize>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            tab: LessonTab::default(),
            restyle_open: false,
            restyle_style: Style::default(),
            restyle_depth: Depth::default(),
            quiz: QuizSession::new(Vec::new()),
            quiz_saved: false,
            card: 0,
            flipped: false,
            revealed: HashSet::new(),
        }
    }
}

impl ViewState {
    pub fn for_lesson(lesson: &Lesson, tab: LessonTab) -> Self {
        Self {
            tab,
            restyle_style: lesson.style,
            restyle_depth: lesson.depth,
            quiz: QuizSession::new(lesson.content.quiz.clone()),
            ..Self::default()
        }
    }
}

impl HilApp {
    pub(crate) fn render_main(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body = match self.selected_lesson() {
            Some(lesson) => v_flex()
                .size_full()
                .child(self.render_header(lesson, cx))
                .child(self.render_tabs(cx))
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "{}-{:?}",
                            lesson.id, self.view.tab
                        )))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .child(
                            v_flex()
                                .w_full()
                                .max_w(px(900.))
                                .p_6()
                                .gap_4()
                                .child(self.render_tab_content(lesson, cx)),
                        ),
                )
                .into_any_element(),
            None => render_empty(cx).into_any_element(),
        };
        div().flex_1().min_w_0().h_full().child(body)
    }

    fn render_tab_content(&self, lesson: &Lesson, cx: &mut Context<Self>) -> AnyElement {
        match self.view.tab {
            LessonTab::Overview => crate::content_view::overview(lesson).into_any_element(),
            LessonTab::Diagram => crate::content_view::diagram(lesson, cx).into_any_element(),
            LessonTab::Code => crate::content_view::code(lesson, cx).into_any_element(),
            LessonTab::Examples => crate::content_view::examples(lesson, cx).into_any_element(),
            LessonTab::Glossary => crate::content_view::glossary(lesson, cx).into_any_element(),
            LessonTab::Questions => self.render_questions(lesson, cx).into_any_element(),
            LessonTab::Flashcards => self.render_cards(lesson, cx).into_any_element(),
            LessonTab::Quiz => self.render_quiz(lesson, cx).into_any_element(),
        }
    }

    fn render_header(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let url = lesson.source.url.clone();
        let theme = cx.theme();
        let link = div()
            .id("source-link")
            .text_color(theme.link)
            .cursor_pointer()
            .hover(|s| s.underline())
            .child(lesson.source.url.clone())
            .on_click(move |_, _, cx| cx.open_url(&url));
        let author =
            (!lesson.source.author.is_empty()).then(|| format!("by {}", lesson.source.author));
        v_flex()
            .gap_2()
            .px_6()
            .pt_5()
            .pb_3()
            .child(
                div()
                    .text_2xl()
                    .font_bold()
                    .child(lesson.content.title.clone()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .text_sm()
                    .child(
                        Tag::secondary()
                            .small()
                            .child(lesson.source.kind.to_string()),
                    )
                    .child(Tag::primary().small().child(lesson.style.label()))
                    .child(Tag::secondary().small().child(lesson.depth.label()))
                    .child(link)
                    .when_some(author, |el, a| el.child(ui::muted(a, cx))),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("restyle")
                            .icon(IconName::RefreshCw)
                            .label("Restyle")
                            .small()
                            .toggled_style(self.view.restyle_open)
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view.restyle_open = !this.view.restyle_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("export")
                            .icon(IconName::FileText)
                            .label("Export Markdown")
                            .small()
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.export_markdown(cx))),
                    )
                    .when_some(self.notice.clone(), |el, n| el.child(ui::muted(n, cx))),
            )
            .when(self.view.restyle_open, |el| {
                el.child(self.render_restyle(lesson, cx))
            })
    }

    fn render_restyle(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let styles = Style::ALL.map(|style| {
            Button::new(SharedString::from(format!("restyle-{style}")))
                .label(style.label())
                .xsmall()
                .toggled_style(self.view.restyle_style == style)
                .tooltip(style.blurb())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.view.restyle_style = style;
                    cx.notify();
                }))
        });
        let depths = Depth::ALL.map(|depth| {
            Button::new(SharedString::from(format!("redepth-{depth}")))
                .label(depth.label())
                .xsmall()
                .toggled_style(self.view.restyle_depth == depth)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.view.restyle_depth = depth;
                    cx.notify();
                }))
        });
        let source = lesson.source.clone();
        ui::card(cx)
            .gap_3()
            .child(ui::muted(
                "Write a new lesson from the saved source text (no re-download).",
                cx,
            ))
            .child(h_flex().gap_1().flex_wrap().children(styles))
            .child(h_flex().gap_1().flex_wrap().children(depths))
            .child(
                h_flex().child(
                    Button::new("regenerate")
                        .primary()
                        .small()
                        .label("Write new lesson")
                        .loading(busy)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let (style, depth) = (this.view.restyle_style, this.view.restyle_depth);
                            this.view.restyle_open = false;
                            this.restyle(source.clone(), style, depth, cx);
                        })),
                ),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = LessonTab::ALL
            .iter()
            .position(|t| *t == self.view.tab)
            .unwrap_or(0);
        div().px_6().child(
            TabBar::new("lesson-tabs")
                .underline()
                .selected_index(selected)
                .children(LessonTab::ALL.map(|t| Tab::new().label(t.label())))
                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                    this.view.tab = LessonTab::ALL[*ix];
                    cx.notify();
                })),
        )
    }
}

fn render_empty(cx: &App) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .p_8()
        .child(
            div()
                .text_2xl()
                .font_bold()
                .child("Learn anything from a link"),
        )
        .child(ui::muted(
            "1. Paste a YouTube video or an X post link on the left.",
            cx,
        ))
        .child(ui::muted(
            "2. Pick a style and depth, then press Learn (or Enter).",
            cx,
        ))
        .child(ui::muted(
            "3. Read the summary, study the diagram and flashcards, then take the quiz.",
            cx,
        ))
        .child(ui::muted("Saved lessons appear in the library.", cx))
}
