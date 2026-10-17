//! Long-running work (fetching a source, writing a lesson) on a worker thread.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use anyhow::{Result, anyhow};
use hil_core::llm::{BackendConfig, build_backend};
use hil_core::model::{Depth, Lesson, Source, Style};
use hil_core::source::{FetchOptions, fetch};

/// What the worker should turn into a lesson.
pub enum JobInput {
    /// A link that still needs downloading.
    Url(String),
    /// Text that was already extracted (restyle or cached link).
    Cached(Source),
}

pub struct JobRequest {
    pub input: JobInput,
    pub style: Style,
    pub depth: Depth,
}

/// What the UI sees when it polls a running job.
pub enum JobUpdate {
    Progress(String),
    Done(Box<Result<Lesson>>),
}

/// A running job; poll it from the UI thread.
pub struct Job {
    rx: Receiver<JobUpdate>,
}

impl Job {
    pub fn start(req: JobRequest) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            let progress = move |msg: &str| {
                let _ = progress_tx.send(JobUpdate::Progress(msg.to_string()));
            };
            let result = run(req, &progress);
            let _ = tx.send(JobUpdate::Done(Box::new(result)));
        });
        Self { rx }
    }

    /// Drains pending messages: the latest progress line, or the final result.
    pub fn poll(&self) -> Option<JobUpdate> {
        let mut latest = None;
        loop {
            match self.rx.try_recv() {
                Ok(JobUpdate::Progress(p)) => latest = Some(JobUpdate::Progress(p)),
                Ok(done) => return Some(done),
                Err(TryRecvError::Empty) => return latest,
                Err(TryRecvError::Disconnected) => {
                    let err = anyhow!("the background job stopped unexpectedly");
                    return Some(JobUpdate::Done(Box::new(Err(err))));
                }
            }
        }
    }
}

fn run(req: JobRequest, progress: &dyn Fn(&str)) -> Result<Lesson> {
    // Fail fast when no model is available, before any download.
    let backend = build_backend(&BackendConfig::from_env())?;
    let source = match req.input {
        JobInput::Url(url) => {
            progress("Fetching the source…");
            fetch(&url, &FetchOptions::default(), progress)?
        }
        JobInput::Cached(source) => source,
    };
    progress(&format!("Writing lesson with {}…", backend.label()));
    hil_core::generate_lesson(&source, req.style, req.depth, backend.as_ref())
}
