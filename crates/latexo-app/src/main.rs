//! Latexo, the system monitor. Binary entry point: the command line, the settings, the log, the
//! language, the theme and the main window.

// On Windows `latexo.exe` is a console subsystem binary, as Postino and ted: shells wait for
// `latexo --help` and read its exit code. The manifest merged at release keeps Windows 11 from
// opening a console for a double click, and `detach_from_console` frees the terminal.

mod state;
mod theme;
mod views;

rust_i18n::i18n!("locales", fallback = "en");

use std::path::Path;
use std::process::ExitCode;

use gpui_kit::component::Root;
use gpui_kit::*;
use latexo_cli::IDENTITY;
use telaio_app::{logging, window};

use state::settings::LogLevel;
use theme::metrics::MIN_WINDOW;
use views::AppView;

fn main() -> ExitCode {
    // `--help`, `--version` and a wrong argument print and exit before anything else.
    if let Err(code) = latexo_cli::start(std::env::args_os().skip(1)) {
        return code;
    }
    if telaio_app::cli::detach_from_console() == telaio_app::cli::GuiLaunch::Relaunched {
        return ExitCode::SUCCESS;
    }

    // Settings first, for the log level: the logger is installed before anything else runs.
    let settings = state::settings::load_settings();
    let log_env_var = logging::env_var(&IDENTITY);
    let env_level = std::env::var(&log_env_var).ok();
    let log_level = logging::startup_level(settings.log_level, env_level.as_deref());
    let log_path = logging::init(&IDENTITY, log_level.to_filter());
    let from_env = env_level.as_deref().and_then(LogLevel::parse).is_some();
    log_startup(
        log_level,
        from_env.then_some(log_env_var.as_str()),
        log_path.as_deref(),
    );
    if let Some(value) = env_level.filter(|_| !from_env) {
        log::warn!("ignoring {log_env_var}={value:?}: not a log level");
    }

    // Before the window opens, so its first frame is already in the right language.
    let language = state::locale::resolve(settings.language);
    state::locale::apply(language);
    log::info!(
        "language: {} ({}, system locales: {})",
        language.code(),
        match settings.language.language() {
            Some(_) => "fixed",
            None => "auto",
        },
        state::locale::system_locales().join(", "),
    );

    let screen = state::nav::startup_screen();
    let theme_choice = settings.theme;
    let app = window::application();
    app.run(move |cx| {
        window::init(cx, &IDENTITY);
        theme::install(cx);
        let sizes = window::WindowSizes {
            min: MIN_WINDOW,
            ..window::WindowSizes::default()
        };
        let window_options = window::options(cx, &IDENTITY, sizes);
        cx.spawn(async move |cx| {
            let opened = cx.open_window(window_options, move |window, cx| {
                telaio_ui::ThemeChoice::from(theme_choice).apply(Some(window), cx);
                let view = cx.new(|cx| AppView::new(settings, screen, window, cx));
                // Always listening: Settings can switch to "System" at any time.
                let weak = view.downgrade();
                window::follow_system_appearance(window, move |cx| {
                    weak.upgrade()
                        .is_some_and(|view| view.read(cx).follows_system())
                });
                let closing = view.downgrade();
                cx.on_window_closed(move |cx, _| {
                    if let Some(view) = closing.upgrade() {
                        view.read(cx).on_close();
                    }
                })
                .detach();
                cx.new(|cx| Root::new(view, window, cx))
            });
            // Opening the very first window failing is not recoverable.
            #[allow(clippy::expect_used)]
            opened.expect("failed to open the main window");
        })
        .detach();
    });
    ExitCode::SUCCESS
}

/// Logs where and how Latexo runs: the first lines of every session.
fn log_startup(level: LogLevel, from_env: Option<&str>, log_path: Option<&Path>) {
    log::info!(
        "Latexo {} starting ({} build) on {} {}",
        latexo_cli::version_label(),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    log::info!(
        "log level {level:?}{}, log file {}",
        from_env.map_or_else(String::new, |variable| format!(" (from {variable})")),
        log_path.map_or_else(|| "none".to_string(), |path| path.display().to_string()),
    );
    log::info!(
        "settings file {}",
        state::settings::settings_path()
            .map_or_else(|| "unknown".to_string(), |path| path.display().to_string()),
    );
}
