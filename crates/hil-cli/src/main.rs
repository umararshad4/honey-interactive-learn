//! `hil`: learn from YouTube and X links in the terminal.

mod interactive;
mod view;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use console::style;
use hil_core::llm::{BackendChoice, BackendConfig, build_backend};
use hil_core::model::{Depth, Lesson, Source, Style};
use hil_core::source::{FetchOptions, Link};
use hil_core::{Library, generate_lesson};
use indicatif::{ProgressBar, ProgressStyle};

use crate::view::Part;

#[derive(Parser)]
#[command(
    name = "hil",
    version,
    about = "Honey Interactive Learn: turn YouTube videos and X posts into lessons, diagrams, flashcards and quizzes.",
    after_help = "Examples:\n  hil learn https://youtu.be/jNQXAC9IVRw\n  hil learn https://x.com/user/status/123 --style dev --depth comprehensive\n  hil quiz 1\n  hil export 1 -o lesson.md"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a lesson from a YouTube or X link.
    Learn {
        url: String,
        #[command(flatten)]
        gen_opts: GenArgs,
        /// Skip Whisper transcription for videos without captions.
        #[arg(long)]
        no_transcribe: bool,
        /// Download the source again even if it is in your library.
        #[arg(long)]
        refresh: bool,
        /// Start the quiz right after the lesson.
        #[arg(long, conflicts_with = "no_quiz")]
        quiz: bool,
        /// Do not offer the quiz afterwards.
        #[arg(long)]
        no_quiz: bool,
    },
    /// Rewrite a saved lesson in another style or depth (reuses the saved source).
    Restyle {
        /// Lesson number from `hil list` or id prefix.
        lesson: String,
        #[command(flatten)]
        gen_opts: GenArgs,
    },
    /// List saved lessons, newest first.
    #[command(alias = "ls")]
    List,
    /// Print a lesson (or some parts of it).
    Show {
        lesson: String,
        /// Only these parts (repeatable).
        #[arg(long = "only", value_enum)]
        only: Vec<Part>,
        /// Reveal quiz answers and model answers.
        #[arg(long)]
        answers: bool,
    },
    /// Take the multiple-choice quiz for a lesson.
    Quiz { lesson: String },
    /// Review flashcards one by one.
    Cards { lesson: String },
    /// Export a lesson as Markdown (with Mermaid diagram) or JSON.
    Export {
        lesson: String,
        /// Output file (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Export raw JSON instead of Markdown.
        #[arg(long)]
        json: bool,
    },
    /// Print the extracted text of a link without creating a lesson.
    Transcript {
        url: String,
        #[arg(long)]
        no_transcribe: bool,
    },
    /// Delete a lesson.
    #[command(alias = "delete")]
    Rm { lesson: String },
    /// Check that yt-dlp, whisper and a language model are available.
    Doctor,
}

#[derive(Args, Clone)]
struct GenArgs {
    /// plain, dev, visual or deep.
    #[arg(short, long, default_value = "plain", env = "HIL_STYLE")]
    style: Style,
    /// brief, compact or comprehensive.
    #[arg(short, long, default_value = "compact", env = "HIL_DEPTH")]
    depth: Depth,
    /// auto, claude-cli or api.
    #[arg(long, env = "HIL_BACKEND", default_value = "auto")]
    backend: BackendChoice,
    /// Model override (API default: claude-opus-5-5).
    #[arg(long, env = "HIL_MODEL")]
    model: Option<String>,
    /// API effort: low, medium, high, xhigh or max.
    #[arg(long, env = "HIL_EFFORT")]
    effort: Option<String>,
}

impl GenArgs {
    fn backend_config(&self) -> BackendConfig {
        BackendConfig {
            choice: self.backend,
            model: self.model.clone(),
            effort: self.effort.clone(),
        }
    }
}

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("{} {err:#}", style("error:").red().bold());
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Doctor => {
            doctor();
            Ok(())
        }
        Command::Transcript { url, no_transcribe } => {
            let source = fetch_with_spinner(&url, !no_transcribe)?;
            println!("{}", source.text);
            Ok(())
        }
        command => {
            let library = Library::open_default()?;
            run_with_library(command, &library)
        }
    }
}

