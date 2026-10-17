//! YouTube: metadata and captions through `yt-dlp`, Whisper as a fallback.

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{FetchOptions, tools, vtt};
use crate::model::{Source, SourceKind, TranscriptOrigin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionChoice {
    pub lang: String,
    pub auto: bool,
}

pub fn fetch(url: &str, opts: &FetchOptions, progress: &dyn Fn(&str)) -> Result<Source> {
    progress("Reading video details…");
    let output = tools::run(
        "yt-dlp",
        &[
            "-J",
            "--skip-download",
            "--no-playlist",
            "--no-warnings",
            url,
        ],
        None,
    )?;
    let meta: Value =
        serde_json::from_slice(&output.stdout).context("yt-dlp returned invalid JSON")?;

    let dir = tempfile::tempdir()?;
    let (origin, transcript) = match pick_caption(&meta) {
        Some(choice) => {
            progress(&format!(
                "Downloading {} captions ({})…",
                if choice.auto { "auto" } else { "manual" },
                choice.lang
            ));
            let vtt_text = download_captions(url, &choice, dir.path())?;
            let origin = if choice.auto {
                TranscriptOrigin::AutoCaptions
            } else {
                TranscriptOrigin::Captions
            };
            (origin, vtt::vtt_to_transcript(&vtt_text))
        }
        None if opts.transcribe => {
            progress("No captions found. Downloading audio for Whisper…");
            let audio = tools::download_audio(url, dir.path())?;
            progress("Transcribing locally with Whisper (this can take a while)…");
            (
                TranscriptOrigin::Whisper,
                tools::whisper_transcribe(&audio, dir.path())?,
            )
        }
        None => bail!("this video has no captions; rerun without --no-transcribe to use Whisper"),
    };
    if transcript.trim().is_empty() {
        bail!("the transcript came back empty");
    }

    Ok(Source {
        url: url.to_string(),
        kind: SourceKind::YouTube,
        title: str_field(&meta, "title")
            .unwrap_or("Untitled video")
            .to_string(),
        author: str_field(&meta, "channel")
            .or_else(|| str_field(&meta, "uploader"))
            .unwrap_or("unknown")
            .to_string(),
        duration_secs: meta
            .get("duration")
            .and_then(Value::as_f64)
            .map(|d| d as u64),
        origin,
        text: compose_text(&meta, &transcript),
        truncated: false,
    })
}

fn download_captions(url: &str, choice: &CaptionChoice, dir: &std::path::Path) -> Result<String> {
    let template = dir.join("captions.%(ext)s");
    let write_flag = if choice.auto {
        "--write-auto-subs"
    } else {
        "--write-subs"
    };
    tools::run(
        "yt-dlp",
        &[
            "--skip-download",
            "--no-playlist",
            "--no-warnings",
            "--quiet",
            write_flag,
            "--sub-langs",
            &choice.lang,
            "--sub-format",
            "vtt",
            "-o",
            &template.to_string_lossy(),
            url,
        ],
        None,
    )?;
    let file = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|e| e == "vtt"))
        .context("yt-dlp did not write a caption file")?;
    Ok(std::fs::read_to_string(file)?)
}

/// Chooses the caption track that best represents what is said in the video.
///
/// Order: manual English, English auto-captions, manual captions in the
/// video's own language, auto-captions in that language, then any manual track.
pub fn pick_caption(meta: &Value) -> Option<CaptionChoice> {
    let manual = track_names(meta, "subtitles");
    let auto = track_names(meta, "automatic_captions");
    let video_lang = str_field(meta, "language").unwrap_or("en");

    let pick = |tracks: &[String], wanted: &[&str], is_auto: bool| {
        wanted.iter().find_map(|w| {
            tracks
                .iter()
                .find(|t| t.as_str() == *w)
                .map(|t| CaptionChoice {
                    lang: t.clone(),
                    auto: is_auto,
                })
        })
    };
    let lang_orig = format!("{video_lang}-orig");

    pick(&manual, &["en", "en-US", "en-GB", "en-CA", "en-AU"], false)
        .or_else(|| pick(&auto, &["en-orig", "en"], true))
        .or_else(|| pick(&manual, &[video_lang], false))
        .or_else(|| pick(&auto, &[lang_orig.as_str(), video_lang], true))
        .or_else(|| {
            manual
                .iter()
                .find(|t| !t.contains("live_chat"))
                .map(|t| CaptionChoice {
                    lang: t.clone(),
                    auto: false,
                })
        })
}

