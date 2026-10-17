//! WebVTT captions to readable transcript text.
//!
//! YouTube auto-captions repeat each line across rolling cues and embed
//! word-timing tags, so we strip tags, drop repeats and group the text into
//! paragraphs that start with a `[mm:ss]` marker.

/// Seconds of speech grouped under one timestamp marker.
const PARAGRAPH_SECS: f64 = 60.0;

pub fn vtt_to_transcript(vtt: &str) -> String {
    let mut paragraphs: Vec<(f64, Vec<String>)> = Vec::new();
    let mut cue_start: Option<f64> = None;
    let mut last_line = String::new();

    for raw in vtt.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((start, _)) = line.split_once("-->") {
            cue_start = parse_timestamp(start.trim());
            continue;
        }
        let Some(start) = cue_start else {
            // Header block (WEBVTT, Kind:, Language:, NOTE, STYLE...).
            continue;
        };
        let text = clean_line(line);
        if text.is_empty() || text == last_line {
            continue;
        }
        last_line = text.clone();

        match paragraphs.last_mut() {
            Some((para_start, lines)) if start - *para_start < PARAGRAPH_SECS => lines.push(text),
            _ => paragraphs.push((start, vec![text])),
        }
    }

    paragraphs
        .into_iter()
        .map(|(start, lines)| format!("[{}] {}", format_timestamp(start), lines.join(" ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn clean_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for c in line.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&quot;", "\"");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parses `hh:mm:ss.mmm` or `mm:ss.mmm` into seconds.
fn parse_timestamp(s: &str) -> Option<f64> {
    let s = s.split_whitespace().next()?;
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.replace(',', ".").parse::<f64>().ok()?;
    }
    Some(total)
}

pub fn format_timestamp(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_captions_become_one_paragraph_per_minute() {
        let vtt = "WEBVTT\nKind: captions\nLanguage: en\n\n\
00:00:01.200 --> 00:00:03.360\nAll right, so here we are, in front of the\nelephants\n\n\
00:00:05.318 --> 00:00:07.974\nthe cool thing about these guys\n\n\
00:01:05.000 --> 00:01:07.000\nlater &amp; louder\n";
        assert_eq!(
            vtt_to_transcript(vtt),
            "[00:01] All right, so here we are, in front of the elephants the cool thing about these guys\n\
[01:05] later & louder"
        );
    }

    #[test]
    fn auto_captions_drop_word_tags_and_rolling_repeats() {
        let vtt = "WEBVTT\n\n\
00:00:00.000 --> 00:00:02.000 align:start position:0%\n\
hello<00:00:00.500><c> world</c>\n\n\
00:00:02.000 --> 00:00:02.010 align:start position:0%\n\
hello world\n\n\
00:00:02.010 --> 00:00:04.000 align:start position:0%\n\
hello world\n\
this<00:00:02.500><c> is</c><00:00:03.000><c> rust</c>\n";
        assert_eq!(vtt_to_transcript(vtt), "[00:00] hello world this is rust");
    }

    #[test]
    fn timestamps() {
        assert_eq!(parse_timestamp("01:02:03.500"), Some(3723.5));
        assert_eq!(parse_timestamp("02:03.000 align:start"), Some(123.0));
        assert_eq!(format_timestamp(3723.5), "1:02:03");
        assert_eq!(format_timestamp(65.0), "01:05");
    }
}
