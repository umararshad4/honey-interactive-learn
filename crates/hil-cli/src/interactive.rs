//! Interactive quiz and flashcard sessions.

use anyhow::{Result, bail};
use console::{Term, style};
use dialoguer::{Confirm, Select};
use hil_core::Library;
use hil_core::markdown::option_letter;
use hil_core::model::Lesson;
use hil_core::quiz::QuizSession;

fn require_terminal() -> Result<()> {
    if !console::user_attended() {
        bail!(
            "this command is interactive; run it in a terminal (or use `hil show --only quiz --answers`)"
        );
    }
    Ok(())
}

pub fn confirm(prompt: &str) -> Result<bool> {
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(true)
        .interact()?)
}

pub fn run_quiz(mut lesson: Lesson, library: &Library) -> Result<()> {
    require_terminal()?;
    let mut session = QuizSession::new(lesson.content.quiz.clone());
    if session.is_empty() {
        bail!("this lesson has no quiz questions");
    }
    println!(
        "\n{} {}  {}\n",
        style("Quiz:").bold().yellow(),
        style(&lesson.content.title).bold(),
        style(format!("{} questions", session.len())).dim()
    );

    while let Some(q) = session.current().cloned() {
        let n = session.current_index() + 1;
        let items: Vec<String> = q
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| format!("{}) {o}", option_letter(i)))
            .collect();
        let choice = Select::new()
            .with_prompt(format!("{n}/{}  {}", session.len(), q.question.trim()))
            .items(&items)
            .default(0)
            .interact()?;
        if let Some(fb) = session.answer(choice) {
            if fb.correct {
                println!(
                    "  {} {}",
                    style("✓ Correct.").green().bold(),
                    q.explanation.trim()
                );
            } else {
                println!(
                    "  {} Answer: {}) {}\n  {}",
                    style("✗ Not quite.").red().bold(),
                    option_letter(fb.answer_index),
                    q.options[fb.answer_index],
                    style(q.explanation.trim()).dim()
                );
            }
        }
        println!();
        session.advance();
    }

    let attempt = session.attempt();
    let pct = attempt.score * 100 / attempt.total.max(1);
    let verdict = match pct {
        90..=100 => style("Excellent!").green().bold(),
        70..=89 => style("Solid.").green(),
        50..=69 => style("Getting there.").yellow(),
        _ => style("Worth another pass.").red(),
    };
    println!(
        "{} {}/{} ({pct}%)  {verdict}",
        style("Score:").bold(),
        attempt.score,
        attempt.total
    );
    let missed = session.missed();
    if !missed.is_empty() {
        println!("{}", style("Review these:").bold());
        for q in missed {
            println!(
                "  • {} → {}",
                q.question.trim(),
                style(&q.options[q.answer_index]).green()
            );
        }
    }
    lesson.attempts.push(attempt);
    library.save(&lesson)?;
    Ok(())
}

pub fn run_cards(lesson: &Lesson) -> Result<()> {
    require_terminal()?;
    let cards = &lesson.content.flashcards;
    if cards.is_empty() {
        bail!("this lesson has no flashcards");
    }
    let term = Term::stdout();
    println!(
        "\n{} {}  {}\n",
        style("Flashcards:").bold().yellow(),
        style(&lesson.content.title).bold(),
        style("Enter = flip/next · q = quit").dim()
    );
    let mut known = 0;
    for (i, card) in cards.iter().enumerate() {
        println!(
            "{} {}",
            style(format!("[{}/{}]", i + 1, cards.len())).dim(),
            style(card.front.trim()).bold()
        );
        if wait_key(&term)? {
            break;
        }
        println!("      {}", style(card.back.trim()).green());
        let got_it = Select::new()
            .with_prompt("Did you know it?")
            .items(["Yes", "Not yet", "Quit"])
            .default(0)
            .interact()?;
        match got_it {
            0 => known += 1,
            2 => break,
            _ => {}
        }
        println!();
    }
    println!("{} knew {known}/{}", style("Done:").bold(), cards.len());
    Ok(())
}

/// Waits for a key; returns true when the user pressed `q`.
fn wait_key(term: &Term) -> Result<bool> {
    let key = term.read_char()?;
    Ok(matches!(key, 'q' | 'Q'))
}
