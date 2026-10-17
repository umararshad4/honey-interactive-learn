//! Markdown export. Mermaid renders on GitHub, Obsidian, GitLab and VS Code.

use std::fmt::Write;

use crate::model::Lesson;
use crate::source::format_timestamp;

pub fn to_markdown(lesson: &Lesson) -> String {
    let c = &lesson.content;
    let s = &lesson.source;
    let mut md = String::new();
    // `write!` into a String cannot fail.
    let _ = writeln!(md, "# {}\n", c.title);
    let duration = s
        .duration_secs
        .map(|d| format!(" · {}", format_timestamp(d as f64)))
        .unwrap_or_default();
    let _ = writeln!(
        md,
        "> **Source:** [{}]({}) by {}{} · {} · style `{}` · depth `{}`\n",
        s.title, s.url, s.author, duration, s.kind, lesson.style, lesson.depth
    );

    let _ = writeln!(md, "## TL;DR\n\n{}\n", c.tldr);

    if !c.key_points.is_empty() {
        md.push_str("## Key points\n\n");
        for p in &c.key_points {
            let _ = writeln!(md, "- {p}");
        }
        md.push('\n');
    }

    if !c.diagram.ascii.is_empty() || !c.diagram.mermaid.is_empty() {
        let _ = writeln!(md, "## Diagram: {}\n", c.diagram.title);
        if !c.diagram.mermaid.is_empty() {
            let _ = writeln!(md, "```mermaid\n{}\n```\n", c.diagram.mermaid);
        }
        if !c.diagram.ascii.is_empty() {
            let _ = writeln!(
                md,
                "<details><summary>Text diagram</summary>\n\n```text\n{}\n```\n\n</details>\n",
                c.diagram.ascii
            );
        }
    }

    for section in &c.sections {
        let _ = writeln!(md, "## {}\n\n{}\n", section.heading, section.body.trim());
    }

    if !c.code_snippets.is_empty() {
        md.push_str("## Code\n\n");
        for snip in &c.code_snippets {
            let _ = writeln!(
                md,
                "### {}\n\n```{}\n{}\n```\n\n{}\n",
                snip.title,
                snip.language,
                snip.code.trim_end(),
                snip.explanation
            );
        }
    }

    if !c.examples.is_empty() {
        md.push_str("## Real-world examples\n\n");
        for ex in &c.examples {
            let _ = writeln!(md, "- **{}**: {}", ex.title, ex.description);
        }
        md.push('\n');
    }

    if !c.glossary.is_empty() {
        md.push_str("## Glossary\n\n| Term | Meaning |\n|---|---|\n");
        for t in &c.glossary {
            let _ = writeln!(md, "| {} | {} |", cell(&t.term), cell(&t.definition));
        }
        md.push('\n');
    }

    if !c.flashcards.is_empty() {
        md.push_str("## Flashcards\n\n");
        for card in &c.flashcards {
            let _ = writeln!(
                md,
                "<details><summary>{}</summary>\n\n{}\n\n</details>\n",
                card.front, card.back
            );
        }
    }

    if !c.quiz.is_empty() {
        md.push_str("## Quiz\n\n");
        for (i, q) in c.quiz.iter().enumerate() {
            let _ = writeln!(md, "**{}. {}**\n", i + 1, q.question);
            for (j, opt) in q.options.iter().enumerate() {
                let _ = writeln!(md, "- {}) {}", option_letter(j), opt);
            }
            let _ = writeln!(
                md,
                "\n<details><summary>Answer</summary>\n\n**{})** {}\n\n</details>\n",
                option_letter(q.answer_index),
                q.explanation
            );
        }
    }

    if !c.questions.is_empty() {
        md.push_str("## Questions to think about\n\n");
        for (i, q) in c.questions.iter().enumerate() {
            let _ = writeln!(
                md,
                "{}. {}\n\n   <details><summary>Model answer</summary>\n\n   {}\n\n   </details>\n",
                i + 1,
                q.question,
                q.answer.replace('\n', "\n   ")
            );
        }
    }

    let _ = writeln!(
        md,
        "---\n*Made with [Honey Interactive Learn](https://github.com/umararshad4/honey-interactive-learn) · {} · {}*",
        lesson.generator,
        lesson.created_at.format("%Y-%m-%d")
    );
    md
}

pub fn option_letter(i: usize) -> char {
    (b'A' + (i % 26) as u8) as char
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use chrono::Utc;

    #[test]
    fn renders_every_part() {
        let lesson = Lesson {
            id: "id".into(),
            created_at: Utc::now(),
            style: Style::Dev,
            depth: Depth::Compact,
            generator: "claude-cli".into(),
            source: Source {
                url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(),
                kind: SourceKind::YouTube,
                title: "Me at the zoo".into(),
                author: "jawed".into(),
                duration_secs: Some(19),
                origin: TranscriptOrigin::Captions,
                text: String::new(),
                truncated: false,
            },
            content: LessonContent {
                title: "Elephants 101".into(),
                tldr: "Long trunks.".into(),
                key_points: vec!["Trunks are long".into()],
                sections: vec![Section {
                    heading: "Trunks".into(),
                    body: "Very long.".into(),
                }],
                diagram: Diagram {
                    title: "Anatomy".into(),
                    ascii: "[head]--[trunk]".into(),
                    mermaid: "flowchart LR\nhead-->trunk".into(),
                },
                code_snippets: vec![CodeSnippet {
                    title: "Model".into(),
                    language: "rust".into(),
                    code: "struct Trunk;".into(),
                    explanation: "A type.".into(),
                }],
                examples: vec![Example {
                    title: "Zoo".into(),
                    description: "San Diego".into(),
                }],
                flashcards: vec![Flashcard {
                    front: "Trunk?".into(),
                    back: "Nose".into(),
                }],
                quiz: vec![Mcq {
                    question: "What is long?".into(),
                    options: vec!["Tail".into(), "Trunk".into()],
                    answer_index: 1,
                    explanation: "Trunks are long.".into(),
                }],
                questions: vec![OpenQuestion {
                    question: "Why?".into(),
                    answer: "Reach.".into(),
                }],
                glossary: vec![Term {
                    term: "Trunk".into(),
                    definition: "Nose | lip".into(),
                }],
            },
            attempts: vec![],
        };
        let md = to_markdown(&lesson);
        for needle in [
            "# Elephants 101",
            "by jawed · 00:19 · YouTube · style `dev` · depth `compact`",
            "```mermaid\nflowchart LR\nhead-->trunk\n```",
            "```text\n[head]--[trunk]\n```",
            "## Trunks\n\nVery long.",
            "```rust\nstruct Trunk;\n```",
            "- **Zoo**: San Diego",
            "| Trunk | Nose \\| lip |",
            "- B) Trunk",
            "**B)** Trunks are long.",
            "1. Why?",
        ] {
            assert!(md.contains(needle), "missing {needle:?} in\n{md}");
        }
    }
}
