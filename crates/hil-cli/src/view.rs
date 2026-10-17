//! Terminal rendering of lessons and the library.

use chrono::Local;
use clap::ValueEnum;
use console::{Term, style};
use hil_core::markdown::option_letter;
use hil_core::model::Lesson;
use hil_core::source::format_timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Part {
    Summary,
    Diagram,
    Sections,
    Code,
    Examples,
    Glossary,
    Cards,
    Quiz,
    Questions,
}

pub fn print_lesson(lesson: &Lesson, only: &[Part], answers: bool) {
    let want = |p: Part| only.is_empty() || only.contains(&p);
    let c = &lesson.content;
    let s = &lesson.source;

    println!();
    println!("{}", style(&c.title).bold().yellow());
    let duration = s
        .duration_secs
        .map(|d| format!(" · {}", format_timestamp(d as f64)))
        .unwrap_or_default();
    println!(
        "{}",
        style(format!(
            "{} · {}{} · {} · {} / {}",
            s.kind, s.author, duration, s.url, lesson.style, lesson.depth
        ))
        .dim()
    );

    if want(Part::Summary) {
        heading("TL;DR");
        println!("{}", inline(c.tldr.trim()));
        if !c.key_points.is_empty() {
            heading("Key points");
            for p in &c.key_points {
                println!("  {} {}", style("•").yellow(), inline(p.trim()));
            }
        }
    }

    if want(Part::Diagram) && !c.diagram.ascii.is_empty() {
        heading(&format!("Diagram: {}", c.diagram.title));
        for line in c.diagram.ascii.lines() {
            println!("  {}", style(line).cyan());
        }
        if !c.diagram.mermaid.is_empty() {
            println!(
                "{}",
                style("  (Mermaid version included in `hil export`)").dim()
            );
        }
    }

    if want(Part::Sections) {
        for section in &c.sections {
            heading(&section.heading);
            print_markdown(&section.body);
        }
    }

    if want(Part::Code) && !c.code_snippets.is_empty() {
        heading("Code");
        for snip in &c.code_snippets {
            println!(
                "{} {}",
                style(&snip.title).bold(),
                style(format!("[{}]", snip.language)).dim()
            );
            for line in snip.code.trim_end().lines() {
                println!("  {} {}", style("│").dim(), style(line).green());
            }
            println!("{}\n", inline(snip.explanation.trim()));
        }
    }

    if want(Part::Examples) && !c.examples.is_empty() {
        heading("Real-world examples");
        for ex in &c.examples {
            println!(
                "  {} {}: {}",
                style("▸").yellow(),
                style(&ex.title).bold(),
                inline(ex.description.trim())
            );
        }
    }

    if want(Part::Glossary) && !c.glossary.is_empty() {
        heading("Glossary");
        let width = c
            .glossary
            .iter()
            .map(|t| t.term.chars().count())
            .max()
            .unwrap_or(0)
            .min(24);
        for t in &c.glossary {
            println!(
                "  {:<width$}  {}",
                style(&t.term).bold(),
                inline(t.definition.trim()),
                width = width
            );
        }
    }

    if want(Part::Cards) && !c.flashcards.is_empty() {
        heading(&format!("Flashcards ({})", c.flashcards.len()));
        for card in &c.flashcards {
            println!(
                "  {} {}",
                style("Q").yellow().bold(),
                inline(card.front.trim())
            );
            println!(
                "  {} {}\n",
                style("A").green().bold(),
                inline(card.back.trim())
            );
        }
    }

    if want(Part::Quiz) && !c.quiz.is_empty() {
        heading(&format!("Quiz ({} questions)", c.quiz.len()));
        for (i, q) in c.quiz.iter().enumerate() {
            println!(
                "{} {}",
                style(format!("{}.", i + 1)).bold(),
                q.question.trim()
            );
            for (j, opt) in q.options.iter().enumerate() {
                let line = format!("   {}) {}", option_letter(j), opt);
                if answers && j == q.answer_index {
                    println!("{}", style(line).green().bold());
                } else {
                    println!("{line}");
                }
            }
            if answers && !q.explanation.is_empty() {
                println!("   {}", style(q.explanation.trim()).dim());
            }
        }
        if !answers {
            println!(
                "{}",
                style("  Take it interactively with `hil quiz`, or add --answers.").dim()
            );
        }
    }

    if want(Part::Questions) && !c.questions.is_empty() {
        heading("Questions to think about");
        for (i, q) in c.questions.iter().enumerate() {
            println!(
                "{} {}",
                style(format!("{}.", i + 1)).bold(),
                q.question.trim()
            );
            if answers {
                println!("   {}", style(q.answer.trim()).dim());
            }
        }
    }
    println!();
}

pub fn print_list(lessons: &[Lesson]) {
    if lessons.is_empty() {
        println!("No lessons yet. Try: hil learn https://youtu.be/<video-id>");
        return;
    }
    let term = Term::stdout();
    let width = if term.is_term() {
        term.size().1 as usize
    } else {
        120
    };
    let title_width = width.saturating_sub(60).clamp(20, 70);
    for (i, l) in lessons.iter().enumerate() {
        let score = l
            .best_score()
            .map(|a| format!("best {}/{}", a.score, a.total))
            .unwrap_or_else(|| "no quiz yet".into());
        println!(
            "{:>3}  {}  {}  {}  {}",
            style(i + 1).bold(),
            style(truncate(&l.content.title, title_width)).yellow(),
            style(format!("{} · {}/{}", l.source.kind, l.style, l.depth)).dim(),
            style(score).dim(),
            style(l.created_at.with_timezone(&Local).format("%Y-%m-%d")).dim(),
        );
        println!("     {}", style(&l.id).cyan().dim());
    }
}

fn heading(text: &str) {
    println!("\n{}", style(text).bold().underlined());
}

/// Light Markdown: style headings and fenced code, leave the rest readable as-is.
fn print_markdown(body: &str) {
    let mut in_code = false;
    for line in body.trim().lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            println!("  {} {}", style("│").dim(), style(line).green());
        } else if let Some(h) = trimmed.strip_prefix('#') {
            println!("{}", style(h.trim_start_matches('#').trim()).bold());
        } else {
            println!("{}", inline(line));
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        format!("{s:<max$}")
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

/// Renders `**bold**` spans; other inline Markdown reads fine as-is.
pub fn inline(text: &str) -> String {
    if !text.contains("**") {
        return text.to_string();
    }
    text.split("**")
        .enumerate()
        .map(|(i, part)| {
            if i % 2 == 1 {
                style(part).bold().to_string()
            } else {
                part.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_bold_strips_markers() {
        console::set_colors_enabled(false);
        assert_eq!(inline("a **b** c"), "a b c");
        assert_eq!(inline("plain"), "plain");
    }

    #[test]
    fn truncate_pads_and_cuts() {
        assert_eq!(truncate("abc", 5), "abc  ");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
