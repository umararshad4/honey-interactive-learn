//! X / Twitter posts through the public FxTwitter API (no login needed).
//!
//! The thread endpoint returns the author's own thread around the post. Video
//! posts are transcribed with `yt-dlp` + Whisper when transcription is on.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{FetchOptions, tools};
use crate::model::{Source, SourceKind, TranscriptOrigin};

const API: &str = "https://api.fxtwitter.com";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XPost {
    pub author_name: String,
    pub screen_name: String,
    pub url: String,
    /// One entry per post in the thread, in reading order.
    pub parts: Vec<String>,
    pub has_video: bool,
}

pub fn fetch(
    status_id: &str,
    canonical_url: &str,
    opts: &FetchOptions,
    progress: &dyn Fn(&str),
) -> Result<Source> {
    progress("Fetching post and thread…");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(concat!(
            "honey-interactive-learn/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .into();
    let mut resp = agent
        .get(format!("{API}/2/thread/{status_id}"))
        .call()
        .context("could not reach api.fxtwitter.com")?;
    let status = resp.status();
    let body: Value = resp
        .body_mut()
        .read_json()
        .context("FxTwitter returned invalid JSON")?;
    if !status.is_success() {
        let msg = body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("request failed");
        bail!("X post {status_id} is unavailable ({status}): {msg}");
    }

    let mut post = parse_thread(&body).context("X post has no readable text")?;
    if post.url.is_empty() {
        post.url = canonical_url.to_string();
    }
    let mut origin = TranscriptOrigin::PostText;
    let mut text = post.parts.join("\n\n---\n\n");

    if post.has_video && opts.transcribe {
        progress("Post has video. Downloading audio for Whisper…");
        let dir = tempfile::tempdir()?;
        match tools::download_audio(&post.url, dir.path()).and_then(|audio| {
            progress("Transcribing video locally with Whisper…");
            tools::whisper_transcribe(&audio, dir.path())
        }) {
            Ok(transcript) if !transcript.is_empty() => {
                text.push_str("\n\n## Video transcript\n");
                text.push_str(&transcript);
                origin = TranscriptOrigin::Whisper;
            }
            Ok(_) => progress("Video had no speech; using post text only."),
            Err(e) => progress(&format!("Skipping video transcript: {e}")),
        }
    }

    let first_line = post.parts[0]
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default();
    Ok(Source {
        url: post.url.clone(),
        kind: SourceKind::X,
        title: truncate_chars(first_line, 90),
        author: format!("{} (@{})", post.author_name, post.screen_name),
        duration_secs: None,
        origin,
        text,
        truncated: false,
    })
}

/// Turns a `/2/thread/<id>` response into readable parts.
pub fn parse_thread(body: &Value) -> Option<XPost> {
    let statuses: Vec<&Value> = match body.get("thread").and_then(Value::as_array) {
        Some(list) if !list.is_empty() => list.iter().collect(),
        _ => vec![body.get("status")?],
    };
    let head = body.get("status").unwrap_or(statuses[0]);
    let author = head.get("author").or_else(|| body.get("author"));

    let parts: Vec<String> = statuses.iter().filter_map(|s| status_text(s)).collect();
    if parts.is_empty() {
        return None;
    }
    Some(XPost {
        author_name: str_at(author, "name").unwrap_or("Unknown").to_string(),
        screen_name: str_at(author, "screen_name")
            .unwrap_or("unknown")
            .to_string(),
        url: str_at(Some(head), "url").unwrap_or_default().to_string(),
        parts,
        has_video: statuses.iter().any(|s| has_video(s)),
    })
}

fn status_text(status: &Value) -> Option<String> {
    let mut out = String::new();
    if let Some(article) = status.get("article") {
        if let Some(title) = str_at(Some(article), "title") {
            out.push_str(&format!("# {title}\n\n"));
        }
        let blocks = article
            .get("content")
            .and_then(|c| c.get("blocks"))
            .and_then(Value::as_array);
        match blocks {
            Some(blocks) => {
                for block in blocks {
                    if let Some(t) = str_at(Some(block), "text") {
                        out.push_str(t);
                        out.push_str("\n\n");
                    }
                }
            }
            None => {
                if let Some(preview) = str_at(Some(article), "preview_text") {
                    out.push_str(preview);
                    out.push_str("\n\n");
                }
            }
        }
    }
    if let Some(text) = str_at(Some(status), "text") {
        out.push_str(text);
    }
    if let Some(quote) = status.get("quote")
        && let Some(qtext) = str_at(Some(quote), "text")
    {
        let who = str_at(quote.get("author"), "screen_name").unwrap_or("unknown");
        out.push_str(&format!(
            "\n\n> Quoting @{who}:\n> {}",
            qtext.replace('\n', "\n> ")
        ));
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then_some(out)
}

fn has_video(status: &Value) -> bool {
    let media = status.get("media");
    let videos = media
        .and_then(|m| m.get("videos"))
        .and_then(Value::as_array)
        .is_some_and(|v| !v.is_empty());
    let all = media
        .and_then(|m| m.get("all"))
        .and_then(Value::as_array)
        .is_some_and(|all| {
            all.iter()
                .any(|m| matches!(str_at(Some(m), "type"), Some("video" | "gif")))
        });
    videos || all
}

fn str_at<'a>(v: Option<&'a Value>, key: &str) -> Option<&'a str> {
    v?.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn truncate_chars(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max - 1).collect::<String>().trim_end()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn status(id: &str, text: &str) -> Value {
        json!({
            "url": format!("https://x.com/dev/status/{id}"),
            "id": id,
            "text": text,
            "author": {"name": "Dev Person", "screen_name": "dev"},
            "media": {}
        })
    }

    #[test]
    fn parses_a_thread_in_order() {
        let body = json!({
            "code": 200,
            "status": status("1", "1/ Rust ownership explained"),
            "thread": [status("1", "1/ Rust ownership explained"), status("2", "2/ Borrowing rules")],
            "author": {"name": "Dev Person", "screen_name": "dev"}
        });
        let post = parse_thread(&body).unwrap();
        assert_eq!(
            post.parts,
            vec!["1/ Rust ownership explained", "2/ Borrowing rules"]
        );
        assert_eq!(post.screen_name, "dev");
        assert_eq!(post.url, "https://x.com/dev/status/1");
        assert!(!post.has_video);
    }

    #[test]
    fn falls_back_to_the_single_status_and_reads_quotes_and_video() {
        let mut s = status("9", "Watch this");
        s["quote"] = json!({"text": "original\nidea", "author": {"screen_name": "other"}});
        s["media"] = json!({"videos": [{"url": "https://video.twimg.com/x.mp4"}]});
        let post = parse_thread(&json!({"status": s, "thread": []})).unwrap();
        assert_eq!(
            post.parts,
            vec!["Watch this\n\n> Quoting @other:\n> original\n> idea"]
        );
        assert!(post.has_video);
    }

    #[test]
    fn reads_long_form_articles() {
        let mut s = status("5", "");
        s["article"] = json!({
            "title": "Why GPUs",
            "content": {"blocks": [{"text": "Para one."}, {"text": "Para two."}]}
        });
        let post = parse_thread(&json!({"status": s})).unwrap();
        assert_eq!(post.parts, vec!["# Why GPUs\n\nPara one.\n\nPara two."]);
    }

    #[test]
    fn empty_post_is_none() {
        assert!(parse_thread(&json!({"status": status("1", "  ")})).is_none());
        assert!(parse_thread(&json!({"code": 404})).is_none());
    }

    #[test]
    fn truncates_titles_on_char_boundaries() {
        assert_eq!(truncate_chars("héllo wörld", 6), "héllo…");
        assert_eq!(truncate_chars("short", 10), "short");
    }
}
