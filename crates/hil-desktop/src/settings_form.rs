//! UI-independent pieces of the Settings screen (kept separate so they are testable).

use hil_core::llm::BackendChoice;
use hil_core::settings::Settings;

/// Effort choices; `None` means the provider default.
pub const EFFORTS: [Option<&str>; 6] = [
    None,
    Some("low"),
    Some("medium"),
    Some("high"),
    Some("xhigh"),
    Some("max"),
];

/// One provider block on the Settings screen.
pub struct ProviderSpec {
    pub title: &'static str,
    /// `Settings::set` name of the key field; `None` for the claude CLI.
    pub key_field: Option<&'static str>,
    /// Environment variables that override the saved key, in priority order.
    pub key_env: &'static [&'static str],
    pub model_field: &'static str,
    pub default_model: &'static str,
}

pub const PROVIDERS: [ProviderSpec; 4] = [
    ProviderSpec {
        title: "Anthropic API",
        key_field: Some("anthropic-key"),
        key_env: &["ANTHROPIC_API_KEY"],
        model_field: "anthropic-model",
        default_model: "claude-opus-5-5",
    },
    ProviderSpec {
        title: "OpenAI API",
        key_field: Some("openai-key"),
        key_env: &["OPENAI_API_KEY"],
        model_field: "openai-model",
        default_model: "gpt-6-astra",
    },
    ProviderSpec {
        title: "Google Gemini API",
        key_field: Some("gemini-key"),
        key_env: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        model_field: "gemini-model",
        default_model: "gemini-3.8-flash",
    },
    ProviderSpec {
        title: "Claude Code CLI",
        key_field: None,
        key_env: &[],
        model_field: "claude-cli-model",
        default_model: "your Claude Code default",
    },
];

/// Saved value of a field, for pre-filling inputs.
pub fn saved_value(s: &Settings, field: &str) -> String {
    let v = match field {
        "anthropic-key" => &s.anthropic_api_key,
        "openai-key" => &s.openai_api_key,
        "gemini-key" => &s.gemini_api_key,
        "anthropic-model" => &s.anthropic_model,
        "openai-model" => &s.openai_model,
        "gemini-model" => &s.gemini_model,
        "claude-cli-model" => &s.claude_cli_model,
        "effort" => &s.effort,
        _ => &None,
    };
    v.clone().unwrap_or_default()
}

/// Applies the form to `base`. Empty values clear their field.
pub fn build_settings(
    mut base: Settings,
    backend: BackendChoice,
    effort: Option<&str>,
    fields: &[(&str, String)],
) -> Result<Settings, String> {
    base.backend = backend;
    base.set("effort", effort.unwrap_or(""))?;
    for (name, value) in fields {
        base.set(name, value)?;
    }
    Ok(base)
}

/// First environment variable in `vars` that is set and non-empty.
pub fn env_source(
    vars: &[&'static str],
    get: impl Fn(&str) -> Option<String>,
) -> Option<&'static str> {
    vars.iter()
        .copied()
        .find(|v| get(v).is_some_and(|x| !x.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_values_are_trimmed_and_empty_clears() {
        let base = Settings {
            openai_api_key: Some("test-key-old".into()),
            gemini_model: Some("old-model".into()),
            ..Settings::default()
        };
        let fields = vec![
            ("openai-key", "  test-key-new0000  ".to_string()),
            ("gemini-model", String::new()),
            ("anthropic-model", "claude-opus-5-5".to_string()),
        ];
        let s = build_settings(base, BackendChoice::OpenAi, Some("high"), &fields).unwrap();
        assert_eq!(s.backend, BackendChoice::OpenAi);
        assert_eq!(s.openai_api_key.as_deref(), Some("test-key-new0000"));
        assert_eq!(s.gemini_model, None);
        assert_eq!(s.anthropic_model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(s.effort.as_deref(), Some("high"));

        let s = build_settings(s, BackendChoice::Auto, None, &[]).unwrap();
        assert_eq!(s.effort, None);
        assert_eq!(saved_value(&s, "openai-key"), "test-key-new0000");
        assert_eq!(saved_value(&s, "gemini-key"), "");
    }

    #[test]
    fn provider_fields_are_known_to_core() {
        let mut s = Settings::default();
        for p in &PROVIDERS {
            for field in p.key_field.into_iter().chain([p.model_field]) {
                s.set(field, "x").unwrap();
                assert_eq!(saved_value(&s, field), "x", "{field}");
            }
        }
    }

    #[test]
    fn env_source_picks_first_non_empty() {
        let get = |k: &str| match k {
            "GEMINI_API_KEY" => Some("  ".to_string()),
            "GOOGLE_API_KEY" => Some("test-key-env".to_string()),
            _ => None,
        };
        assert_eq!(
            env_source(&["GEMINI_API_KEY", "GOOGLE_API_KEY"], get),
            Some("GOOGLE_API_KEY")
        );
        assert_eq!(env_source(&["OPENAI_API_KEY"], get), None);
    }
}
