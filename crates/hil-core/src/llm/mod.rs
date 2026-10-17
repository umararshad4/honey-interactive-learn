//! Language-model backends that write lessons.

mod anthropic;
mod claude_cli;
pub mod prompt;

use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub use anthropic::{AnthropicApi, parse_sse_stream};
pub use claude_cli::ClaudeCli;

use crate::model::LessonContent;
use crate::source::tools::find_program;

/// One structured-output request.
pub struct GenerateRequest<'a> {
    pub system: &'a str,
    pub user: &'a str,
    pub schema: &'a Value,
}

pub trait LlmBackend: Send + Sync {
    /// Short label stored with each lesson, e.g. `claude-cli` or `api:claude-opus-5-5`.
    fn label(&self) -> String;
    /// Returns a JSON value that matches `req.schema`.
    fn generate(&self, req: &GenerateRequest) -> Result<Value>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendChoice {
    /// API when `ANTHROPIC_API_KEY` is set, otherwise the `claude` CLI.
    #[default]
    Auto,
    ClaudeCli,
    Api,
}

impl FromStr for BackendChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "claude-cli" | "cli" | "claude" => Ok(Self::ClaudeCli),
            "api" | "anthropic" => Ok(Self::Api),
            other => Err(format!(
                "unknown backend `{other}`, expected auto, claude-cli or api"
            )),
        }
    }
}

impl fmt::Display for BackendChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::ClaudeCli => "claude-cli",
            Self::Api => "api",
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct BackendConfig {
    pub choice: BackendChoice,
    /// Model override. API default: `claude-opus-5-5`; CLI default: your Claude Code default.
    pub model: Option<String>,
    /// API effort: low, medium, high, xhigh or max.
    pub effort: Option<String>,
}

impl BackendConfig {
    /// Reads `HIL_BACKEND`, `HIL_MODEL` and `HIL_EFFORT`.
    pub fn from_env() -> Self {
        let get = |k| {
            std::env::var(k)
                .ok()
                .filter(|v: &String| !v.trim().is_empty())
        };
        Self {
            choice: get("HIL_BACKEND")
                .and_then(|v| v.parse().ok())
                .unwrap_or_default(),
            model: get("HIL_MODEL"),
            effort: get("HIL_EFFORT"),
        }
    }
}

pub fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
}

/// Builds the backend described by `config`.
pub fn build_backend(config: &BackendConfig) -> Result<Box<dyn LlmBackend>> {
    let use_api = match config.choice {
        BackendChoice::Api => true,
        BackendChoice::ClaudeCli => false,
        BackendChoice::Auto => {
            if api_key().is_some() {
                true
            } else if find_program("claude").is_some() {
                false
            } else {
                bail!(
                    "no language model available.\n  • set ANTHROPIC_API_KEY to use the Anthropic API, or\n  • install Claude Code (https://claude.com/claude-code) and log in, so `claude` is on PATH"
                );
            }
        }
    };
    if use_api {
        let key = api_key().context("ANTHROPIC_API_KEY is not set")?;
        Ok(Box::new(AnthropicApi::new(
            key,
            config.model.clone(),
            config.effort.clone(),
        )))
    } else {
        Ok(Box::new(ClaudeCli::new(config.model.clone())))
    }
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
        assert_eq!("api".parse::<BackendChoice>().unwrap(), BackendChoice::Api);
        assert!("gpt".parse::<BackendChoice>().is_err());
    }
}
