//! Language-model backends that write lessons.

mod anthropic;
mod claude_cli;
mod gemini;
mod openai;
pub mod prompt;

use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub use anthropic::{AnthropicApi, parse_sse_stream};
pub use claude_cli::ClaudeCli;
pub use gemini::GeminiApi;
pub use openai::OpenAiApi;

use crate::model::LessonContent;
use crate::settings::Settings;
use crate::source::tools::find_program;

/// One structured-output request.
pub struct GenerateRequest<'a> {
    pub system: &'a str,
    pub user: &'a str,
    pub schema: &'a Value,
}

pub trait LlmBackend: Send + Sync {
    /// Short label stored with each lesson, e.g. `claude-cli` or `openai:gpt-6-astra`.
    fn label(&self) -> String;
    /// Returns a JSON value that matches `req.schema`.
    fn generate(&self, req: &GenerateRequest) -> Result<Value>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendChoice {
    /// First available: Anthropic key, OpenAI key, Gemini key, then the `claude` CLI.
    #[default]
    Auto,
    ClaudeCli,
    Anthropic,
    OpenAi,
    Gemini,
}

impl BackendChoice {
    pub const ALL: [BackendChoice; 5] = [
        Self::Auto,
        Self::ClaudeCli,
        Self::Anthropic,
        Self::OpenAi,
        Self::Gemini,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ClaudeCli => "claude-cli",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }

    /// Human name for UIs.
    pub fn title(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::ClaudeCli => "Claude Code CLI",
            Self::Anthropic => "Anthropic API",
            Self::OpenAi => "OpenAI API",
            Self::Gemini => "Google Gemini API",
        }
    }
}

impl FromStr for BackendChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "claude-cli" | "cli" | "claude" => Ok(Self::ClaudeCli),
            "anthropic" | "api" => Ok(Self::Anthropic),
            "openai" | "gpt" => Ok(Self::OpenAi),
            "gemini" | "google" => Ok(Self::Gemini),
            other => Err(format!(
                "unknown backend `{other}`, expected auto, claude-cli, anthropic, openai or gemini"
            )),
        }
    }
}

impl fmt::Display for BackendChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl Serialize for BackendChoice {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for BackendChoice {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Everything needed to pick and build a backend: settings file + environment + overrides.
#[derive(Debug, Clone, Default)]
pub struct BackendConfig {
    pub choice: BackendChoice,
    /// Model for this run; otherwise the provider's model from settings, then its default.
    pub model: Option<String>,
    /// Reasoning effort for API providers: low, medium, high, xhigh or max.
    pub effort: Option<String>,
    /// Keys and per-provider models, with environment variables applied.
    pub settings: Settings,
}

impl BackendConfig {
    /// Loads `settings.json`, then applies environment variables on top.
    pub fn load() -> Self {
        Self::from_settings(Settings::load().unwrap_or_default())
    }

    pub fn from_settings(mut settings: Settings) -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        if let Some(k) = get("ANTHROPIC_API_KEY") {
            settings.anthropic_api_key = Some(k);
        }
        if let Some(k) = get("OPENAI_API_KEY") {
            settings.openai_api_key = Some(k);
        }
        if let Some(k) = get("GEMINI_API_KEY").or_else(|| get("GOOGLE_API_KEY")) {
            settings.gemini_api_key = Some(k);
        }
        let choice = get("HIL_BACKEND")
            .and_then(|v| v.parse().ok())
            .unwrap_or(settings.backend);
        Self {
            choice,
            model: get("HIL_MODEL"),
            effort: get("HIL_EFFORT").or_else(|| settings.effort.clone()),
            settings,
        }
    }

    /// The provider `Auto` resolves to, or an error explaining how to set one up.
    pub fn resolve(&self) -> Result<BackendChoice> {
        let s = &self.settings;
        let resolved = match self.choice {
            BackendChoice::Auto => {
                if s.anthropic_api_key.is_some() {
                    BackendChoice::Anthropic
                } else if s.openai_api_key.is_some() {
                    BackendChoice::OpenAi
                } else if s.gemini_api_key.is_some() {
                    BackendChoice::Gemini
                } else if find_program("claude").is_some() {
                    BackendChoice::ClaudeCli
                } else {
                    bail!(
                        "no language model available. Set up one of:\n  • an API key: `hil config set anthropic-key|openai-key|gemini-key <key>` (or the desktop Settings)\n  • ANTHROPIC_API_KEY, OPENAI_API_KEY or GEMINI_API_KEY in your environment\n  • Claude Code (https://claude.com/claude-code), logged in, so `claude` is on PATH"
                    );
                }
            }
            other => other,
        };
        Ok(resolved)
    }
}

/// Builds the backend described by `config`.
pub fn build_backend(config: &BackendConfig) -> Result<Box<dyn LlmBackend>> {
    let s = &config.settings;
    let model = |saved: &Option<String>| config.model.clone().or_else(|| saved.clone());
    let missing = |provider: &str, field: &str, env: &str| {
        format!(
            "no {provider} API key. Run `hil config set {field} <key>`, add it in the desktop Settings, or export {env}"
        )
    };
    Ok(match config.resolve()? {
        BackendChoice::Anthropic => Box::new(AnthropicApi::new(
            s.anthropic_api_key
                .clone()
                .with_context(|| missing("Anthropic", "anthropic-key", "ANTHROPIC_API_KEY"))?,
            model(&s.anthropic_model),
            config.effort.clone(),
        )),
        BackendChoice::OpenAi => Box::new(OpenAiApi::new(
            s.openai_api_key
                .clone()
                .with_context(|| missing("OpenAI", "openai-key", "OPENAI_API_KEY"))?,
            model(&s.openai_model),
            config.effort.clone(),
        )),
        BackendChoice::Gemini => Box::new(GeminiApi::new(
            s.gemini_api_key
                .clone()
                .with_context(|| missing("Gemini", "gemini-key", "GEMINI_API_KEY"))?,
            model(&s.gemini_model),
        )),
        BackendChoice::ClaudeCli | BackendChoice::Auto => {
            Box::new(ClaudeCli::new(model(&s.claude_cli_model)))
        }
    })
}

/// Cleans model output so the UIs can trust it.
pub fn normalize(mut c: LessonContent) -> LessonContent {
    c.title = c.title.trim().to_string();
    c.key_points.retain(|p| !p.trim().is_empty());
    c.sections.retain(|s| !s.body.trim().is_empty());
    c.code_snippets.retain(|s| !s.code.trim().is_empty());
    c.flashcards
        .retain(|f| !f.front.trim().is_empty() && !f.back.trim().is_empty());
    c.quiz.retain(|q| {
        q.options.len() >= 2 && q.answer_index < q.options.len() && !q.question.trim().is_empty()
    });
    c.questions.retain(|q| !q.question.trim().is_empty());
    c.diagram.ascii = c.diagram.ascii.trim_end().to_string();
    c.diagram.mermaid = strip_fence(&c.diagram.mermaid);
    c
}

fn strip_fence(s: &str) -> String {
    let t = s.trim();
    let t = t
        .strip_prefix("```mermaid")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t).trim().to_string()
}