fn run_with_library(command: Command, library: &Library) -> Result<()> {
    match command {
        Command::Learn {
            url,
            gen_opts,
            no_transcribe,
            refresh,
            quiz,
            no_quiz,
        } => {
            let canonical = Link::parse(&url)?.canonical_url();
            let source = match library.find_source(&canonical).filter(|_| !refresh) {
                Some(cached) => {
                    eprintln!(
                        "{} using saved source text (pass --refresh to re-download)",
                        style("•").dim()
                    );
                    cached
                }
                None => fetch_with_spinner(&url, !no_transcribe)?,
            };
            let lesson = write_lesson(&source, &gen_opts, library)?;
            view::print_lesson(&lesson, &[], false);
            next_steps(&lesson);
            let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
            let take_quiz = quiz
                || (!no_quiz
                    && interactive
                    && !lesson.content.quiz.is_empty()
                    && interactive::confirm("Take the quiz now?")?);
            if take_quiz {
                interactive::run_quiz(lesson, library)?;
            }
            Ok(())
        }
        Command::Restyle { lesson, gen_opts } => {
            let old = library.resolve(&lesson)?;
            let lesson = write_lesson(&old.source, &gen_opts, library)?;
            view::print_lesson(&lesson, &[], false);
            next_steps(&lesson);
            Ok(())
        }
        Command::List => {
            view::print_list(&library.list()?);
            Ok(())
        }
        Command::Show {
            lesson,
            only,
            answers,
        } => {
            view::print_lesson(&library.resolve(&lesson)?, &only, answers);
            Ok(())
        }
        Command::Quiz { lesson } => interactive::run_quiz(library.resolve(&lesson)?, library),
        Command::Cards { lesson } => interactive::run_cards(&library.resolve(&lesson)?),
        Command::Export {
            lesson,
            output,
            json,
        } => {
            let lesson = library.resolve(&lesson)?;
            let text = if json {
                serde_json::to_string_pretty(&lesson)? + "\n"
            } else {
                hil_core::markdown::to_markdown(&lesson)
            };
            match output {
                Some(path) => {
                    std::fs::write(&path, text)
                        .with_context(|| format!("could not write {}", path.display()))?;
                    eprintln!("{} wrote {}", style("✓").green(), path.display());
                }
                None => print!("{text}"),
            }
            Ok(())
        }
        Command::Rm { lesson } => {
            let lesson = library.resolve(&lesson)?;
            library.delete(&lesson.id)?;
            eprintln!(
                "{} deleted {} ({})",
                style("✓").green(),
                lesson.content.title,
                lesson.id
            );
            Ok(())
        }
        Command::Doctor | Command::Transcript { .. } => unreachable!("handled in run"),
    }
}

fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.yellow} {msg} {elapsed:.dim}")
            .expect("valid template"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(message.to_string());
    pb
}

fn fetch_with_spinner(url: &str, transcribe: bool) -> Result<Source> {
    let pb = spinner("Reading link…");
    let result = hil_core::source::fetch(url, &FetchOptions { transcribe }, &|msg| {
        pb.set_message(msg.to_string())
    });
    pb.finish_and_clear();
    let source = result?;
    eprintln!(
        "{} {} · {} · {} chars of {}{}",
        style("✓").green(),
        style(&source.title).bold(),
        source.author,
        source.text.chars().count(),
        source.origin,
        if source.truncated { " (truncated)" } else { "" }
    );
    Ok(source)
}

fn write_lesson(source: &Source, opts: &GenArgs, library: &Library) -> Result<Lesson> {
    let backend = build_backend(&opts.backend_config())?;
    let pb = spinner(&format!(
        "Writing a {} {} lesson with {}…",
        opts.depth,
        opts.style,
        backend.label()
    ));
    let result = generate_lesson(source, opts.style, opts.depth, backend.as_ref());
    pb.finish_and_clear();
    let lesson = result?;
    library.save(&lesson)?;
    Ok(lesson)
}

fn next_steps(lesson: &Lesson) {
    eprintln!(
        "\n{} saved as {}\n  next: {}  {}  {}  {}",
        style("✓").green(),
        style(&lesson.id).cyan(),
        style(format!("hil quiz {}", lesson.id)).dim(),
        style(format!("hil cards {}", lesson.id)).dim(),
        style(format!("hil export {} -o lesson.md", lesson.id)).dim(),
        style(format!("hil restyle {} -s dev", lesson.id)).dim(),
    );
}

fn doctor() {
    println!(
        "{}",
        style("Honey Interactive Learn: environment check").bold()
    );
    for check in hil_core::doctor::run_checks() {
        let mark = if check.ok {
            style("✓").green()
        } else {
            style("✗").red()
        };
        println!("  {mark} {:<18} {}", check.name, style(check.purpose).dim());
        if !check.ok {
            println!("    {} {}", style("→").yellow(), check.hint);
        }
    }
    match Library::open_default() {
        Ok(lib) => println!("  {} library: {}", style("•").dim(), lib.dir().display()),
        Err(e) => println!("  {} library: {e:#}", style("✗").red()),
    }
}
