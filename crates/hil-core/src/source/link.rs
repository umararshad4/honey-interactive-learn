//! Recognise supported links.

use anyhow::{Result, bail};
use url::Url;

/// A supported link, normalised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    YouTube { video_id: String },
    X { status_id: String },
}

impl Link {
    pub fn parse(input: &str) -> Result<Link> {
        let trimmed = input.trim();
        let with_scheme = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };
        let Ok(url) = Url::parse(&with_scheme) else {
            bail!("not a valid URL: {trimmed}");
        };
        let host = url
            .host_str()
            .unwrap_or_default()
            .trim_start_matches("www.")
            .trim_start_matches("m.")
            .trim_start_matches("mobile.")
            .to_ascii_lowercase();
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|p| !p.is_empty()).collect())
            .unwrap_or_default();

        let link = match host.as_str() {
            "youtu.be" => segments.first().map(|id| youtube(id)),
            "youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => {
                match segments.as_slice() {
                    ["watch", ..] => url
                        .query_pairs()
                        .find(|(k, _)| k == "v")
                        .map(|(_, v)| youtube(&v)),
                    ["shorts" | "live" | "embed" | "v", id, ..] => Some(youtube(id)),
                    _ => None,
                }
            }
            "x.com" | "twitter.com" | "fxtwitter.com" | "vxtwitter.com" | "fixupx.com"
            | "fixvx.com" => match segments.as_slice() {
                [_, "status" | "statuses", id, ..] => Some(Link::X {
                    status_id: (*id).to_string(),
                }),
                _ => None,
            },
            _ => None,
        };

        match link {
            Some(Link::YouTube { video_id }) if is_youtube_id(&video_id) => {
                Ok(Link::YouTube { video_id })
            }
            Some(Link::X { status_id }) if is_status_id(&status_id) => Ok(Link::X { status_id }),
            _ => bail!(
                "unsupported link: {trimmed}\nSupported: YouTube videos (youtube.com/watch?v=…, youtu.be/…, /shorts/…) and X posts (x.com/<user>/status/<id>)"
            ),
        }
    }

    /// Canonical URL used for fetching and de-duplication.
    pub fn canonical_url(&self) -> String {
        match self {
            Link::YouTube { video_id } => format!("https://www.youtube.com/watch?v={video_id}"),
            Link::X { status_id } => format!("https://x.com/i/status/{status_id}"),
        }
    }
}

fn youtube(id: &str) -> Link {
    Link::YouTube {
        video_id: id.to_string(),
    }
}

fn is_youtube_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_status_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 25 && id.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yt(id: &str) -> Link {
        Link::YouTube {
            video_id: id.into(),
        }
    }

    #[test]
    fn parses_youtube_variants() {
        for url in [
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
            "https://youtube.com/watch?feature=share&v=jNQXAC9IVRw&t=10s",
            "youtube.com/watch?v=jNQXAC9IVRw",
            "https://m.youtube.com/watch?v=jNQXAC9IVRw",
            "https://youtu.be/jNQXAC9IVRw?si=abc",
            "https://www.youtube.com/shorts/jNQXAC9IVRw",
            "https://www.youtube.com/live/jNQXAC9IVRw?feature=share",
            "https://www.youtube.com/embed/jNQXAC9IVRw",
            "https://music.youtube.com/watch?v=jNQXAC9IVRw",
        ] {
            assert_eq!(Link::parse(url).unwrap(), yt("jNQXAC9IVRw"), "{url}");
        }
    }

    #[test]
    fn parses_x_variants() {
        for url in [
            "https://x.com/jack/status/20",
            "https://twitter.com/jack/status/20?s=46",
            "https://mobile.twitter.com/jack/status/20",
            "x.com/i/status/20",
            "https://fxtwitter.com/jack/status/20/photo/1",
            "https://x.com/jack/statuses/20",
        ] {
            assert_eq!(
                Link::parse(url).unwrap(),
                Link::X {
                    status_id: "20".into()
                },
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_unsupported_links() {
        for url in [
            "https://example.com/watch?v=jNQXAC9IVRw",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/@channel",
            "https://x.com/jack",
            "https://x.com/jack/status/not-a-number",
            "not a url at all",
        ] {
            assert!(Link::parse(url).is_err(), "{url}");
        }
    }

    #[test]
    fn canonical_urls() {
        assert_eq!(
            yt("jNQXAC9IVRw").canonical_url(),
            "https://www.youtube.com/watch?v=jNQXAC9IVRw"
        );
        assert_eq!(
            Link::X {
                status_id: "20".into()
            }
            .canonical_url(),
            "https://x.com/i/status/20"
        );
    }
}