/// Parses JSON that may be wrapped in a Markdown code fence.
pub fn parse_json_loose(text: &str) -> Result<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str(t) {
        return Ok(v);
    }
    let start = t
        .find('{')
        .context("model reply contained no JSON object")?;
    let end = t
        .rfind('}')
        .context("model reply contained no JSON object")?;
    serde_json::from_str(&t[start..=end]).context("model reply was not valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Mcq;

    fn mcq(options: usize, answer: usize) -> Mcq {
        Mcq {
            question: "Q?".into(),
            options: (0..options).map(|i| format!("o{i}")).collect(),
            answer_index: answer,
            explanation: String::new(),
        }
    }

    #[test]
    fn normalize_drops_broken_quiz_items_and_fences() {
        let mut c = LessonContent {
            quiz: vec![mcq(4, 1), mcq(4, 4), mcq(1, 0)],
            ..Default::default()
        };
        c.diagram.mermaid = "```mermaid\nflowchart LR\nA-->B\n```".into();
        let c = normalize(c);
        assert_eq!(c.quiz, vec![mcq(4, 1)]);
        assert_eq!(c.diagram.mermaid, "flowchart LR\nA-->B");
    }

    #[test]
    fn parse_json_loose_handles_fences_and_chatter() {
        assert_eq!(parse_json_loose("{\"a\":1}").unwrap()["a"], 1);
        assert_eq!(
            parse_json_loose("Here you go:\n```json\n{\"a\": {\"b\": 2}}\n```").unwrap()["a"]["b"],
            2
        );
        assert!(parse_json_loose("no json").is_err());
    }

    #[test]
    fn backend_choice_parses() {
        assert_eq!(
            "CLI".parse::<BackendChoice>().unwrap(),
            BackendChoice::ClaudeCli
        );
        assert_eq!(
            "api".parse::<BackendChoice>().unwrap(),
            BackendChoice::Anthropic
        );
        assert_eq!(
            "OpenAI".parse::<BackendChoice>().unwrap(),
            BackendChoice::OpenAi
        );
        assert_eq!(
            "gemini".parse::<BackendChoice>().unwrap(),
            BackendChoice::Gemini
        );
        assert!("llama".parse::<BackendChoice>().is_err());
        for c in BackendChoice::ALL {
            assert_eq!(c.label().parse::<BackendChoice>().unwrap(), c);
        }
    }

    fn config(choice: BackendChoice, settings: Settings) -> BackendConfig {
        BackendConfig {
            choice,
            model: None,
            effort: None,
            settings,
        }
    }

    #[test]
    fn auto_prefers_keys_in_order() {
        let mut s = Settings {
            gemini_api_key: Some("g".into()),
            ..Default::default()
        };
        assert_eq!(
            config(BackendChoice::Auto, s.clone()).resolve().unwrap(),
            BackendChoice::Gemini
        );
        s.openai_api_key = Some("o".into());
        assert_eq!(
            config(BackendChoice::Auto, s.clone()).resolve().unwrap(),
            BackendChoice::OpenAi
        );
        s.anthropic_api_key = Some("a".into());
        assert_eq!(
            config(BackendChoice::Auto, s.clone()).resolve().unwrap(),
            BackendChoice::Anthropic
        );
        assert_eq!(
            config(BackendChoice::Gemini, s).resolve().unwrap(),
            BackendChoice::Gemini
        );
    }

    #[test]
    fn builds_each_provider_with_saved_or_overridden_model() {
        let s = Settings {
            openai_api_key: Some("o".into()),
            openai_model: Some("gpt-6-luna".into()),
            gemini_api_key: Some("g".into()),
            ..Default::default()
        };
        let openai = build_backend(&config(BackendChoice::OpenAi, s.clone())).unwrap();
        assert_eq!(openai.label(), "openai:gpt-6-luna");
        let mut c = config(BackendChoice::Gemini, s.clone());
        assert_eq!(
            build_backend(&c).unwrap().label(),
            "gemini:gemini-3.8-flash"
        );
        c.model = Some("gemini-4-argon".into());
        assert_eq!(build_backend(&c).unwrap().label(), "gemini:gemini-4-argon");
        let err = build_backend(&config(BackendChoice::Anthropic, s))
            .err()
            .unwrap();
        assert!(err.to_string().contains("anthropic-key"), "{err}");
    }
}
