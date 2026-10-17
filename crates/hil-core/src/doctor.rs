//! Checks the external tools Honey Interactive Learn relies on.

use crate::llm::api_key;
use crate::source::tools::find_program;

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    /// Needed for: …
    pub purpose: &'static str,
    pub hint: &'static str,
}

pub fn run_checks() -> Vec<Check> {
    let has = |p: &str| find_program(p).is_some();
    let llm_ready = api_key().is_some() || has("claude");
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
            name: "claude",
            ok: has("claude"),
            purpose: "lesson writing via your Claude Code login",
            hint: "npm install -g @anthropic-ai/claude-code, then run `claude` once to log in",
        },
        Check {
            name: "ANTHROPIC_API_KEY",
            ok: api_key().is_some(),
            purpose: "lesson writing via the Anthropic API (alternative to claude)",
            hint: "export ANTHROPIC_API_KEY=sk-ant-…",
        },
        Check {
            name: "language model",
            ok: llm_ready,
            purpose: "at least one of `claude` or ANTHROPIC_API_KEY",
            hint: "install Claude Code or set ANTHROPIC_API_KEY",
        },
    ]
}