fn track_names(meta: &Value, key: &str) -> Vec<String> {
    let mut names: Vec<String> = meta
        .get(key)
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter(|(_, formats)| formats.as_array().is_some_and(|f| !f.is_empty()))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn str_field<'a>(meta: &'a Value, key: &str) -> Option<&'a str> {
    meta.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// Description and chapters give the model structure the transcript lacks.
fn compose_text(meta: &Value, transcript: &str) -> String {
    let mut text = String::new();
    if let Some(desc) = str_field(meta, "description") {
        text.push_str("## Video description\n");
        text.push_str(desc.trim());
        text.push_str("\n\n");
    }
    if let Some(chapters) = meta.get("chapters").and_then(Value::as_array)
        && !chapters.is_empty()
    {
        text.push_str("## Chapters\n");
        for ch in chapters {
            let start = ch.get("start_time").and_then(Value::as_f64).unwrap_or(0.0);
            let title = ch.get("title").and_then(Value::as_str).unwrap_or("");
            text.push_str(&format!("[{}] {title}\n", vtt::format_timestamp(start)));
        }
        text.push('\n');
    }
    text.push_str("## Transcript\n");
    text.push_str(transcript);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(manual: &[&str], auto: &[&str], lang: &str) -> Value {
        let tracks = |names: &[&str]| {
            names
                .iter()
                .map(|n| (n.to_string(), json!([{"ext": "vtt"}])))
                .collect::<serde_json::Map<_, _>>()
        };
        json!({"subtitles": tracks(manual), "automatic_captions": tracks(auto), "language": lang})
    }

    fn choice(lang: &str, auto: bool) -> Option<CaptionChoice> {
        Some(CaptionChoice {
            lang: lang.into(),
            auto,
        })
    }

    #[test]
    fn prefers_manual_english() {
        let m = meta(&["de", "en", "en-de"], &["en-orig", "en"], "en");
        assert_eq!(pick_caption(&m), choice("en", false));
    }

    #[test]
    fn falls_back_to_english_auto_captions() {
        let m = meta(&[], &["de", "en", "en-orig"], "en");
        assert_eq!(pick_caption(&m), choice("en-orig", true));
    }

    #[test]
    fn uses_the_video_language_when_no_english() {
        let m = meta(&[], &["hi-orig", "hi", "ja"], "hi");
        assert_eq!(pick_caption(&m), choice("hi-orig", true));
        let m = meta(&["ur"], &["ur-orig"], "ur");
        assert_eq!(pick_caption(&m), choice("ur", false));
    }

    #[test]
    fn none_when_no_tracks() {
        assert_eq!(pick_caption(&meta(&[], &[], "en")), None);
        assert_eq!(pick_caption(&meta(&["live_chat"], &[], "en")), None);
    }

    #[test]
    fn compose_text_includes_description_and_chapters() {
        let m = json!({
            "description": "About ownership.",
            "chapters": [{"start_time": 0.0, "title": "Intro"}, {"start_time": 75.0, "title": "Borrowing"}]
        });
        let text = compose_text(&m, "[00:00] hi");
        assert_eq!(
            text,
            "## Video description\nAbout ownership.\n\n## Chapters\n[00:00] Intro\n[01:15] Borrowing\n\n## Transcript\n[00:00] hi"
        );
    }
}
