//! Checks the external tools and model providers Honey Interactive Learn relies on.

use crate::llm::BackendConfig;
use crate::source::tools::find_program;

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    /// What it is needed for.
    pub purpose: &'static str,
    pub hint: &'static str,
}

pub fn run_checks(config: &BackendConfig) -> Vec<Check> {
    let has = |p: &str| find_program(p).is_some();
    let s = &config.settings;
    vec![
        Check {
            name: "yt-dlp",
            ok: has("yt-dlp"),
            purpose: "YouTube captions, audio for videos",
            hint: "brew install yt-dlp   (or: pipx install yt-dlp)",
        },
        Check {
            name: "ffmpeg",
            ok: has("ffmpeg"),
            purpose: "audio decoding for Whisper",
            hint: "brew install ffmpeg",
        },
        Check {
            name: "whisper",
            ok: has("whisper"),
            purpose: "transcribing videos without captions (optional)",
            hint: "pipx install openai-whisper",
        },
        Check {
            name: "claude CLI",
            ok: has("claude"),
            purpose: "lessons via your Claude Code login",
            hint: "install Claude Code, then run `claude` once to log in",
        },
        Check {
            name: "Anthropic key",
            ok: s.anthropic_api_key.is_some(),
            purpose: "lessons via the Anthropic API",
            hint: "hil config set anthropic-key <key>   (or export ANTHROPIC_API_KEY)",
        },
        Check {
            name: "OpenAI key",
            ok: s.openai_api_key.is_some(),
            purpose: "lessons via the OpenAI API",
            hint: "hil config set openai-key <key>   (or export OPENAI_API_KEY)",
        },
        Check {
            name: "Gemini key",
            ok: s.gemini_api_key.is_some(),
            purpose: "lessons via the Google Gemini API",
            hint: "hil config set gemini-key <key>   (or export GEMINI_API_KEY)",
        },
    ]
}
