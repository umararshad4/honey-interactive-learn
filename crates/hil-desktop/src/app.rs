//! Root view: owns the library, the selection and the running job.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Theme, h_flex};
use gpui_kit::*;
use hil_core::Library;
use hil_core::markdown::to_markdown;
use hil_core::model::{Depth, Lesson, Source, Style};
use hil_core::source::Link;

use crate::jobs::{Job, JobInput, JobRequest, JobUpdate};
use crate::lesson_view::{LessonTab, ViewState};
use crate::settings_view::{SettingsEvent, SettingsView};

/// Startup choices, mostly useful for screenshots and debugging.
#[derive(Default)]
pub struct StartOptions {
    /// Id (or id prefix) of the lesson to select on launch.
    pub select: Option<String>,
    pub tab: Option<LessonTab>,
    /// Open the Settings screen on launch.
    pub settings: bool,
}

struct RunningJob {
    status: String,
    _task: Task<()>,
}

pub struct HilApp {
    pub(crate) library: Option<Library>,
    pub(crate) lessons: Vec<Lesson>,
    pub(crate) selected: Option<String>,
    pub(crate) url_input: Entity<InputState>,
    pub(crate) style: Style,
    pub(crate) depth: Depth,
    job: Option<RunningJob>,
    pub(crate) error: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) view: ViewState,
    pub(crate) settings: Option<(Entity<SettingsView>, Subscription)>,
    _subscriptions: Vec<Subscription>,
}

impl HilApp {
    pub fn new(opts: StartOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Paste a YouTube or X link"));
        let subscriptions = vec![
            cx.subscribe_in(&url_input, window, |this, _, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.learn(cx);
                }
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];
        let (library, error) = match Library::open_default() {
            Ok(lib) => (Some(lib), None),
            Err(e) => (None, Some(format!("Could not open the library: {e:#}"))),
        };
        let mut app = Self {
            library,
            lessons: Vec::new(),
            selected: None,
            url_input,
            style: Style::default(),
            depth: Depth::default(),
            job: None,
            error,
            notice: None,
            view: ViewState::default(),
            settings: None,
            _subscriptions: subscriptions,
        };
        app.reload();
        if let Some(query) = opts.select
            && let Some(id) = app
                .lessons
                .iter()
                .find(|l| l.id.starts_with(&query))
                .map(|l| l.id.clone())
        {
            app.select(&id);
        }
        if let Some(tab) = opts.tab {
            app.view.tab = tab;
        }
        if opts.settings {
            app.open_settings(window, cx);
        }
        app
    }

    fn reload(&mut self) {
        if let Some(lib) = &self.library {
            match lib.list() {
                Ok(lessons) => self.lessons = lessons,
                Err(e) => self.error = Some(format!("Could not read the library: {e:#}")),
            }
        }
    }

    pub(crate) fn selected_lesson(&self) -> Option<&Lesson> {
        let id = self.selected.as_deref()?;
        self.lessons.iter().find(|l| l.id == id)
    }

    pub(crate) fn selected_lesson_mut(&mut self) -> Option<&mut Lesson> {
        let id = self.selected.clone()?;
        self.lessons.iter_mut().find(|l| l.id == id)
    }

    pub(crate) fn select(&mut self, id: &str) {
        let Some(lesson) = self.lessons.iter().find(|l| l.id == id) else {
            return;
        };
        self.view = ViewState::for_lesson(lesson, self.view.tab);
        self.selected = Some(id.to_string());
        self.notice = None;
        self.settings = None;
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.new(|cx| SettingsView::new(window, cx));
        let sub = cx.subscribe(&view, |this, _, ev: &SettingsEvent, cx| match ev {
            SettingsEvent::Close => {
                this.settings = None;
                cx.notify();
            }
        });
        self.settings = Some((view, sub));
        cx.notify();
    }

    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub(crate) fn job_status(&self) -> Option<&str> {
        self.job.as_ref().map(|j| j.status.as_str())
    }

