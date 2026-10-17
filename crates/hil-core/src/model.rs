//! Data types shared by the CLI and the desktop app.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where a piece of content came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    YouTube,
    X,
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SourceKind::YouTube => "YouTube",
            SourceKind::X => "X",
        })
    }
}

/// How the text of a source was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptOrigin {
    Captions,
    AutoCaptions,
    Whisper,
    PostText,
}

impl fmt::Display for TranscriptOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TranscriptOrigin::Captions => "captions",
            TranscriptOrigin::AutoCaptions => "auto-captions",
            TranscriptOrigin::Whisper => "whisper transcript",
            TranscriptOrigin::PostText => "post text",
        })
    }
}

/// Extracted content of a link, ready to be turned into a lesson.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub kind: SourceKind,
    pub title: String,
    pub author: String,
    #[serde(default)]
    pub duration_secs: Option<u64>,
    pub origin: TranscriptOrigin,
    /// Full text handed to the model (transcript, post text, description...).
    pub text: String,
    /// True when `text` was cut to fit the model budget.
    #[serde(default)]
    pub truncated: bool,
}

/// Voice of the lesson.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Everyday language, analogies, no unexplained jargon.
    #[default]
    Plain,
    /// Developer-oriented: precise terms, code, commands, trade-offs.
    Dev,
    /// Diagram-first: flows, hierarchies, tables.
    Visual,
    /// First principles, edge cases, misconceptions.
    Deep,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Plain, Style::Dev, Style::Visual, Style::Deep];

    pub fn label(self) -> &'static str {
        match self {
            Style::Plain => "plain",
            Style::Dev => "dev",
            Style::Visual => "visual",
            Style::Deep => "deep",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Style::Plain => "Readable, everyday language with analogies",
            Style::Dev => "Developer view with code snippets and trade-offs",
            Style::Visual => "Diagram-first: flows, hierarchies and tables",
            Style::Deep => "First principles, edge cases and misconceptions",
        }
    }
}

/// How much the lesson covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Depth {
    Brief,
    #[default]
    Compact,
    Comprehensive,
}

impl Depth {
    pub const ALL: [Depth; 3] = [Depth::Brief, Depth::Compact, Depth::Comprehensive];

    pub fn label(self) -> &'static str {
        match self {
            Depth::Brief => "brief",
            Depth::Compact => "compact",
            Depth::Comprehensive => "comprehensive",
        }
    }
}

macro_rules! label_enum_impls {
    ($ty:ty) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.label())
            }
        }

        impl FromStr for $ty {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let wanted = s.trim().to_ascii_lowercase();
                Self::ALL
                    .into_iter()
                    .find(|v| v.label() == wanted)
                    .ok_or_else(|| {
                        let names: Vec<_> = Self::ALL.iter().map(|v| v.label()).collect();
                        format!("unknown value `{s}`, expected one of: {}", names.join(", "))
                    })
            }
        }
    };
}

label_enum_impls!(Style);
label_enum_impls!(Depth);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Diagram {
    pub title: String,
    /// Monospace diagram that renders anywhere.
    pub ascii: String,
    /// Mermaid source for GitHub, Obsidian and other Markdown viewers.
    pub mermaid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub title: String,
    pub language: String,
    pub code: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flashcard {
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mcq {
    pub question: String,
    pub options: Vec<String>,
    /// Zero-based index into `options`.
    pub answer_index: usize,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenQuestion {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    pub definition: String,
}

/// Everything the model writes for one lesson.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LessonContent {
    pub title: String,
    pub tldr: String,
    pub key_points: Vec<String>,
    pub sections: Vec<Section>,
    pub diagram: Diagram,
    pub code_snippets: Vec<CodeSnippet>,
    pub examples: Vec<Example>,
    pub flashcards: Vec<Flashcard>,
    pub quiz: Vec<Mcq>,
    pub questions: Vec<OpenQuestion>,
    pub glossary: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuizAttempt {
    pub at: DateTime<Utc>,
    pub score: usize,
    pub total: usize,
}

/// A saved lesson: the source, the generated content and quiz history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lesson {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub style: Style,
    pub depth: Depth,
    /// Backend and model that wrote the lesson, e.g. `claude-cli` or `api:claude-opus-5-5`.
    pub generator: String,
    pub source: Source,
    pub content: LessonContent,
    #[serde(default)]
    pub attempts: Vec<QuizAttempt>,
}

impl Lesson {
    pub fn best_score(&self) -> Option<&QuizAttempt> {
        self.attempts
            .iter()
            .max_by(|a, b| (a.score * b.total).cmp(&(b.score * a.total)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_and_depth_parse_case_insensitively() {
        assert_eq!("DEV".parse::<Style>().unwrap(), Style::Dev);
        assert_eq!(
            " comprehensive ".parse::<Depth>().unwrap(),
            Depth::Comprehensive
        );
        let err = "fancy".parse::<Style>().unwrap_err();
        assert!(err.contains("plain, dev, visual, deep"), "{err}");
    }

    #[test]
    fn lesson_content_tolerates_missing_fields() {
        let content: LessonContent = serde_json::from_str(r#"{"title":"T"}"#).unwrap();
        assert_eq!(content.title, "T");
        assert!(content.quiz.is_empty());
    }
}
