//! Prompt and JSON schema for lesson generation.

use serde_json::{Value, json};

use crate::model::{Depth, Source, Style};

pub const SYSTEM_PROMPT: &str = "You are an expert teacher who turns videos and social posts into \
lessons people remember. Teach the ideas in the source, not the source itself: never write \
\"the video says\" or \"the author mentions\". Stay faithful to the source; when you add context \
the source lacks, keep it short and accurate. Write Markdown inside string fields where it helps \
(bold, lists, inline code). Respond only with JSON matching the schema.";

/// Counts that scale with depth: (key points, sections, quiz, open questions, flashcards, words).
fn depth_targets(
    depth: Depth,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    match depth {
        Depth::Brief => ("3-5", "1-2", "3", "2", "4-5", "250-400"),
        Depth::Compact => ("5-7", "3-4", "5", "3-4", "6-8", "600-900"),
        Depth::Comprehensive => ("7-10", "5-8", "10", "6-8", "10-15", "1500-2500"),
    }
}

fn style_rules(style: Style) -> &'static str {
    match style {
        Style::Plain => {
            "Style: PLAIN. Use everyday language a curious 16-year-old understands. \
Define every technical term the first time it appears. Lean on analogies and concrete \
real-world examples. Include code_snippets only if the topic is about code."
        }
        Style::Dev => {
            "Style: DEV. Write for software engineers. Use precise terminology, name the \
APIs, tools, commands and patterns involved, and discuss trade-offs, pitfalls and performance. \
Provide 2-5 runnable, idiomatic code_snippets (or shell commands/config) that demonstrate the \
ideas, each with a short explanation. If the topic is not about code, use snippets for \
pseudo-code, data shapes or commands that make the idea concrete."
        }
        Style::Visual => {
            "Style: VISUAL. Lead with structure. Make the diagram the centrepiece and \
detailed. Inside section bodies, use Markdown tables, numbered flows (A -> B -> C) and indented \
hierarchies instead of long paragraphs. Keep prose minimal."
        }
        Style::Deep => {
            "Style: DEEP. Teach from first principles. Explain why things work, not \
only what they are. Cover edge cases, common misconceptions, limits of the claims in the source \
and how the ideas connect to related concepts."
        }
    }
}

pub fn user_prompt(source: &Source, style: Style, depth: Depth) -> String {
    let (points, sections, quiz, questions, cards, words) = depth_targets(depth);
    let truncated = if source.truncated {
        "\nNote: the source was cut short because it is very long; teach what is present.\n"
    } else {
        ""
    };
    format!(
        "Create a lesson from this {kind} source.

{style_rules}
Depth: {depth}. Aim for about {words} words across tldr and sections.

Fill the schema like this:
- title: a short, specific lesson title (not the source title verbatim if it is clickbait).
- tldr: 2-3 sentences with the core takeaway.
- key_points: {points} crisp bullet points.
- sections: {sections} sections, each a heading and a Markdown body.
- diagram: one diagram of the central mechanism or structure. `ascii` uses box-drawing or \
ASCII characters, max 72 columns wide, and must line up in a monospace font. `mermaid` is \
valid Mermaid source (flowchart, sequenceDiagram or mindmap) without code fences.
- code_snippets: follow the style rules above (empty array when not applicable).
- examples: 2-4 real-world examples or applications.
- flashcards: {cards} cards, front is a question or term, back is a short answer.
- quiz: exactly {quiz} multiple-choice questions with 4 options each. answer_index is the \
0-based index of the single correct option. Vary the correct position. Distractors must be \
plausible. explanation says why the answer is right.
- questions: {questions} open questions that test understanding, each with a model answer.
- glossary: key terms with one-line definitions (empty array if none).
{truncated}
Source title: {title}
Author: {author}
URL: {url}
Text origin: {origin}

<source>
{text}
</source>",
        kind = source.kind,
        style_rules = style_rules(style),
        title = source.title,
        author = source.author,
        url = source.url,
        origin = source.origin,
        text = source.text,
    )
}

/// JSON schema for `LessonContent`. Uses only features structured outputs support.
pub fn lesson_schema() -> Value {
    fn obj(props: Value) -> Value {
        let required: Vec<String> = props
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
    }
    let s = || json!({"type": "string"});
    let arr = |items: Value| json!({"type": "array", "items": items});

    obj(json!({
        "title": s(),
        "tldr": s(),
        "key_points": arr(s()),
        "sections": arr(obj(json!({"heading": s(), "body": s()}))),
        "diagram": obj(json!({"title": s(), "ascii": s(), "mermaid": s()})),
        "code_snippets": arr(obj(json!({"title": s(), "language": s(), "code": s(), "explanation": s()}))),
        "examples": arr(obj(json!({"title": s(), "description": s()}))),
        "flashcards": arr(obj(json!({"front": s(), "back": s()}))),
        "quiz": arr(obj(json!({
            "question": s(),
            "options": arr(s()),
            "answer_index": {"type": "integer"},
            "explanation": s()
        }))),
        "questions": arr(obj(json!({"question": s(), "answer": s()}))),
        "glossary": arr(obj(json!({"term": s(), "definition": s()}))),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LessonContent, SourceKind, TranscriptOrigin};

    fn source() -> Source {
        Source {
            url: "https://x.com/i/status/1".into(),
            kind: SourceKind::X,
            title: "Ownership".into(),
            author: "Dev (@dev)".into(),
            duration_secs: None,
            origin: TranscriptOrigin::PostText,
            text: "Ownership moves values.".into(),
            truncated: true,
        }
    }

    #[test]
    fn prompt_carries_style_depth_and_source() {
        let p = user_prompt(&source(), Style::Dev, Depth::Brief);
        assert!(p.contains("Style: DEV"));
        assert!(p.contains("exactly 3 multiple-choice"));
        assert!(p.contains("<source>\nOwnership moves values.\n</source>"));
        assert!(p.contains("cut short"));
    }

    #[test]
    fn schema_requires_every_field_and_forbids_extras() {
        fn check(v: &Value) {
            if v["type"] == "object" {
                assert_eq!(v["additionalProperties"], false);
                let props = v["properties"].as_object().unwrap();
                assert_eq!(v["required"].as_array().unwrap().len(), props.len());
                props.values().for_each(check);
            } else if v["type"] == "array" {
                check(&v["items"]);
            }
        }
        let schema = lesson_schema();
        check(&schema);
        // Every schema field maps onto LessonContent.
        let empty = serde_json::to_value(LessonContent::default()).unwrap();
        let mut a: Vec<_> = schema["properties"].as_object().unwrap().keys().collect();
        let mut b: Vec<_> = empty.as_object().unwrap().keys().collect();
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }
}
