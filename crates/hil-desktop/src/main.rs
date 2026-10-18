//! Honey Interactive Learn desktop app (GPUI via gpui-kit).
//!
//! Usage: `hil-desktop [--select <lesson-id-prefix>] [--tab <overview|diagram|code|examples|cards|quiz|qa|glossary>] [--settings]`
//!        `hil-desktop --doctor` prints which tools and providers the app can see, then exits.

mod app;
mod cards_view;
mod content_view;
mod jobs;
mod lesson_view;
mod quiz_view;
mod settings_form;
mod settings_view;
mod shell_path;
mod sidebar;
mod ui;

use gpui_kit::component::Theme;
use gpui_kit::*;

use crate::app::{HilApp, StartOptions};
use crate::lesson_view::LessonTab;

fn parse_args() -> StartOptions {
    let mut opts = StartOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--select" => opts.select = args.next(),
            "--settings" => opts.settings = true,
            "--tab" => opts.tab = args.next().as_deref().and_then(LessonTab::parse),
            other => eprintln!("hil-desktop: ignoring unknown argument `{other}`"),
        }
    }
    opts
}

/// Prints what the app sees after importing the shell environment.
fn print_doctor() {
    let config = hil_core::llm::BackendConfig::load();
    for check in hil_core::doctor::run_checks(&config) {
        println!("{} {}", if check.ok { "ok  " } else { "MISS" }, check.name);
    }
    match config.resolve() {
        Ok(choice) => println!("provider: {}", choice.title()),
        Err(e) => println!("provider: none ({e})"),
    }
}

fn main() {
    shell_path::import_shell_env_for_gui_launch();
    if std::env::args().any(|a| a == "--doctor") {
        print_doctor();
        return;
    }
    let opts = parse_args();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Honey Interactive Learn".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::centered(size(px(1200.), px(800.)), cx)),
                window_min_size: Some(size(px(820.), px(560.))),
                ..Default::default()
            };
            let opened = gpui_kit::open_window(options, cx, |window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
                cx.new(|cx| HilApp::new(opts, window, cx))
            });
            if let Err(e) = opened {
                eprintln!("hil-desktop: could not open a window: {e:#}");
                cx.quit();
                return;
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
