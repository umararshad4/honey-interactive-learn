//! Honey Interactive Learn core: link → source text → lesson → library.

pub mod doctor;
pub mod library;
pub mod llm;
pub mod markdown;
pub mod model;
pub mod quiz;
pub mod settings;
pub mod source;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::llm::{GenerateRequest, LlmBackend, prompt};
use crate::model::{Depth, Lesson, LessonContent, Source, Style};

pub use library::Library;

/// Writes a lesson for `source` with the given backend.
pub fn generate_lesson(
    source: &Source,
    style: Style,
    depth: Depth,
    backend: &dyn LlmBackend,
) -> Result<Lesson> {
    let schema = prompt::lesson_schema();
    let user = prompt::user_prompt(source, style, depth);
    let value = backend.generate(&GenerateRequest {
        system: prompt::SYSTEM_PROMPT,
        user: &user,
        schema: &schema,
    })?;
    let content: LessonContent =
        serde_json::from_value(value).context("the model's lesson did not match the schema")?;
    let mut content = llm::normalize(content);
    if content.title.is_empty() {
        content.title = source.title.clone();
    }
    let created_at = Utc::now();
    Ok(Lesson {
        id: library::make_id(
            &content.title,
            &format!("{}{}{:?}", source.url, created_at.to_rfc3339(), style),
        ),
        created_at,
        style,
        depth,
        generator: backend.label(),
        source: source.clone(),
        content,
        attempts: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SourceKind, TranscriptOrigin};
    use serde_json::{Value, json};

    struct FakeBackend;

    impl LlmBackend for FakeBackend {
        fn label(&self) -> String {
            "fake".into()
        }
        fn generate(&self, req: &GenerateRequest) -> Result<Value> {
            assert!(req.user.contains("Style: VISUAL"));
            Ok(json!({
                "title": "  Ownership in Rust ",
                "tldr": "Values have one owner.",
                "key_points": ["One owner", ""],
                "sections": [],
                "diagram": {"title": "d", "ascii": "a -> b  \n", "mermaid": "flowchart LR\na-->b"},
                "code_snippets": [],
                "examples": [],
                "flashcards": [],
                "quiz": [
                    {"question": "Owners?", "options": ["1", "2"], "answer_index": 0, "explanation": ""},
                    {"question": "Bad", "options": ["1"], "answer_index": 3, "explanation": ""}
                ],
                "questions": [],
                "glossary": []
            }))
        }
    }

    #[test]
    fn generate_lesson_normalizes_and_labels() {
        let source = Source {
            url: "https://x.com/i/status/1".into(),
            kind: SourceKind::X,
            title: "post".into(),
            author: "a".into(),
            duration_secs: None,
            origin: TranscriptOrigin::PostText,
            text: "Ownership.".into(),
            truncated: false,
        };
        let lesson = generate_lesson(&source, Style::Visual, Depth::Brief, &FakeBackend).unwrap();
        assert_eq!(lesson.content.title, "Ownership in Rust");
        assert_eq!(lesson.content.key_points, vec!["One owner"]);
        assert_eq!(lesson.content.quiz.len(), 1);
        assert_eq!(lesson.content.diagram.ascii, "a -> b");
        assert_eq!(lesson.generator, "fake");
        assert!(lesson.id.starts_with("ownership-in-rust-"));
    }
}