    /// Starts a lesson from the link in the URL box.
    pub(crate) fn learn(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let url = self.url_input.read(cx).value().trim().to_string();
        if url.is_empty() {
            self.error = Some("Paste a YouTube or X link first.".into());
            cx.notify();
            return;
        }
        let canonical = match Link::parse(&url) {
            Ok(link) => link.canonical_url(),
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                cx.notify();
                return;
            }
        };
        let cached = self
            .library
            .as_ref()
            .and_then(|l| l.find_source(&canonical));
        let (input, status) = match cached {
            Some(source) => (JobInput::Cached(source), "Reusing the saved source text…"),
            None => (JobInput::Url(url), "Starting…"),
        };
        let (style, depth) = (self.style, self.depth);
        self.start_job(
            JobRequest {
                input,
                style,
                depth,
            },
            status,
            cx,
        );
    }

    /// Writes a new lesson from an existing source, without re-downloading.
    pub(crate) fn restyle(
        &mut self,
        source: Source,
        style: Style,
        depth: Depth,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let req = JobRequest {
            input: JobInput::Cached(source),
            style,
            depth,
        };
        self.start_job(req, "Restyling from the saved source text…", cx);
    }

    fn start_job(&mut self, req: JobRequest, status: &str, cx: &mut Context<Self>) {
        self.error = None;
        let job = Job::start(req);
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let update = job.poll();
                let running = this.update(cx, |app, cx| app.on_job_update(update, cx));
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        });
        self.job = Some(RunningJob {
            status: status.to_string(),
            _task: task,
        });
        cx.notify();
    }

    /// Applies a job update; returns whether the job is still running.
    fn on_job_update(&mut self, update: Option<JobUpdate>, cx: &mut Context<Self>) -> bool {
        match update {
            None => true,
            Some(JobUpdate::Progress(msg)) => {
                if let Some(job) = &mut self.job {
                    job.status = msg;
                }
                cx.notify();
                true
            }
            Some(JobUpdate::Done(result)) => {
                self.job = None;
                match *result {
                    Ok(lesson) => self.add_lesson(lesson),
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
                cx.notify();
                false
            }
        }
    }

    fn add_lesson(&mut self, lesson: Lesson) {
        let id = lesson.id.clone();
        match &self.library {
            Some(lib) => {
                if let Err(e) = lib.save(&lesson) {
                    self.error = Some(format!("Could not save the lesson: {e:#}"));
                }
                self.reload();
            }
            None => self.lessons.insert(0, lesson),
        }
        self.select(&id);
    }

    /// Saves the selected lesson (e.g. after a quiz attempt).
    pub(crate) fn save_selected(&mut self) {
        let (Some(lib), Some(lesson)) = (&self.library, self.selected_lesson()) else {
            return;
        };
        if let Err(e) = lib.save(lesson) {
            self.error = Some(format!("Could not save the lesson: {e:#}"));
        }
    }

    pub(crate) fn delete_lesson(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(lib) = &self.library
            && let Err(e) = lib.delete(id)
        {
            self.error = Some(format!("{e:#}"));
        }
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        self.reload();
        cx.notify();
    }

    pub(crate) fn export_markdown(&mut self, cx: &mut Context<Self>) {
        let Some(lesson) = self.selected_lesson() else {
            return;
        };
        let markdown = to_markdown(lesson);
        let name = format!("{}.md", lesson.id);
        let dir = downloads_dir();
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return, // cancelled
                // No save dialog available: fall back to the Downloads folder.
                _ => dir.join(&name),
            };
            let outcome = std::fs::write(&path, markdown);
            let _ = this.update(cx, |app, cx| {
                match outcome {
                    Ok(()) => app.notice = Some(format!("Saved to {}", path.display())),
                    Err(e) => app.error = Some(format!("Could not write {}: {e}", path.display())),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn downloads_dir() -> PathBuf {
    std::env::home_dir()
        .map(|h| h.join("Downloads"))
        .filter(|d| d.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

impl Render for HilApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .items_start()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(window, cx))
            .child(self.render_main(window, cx))
    }
}
