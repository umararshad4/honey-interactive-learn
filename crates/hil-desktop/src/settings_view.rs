//! Settings screen: AI provider, API keys, models and effort.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hil_core::llm::{BackendChoice, BackendConfig};
use hil_core::settings::Settings;

use crate::settings_form::{self, EFFORTS, PROVIDERS, ProviderSpec, env_source};
use crate::ui;
use crate::ui::ToggleStyle as _;

pub enum SettingsEvent {
    Close,
}

struct ProviderInputs {
    spec: &'static ProviderSpec,
    key: Option<Entity<InputState>>,
    model: Entity<InputState>,
}

pub struct SettingsView {
    base: Settings,
    backend: BackendChoice,
    effort: Option<&'static str>,
    providers: Vec<ProviderInputs>,
    /// What Auto resolves to with the saved settings, or why it cannot.
    auto_status: Result<String, String>,
    status: Option<Result<String, String>>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn auto_status() -> Result<String, String> {
    let mut cfg = BackendConfig::load();
    cfg.choice = BackendChoice::Auto;
    cfg.resolve()
        .map(|c| c.title().to_string())
        .map_err(|e| format!("{e:#}"))
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (base, load_error) = match Settings::load() {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(format!("{e:#}"))),
        };
        let providers = PROVIDERS
            .iter()
            .map(|spec| {
                let key = spec.key_field.map(|field| {
                    let value = settings_form::saved_value(&base, field);
                    cx.new(|cx| {
                        InputState::new(window, cx)
                            .masked(true)
                            .placeholder("Not set")
                            .default_value(value)
                    })
                });
                let model_value = settings_form::saved_value(&base, spec.model_field);
                let placeholder = format!("Default: {}", spec.default_model);
                let model = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .default_value(model_value)
                });
                ProviderInputs { spec, key, model }
            })
            .collect();
        let effort = base
            .effort
            .as_deref()
            .and_then(|e| EFFORTS.into_iter().flatten().find(|x| *x == e));
        Self {
            backend: base.backend,
            effort,
            base,
            providers,
            auto_status: auto_status(),
            status: load_error.map(Err),
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let mut fields = Vec::new();
        for p in &self.providers {
            if let (Some(field), Some(key)) = (p.spec.key_field, &p.key) {
                fields.push((field, key.read(cx).value().to_string()));
            }
            fields.push((p.spec.model_field, p.model.read(cx).value().to_string()));
        }
        let result =
            settings_form::build_settings(self.base.clone(), self.backend, self.effort, &fields)
                .map_err(|e| e.to_string())
                .and_then(|s| s.save().map(|()| s).map_err(|e| format!("{e:#}")));
        self.status = Some(match result {
            Ok(saved) => {
                self.base = saved;
                self.auto_status = auto_status();
                Ok("Saved".into())
            }
            Err(e) => Err(format!("Could not save: {e}")),
        });
        cx.notify();
    }

    fn render_provider_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = BackendChoice::ALL.map(|choice| {
            Button::new(SharedString::from(format!("backend-{}", choice.label())))
                .label(choice.title())
                .small()
                .toggled_style(self.backend == choice)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.backend = choice;
                    this.status = None;
                    cx.notify();
                }))
        });
        let theme = cx.theme();
        let auto_line = match &self.auto_status {
            Ok(title) => div().child(format!("New lessons will use: {title}")),
            Err(e) => div().text_color(theme.danger).child(e.clone()),
        };
        v_flex()
            .gap_2()
            .child(ui::heading("Provider"))
            .child(h_flex().gap_1().flex_wrap().children(buttons))
            .child(auto_line.text_sm())
            .when_some(
                env("HIL_BACKEND").filter(|v| !v.trim().is_empty()),
                |el, v| {
                    el.child(ui::muted(
                        format!("HIL_BACKEND={v} in your environment overrides this choice."),
                        cx,
                    ))
                },
            )
    }

    fn render_provider(&self, ix: usize, p: &ProviderInputs, cx: &App) -> impl IntoElement {
        let env_var = env_source(p.spec.key_env, env);
        ui::card(cx)
            .child(ui::heading(p.spec.title))
            .when_some(p.key.as_ref(), |el, key| {
                el.child(ui::caption("API key", cx))
                    .child(Input::new(key).mask_toggle().cleanable(true))
                    .when_some(env_var, |el, var| {
                        el.child(ui::muted(
                            format!(
                                "Using {var} from your shell environment (it wins over this field)."
                            ),
                            cx,
                        ))
                    })
            })
            .child(ui::caption("Model (optional)", cx))
            .child(Input::new(&p.model).id(("model", ix)))
    }

    fn render_effort(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = EFFORTS.map(|effort| {
            Button::new(SharedString::from(format!(
                "effort-{}",
                effort.unwrap_or("default")
            )))
            .label(effort.unwrap_or("default"))
            .small()
            .toggled_style(self.effort == effort)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.effort = effort;
                this.status = None;
                cx.notify();
            }))
        });
        v_flex()
            .gap_2()
            .child(ui::heading("Effort"))
            .child(h_flex().gap_1().flex_wrap().children(buttons))
            .child(ui::muted(
                "Reasoning effort for API providers that support it.",
                cx,
            ))
            .when_some(
                env("HIL_EFFORT").filter(|v| !v.trim().is_empty()),
                |el, v| {
                    el.child(ui::muted(
                        format!("HIL_EFFORT={v} in your environment overrides this choice."),
                        cx,
                    ))
                },
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path = Settings::path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let theme = cx.theme();
        let status = self.status.clone().map(|s| match s {
            Ok(msg) => div().text_color(theme.success).child(msg),
            Err(msg) => div().text_color(theme.danger).child(msg),
        });
        let providers: Vec<AnyElement> = self
            .providers
            .iter()
            .enumerate()
            .map(|(i, p)| self.render_provider(i, p, cx).into_any_element())
            .collect();
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .gap_3()
                    .px_6()
                    .pt_5()
                    .pb_3()
                    .child(
                        Button::new("settings-back")
                            .icon(IconName::ArrowLeft)
                            .label("Back")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Close))),
                    )
                    .child(div().text_2xl().font_bold().child("Settings")),
            )
            .child(
                div().id("settings-scroll").flex_1().min_h_0().overflow_y_scrollbar().child(
                    v_flex()
                        .w_full()
                        .max_w(px(760.))
                        .px_6()
                        .pb_6()
                        .gap_5()
                        .child(self.render_provider_picker(cx))
                        .child(v_flex().gap_3().children(providers))
                        .child(self.render_effort(cx))
                        .child(
                            h_flex()
                                .gap_3()
                                .child(
                                    Button::new("settings-save")
                                        .primary()
                                        .label("Save")
                                        .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                                )
                                .children(status),
                        )
                        .child(ui::muted(
                            format!(
                                "Stored in {path} (readable only by you). Environment variables override saved values. New lessons and restyles use these settings."
                            ),
                            cx,
                        )),
                ),
            )
    }
}
