//! Left column: new-lesson form and the saved-lesson library.

use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hil_core::model::{Depth, Lesson, Style};

use crate::app::HilApp;
use crate::ui;
use crate::ui::ToggleStyle as _;

pub const SIDEBAR_WIDTH: f32 = 300.;

impl HilApp {
    pub(crate) fn render_sidebar(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(self.render_form(cx))
            .child(self.render_library(cx))
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let style_buttons = Style::ALL.map(|style| {
            Button::new(SharedString::from(format!("style-{style}")))
                .label(style.label())
                .small()
                .toggled_style(self.style == style)
                .disabled(busy)
                .tooltip(style.blurb())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.style = style;
                    cx.notify();
                }))
        });
        let depth_buttons = Depth::ALL.map(|depth| {
            Button::new(SharedString::from(format!("depth-{depth}")))
                .label(depth.label())
                .small()
                .toggled_style(self.depth == depth)
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.depth = depth;
                    cx.notify();
                }))
        });
        let theme = cx.theme();
        v_flex()
            .gap_3()
            .p_4()
            .border_b_1()
            .border_color(theme.sidebar_border)
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .child("Honey Interactive Learn"),
            )
            .child(Input::new(&self.url_input).disabled(busy))
            .child(ui::caption("Style", cx))
            .child(h_flex().gap_1().flex_wrap().children(style_buttons))
            .child(ui::caption("Depth", cx))
            .child(h_flex().gap_1().flex_wrap().children(depth_buttons))
            .child(
                Button::new("learn")
                    .primary()
                    .label(if busy { "Working…" } else { "Learn" })
                    .loading(busy)
                    .disabled(busy)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.learn(cx))),
            )
            .when_some(self.job_status(), |el, status| {
                el.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(Spinner::new().small())
                        .child(div().flex_1().child(status.to_string())),
                )
            })
            .when_some(self.error.clone(), |el, error| {
                el.child(div().text_sm().text_color(theme.danger).child(error))
            })
    }

    fn render_library(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<AnyElement> = self
            .lessons
            .iter()
            .map(|lesson| self.render_item(lesson, cx).into_any_element())
            .collect();
        let empty = items.is_empty();
        v_flex()
            .flex_1()
            .min_h_0()
            .child(div().px_4().pt_3().pb_1().child(ui::caption("Library", cx)))
            .child(
                v_flex()
                    .id("library-list")
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pb_2()
                    .gap_1()
                    .overflow_y_scrollbar()
                    .when(empty, |el| {
                        el.child(div().px_2().child(ui::muted("No lessons yet.", cx)))
                    })
                    .children(items),
            )
    }

    fn render_item(&self, lesson: &Lesson, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let selected = self.selected.as_deref() == Some(lesson.id.as_str());
        let mut meta = format!(
            "{} · {} · {}",
            lesson.source.kind, lesson.style, lesson.depth
        );
        if let Some(best) = lesson.best_score() {
            meta.push_str(&format!(" · best {}/{}", best.score, best.total));
        }
        let select_id = lesson.id.clone();
        let delete_id = lesson.id.clone();
        h_flex()
            .id(SharedString::from(format!("item-{}", lesson.id)))
            .gap_2()
            .px_2()
            .py_2()
            .rounded(theme.radius)
            .cursor_pointer()
            .when(selected, |el| {
                el.bg(theme.sidebar_accent)
                    .text_color(theme.sidebar_accent_foreground)
            })
            .when(!selected, |el| el.hover(|s| s.bg(theme.list_hover)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select(&select_id);
                cx.notify();
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .line_clamp(2)
                            .child(lesson.content.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(meta),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("delete-{}", lesson.id)))
                    .icon(IconName::Delete)
                    .ghost()
                    .xsmall()
                    .tooltip("Delete lesson")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.delete_lesson(&delete_id, cx);
                    })),
            )
    }
}
