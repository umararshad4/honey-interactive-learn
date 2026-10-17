//! Turning a link into source text.

mod link;
pub mod tools;
mod vtt;
pub mod x;
pub mod youtube;

use anyhow::Result;

pub use link::Link;
pub use vtt::{format_timestamp, vtt_to_transcript};

use crate::model::Source;

/// Upper bound on characters sent to the model (~150k tokens).
pub const MAX_SOURCE_CHARS: usize = 600_000;

#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Allow downloading audio and running Whisper when there is no text.
    pub transcribe: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self { transcribe: true }
    }
}

/// Fetches the text behind a YouTube or X link.
pub fn fetch(input: &str, opts: &FetchOptions, progress: &dyn Fn(&str)) -> Result<Source> {
    let link = Link::parse(input)?;
    let canonical = link.canonical_url();
    let mut source = match &link {
        Link::YouTube { .. } => youtube::fetch(&canonical, opts, progress)?,
        Link::X { status_id } => x::fetch(status_id, &canonical, opts, progress)?,
    };
    if source.text.chars().count() > MAX_SOURCE_CHARS {
        source.text = source.text.chars().take(MAX_SOURCE_CHARS).collect();
        source.truncated = true;
        progress("Source is very long; only the first part will be used.");
    }
    Ok(source)
}
