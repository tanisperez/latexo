//! Latexo, the system monitor. Binary entry point: the command line, the settings, the log, the
//! language, the theme and the main window.

// On Windows `latexo.exe` is a console subsystem binary, as Postino and ted: shells wait for
// `latexo --help` and read its exit code. The manifest merged at release keeps Windows 11 from
// opening a console for a double click, and `detach_from_console` frees the terminal.

mod actions;
mod state;
mod theme;
mod views;

rust_i18n::i18n!("locales", fallback = "en");

use std::cell::Cell;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use gpui_kit::component::Root;
use gpui_kit::*;
use latexo_cli::IDENTITY;
use telaio_app::{logging, window};

use state::settings::LogLevel;
use state::window::WindowState;
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
        actions::bind_keys(cx);
        let sizes = window::WindowSizes {
            min: MIN_WINDOW,
            ..window::WindowSizes::default()
        };
        let mut window_options = window::options(cx, &IDENTITY, sizes);
        let saved_window = state::window::load();
        if let Some(saved) = saved_window {
            window_options.window_bounds = Some(saved_window_bounds(&saved, sizes, cx));
        }
        cx.spawn(async move |cx| {
            let opened = cx.open_window(window_options, move |window, cx| {
                telaio_ui::ThemeChoice::from(theme_choice).apply(Some(window), cx);
                let view = cx.new(|cx| AppView::new(settings, screen, window, cx));
                remember_window_size(saved_window, &view, window, cx);
                // Always listening: Settings can switch to "System" at any time.
                let weak = view.downgrade();
                window::follow_system_appearance(window, move |cx| {
                    weak.upgrade()
                        .is_some_and(|view| view.read(cx).follows_system())
                });
                // The main window closing ends Latexo, the floating pill with it.
                let closing = view.downgrade();
                let main_id = window.window_handle().window_id();
                cx.on_window_closed(move |cx, closed| {
                    if closed != main_id {
                        return;
                    }
                    if let Some(view) = closing.upgrade() {
                        view.read(cx).on_close();
                        view.update(cx, |view, cx| view.close_pill(cx));
                    }
                    cx.quit();
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

/// Where a window with a saved size opens: centered, as the first time.
fn saved_window_bounds(saved: &WindowState, sizes: window::WindowSizes, cx: &App) -> WindowBounds {
    let visible = cx.primary_display().map(|display| display.visible_bounds());
    let (width, height) = saved.fitted(
        visible.map(|area| (f32::from(area.size.width), f32::from(area.size.height))),
        sizes.min,
    );
    let size = size(px(width), px(height));
    let bounds = match visible {
        Some(area) => Bounds::centered_at(area.center(), size),
        None => Bounds::new(point(px(0.0), px(0.0)), size),
    };
    if saved.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

/// Keeps the window's last size in memory while it changes and saves it when the window closes,
/// however it closes, as ted. Nothing runs while the window is not being resized.
fn remember_window_size(
    saved: Option<WindowState>,
    view: &Entity<AppView>,
    window: &mut Window,
    cx: &mut App,
) {
    // On X11 gpui asks to maximize before the window is mapped, and the request is lost.
    if saved.is_some_and(|saved| saved.maximized) && cx.compositor_name() == "X11" {
        window.on_next_frame(|window, _| {
            if !window.is_maximized() {
                window.zoom_window();
            }
        });
    }
    let wayland = cx.compositor_name() == "Wayland";
    let opened = reopening_size(wayland, window);
    let last = Rc::new(Cell::new(saved.unwrap_or(WindowState {
        width: opened.width.into(),
        height: opened.height.into(),
        maximized: false,
    })));
    let resized = last.clone();
    view.update(cx, |_, cx| {
        cx.observe_window_bounds(window, move |_, window, _| {
            // A minimized window has no usable size: keep the state it had before.
            let size = reopening_size(wayland, window);
            if !state::window::usable_size(size.width.into(), size.height.into()) {
                return;
            }
            let mut state = resized.get();
            state.maximized = window.is_maximized();
            if !state.maximized && !window.is_fullscreen() {
                state.width = size.width.into();
                state.height = size.height.into();
            }
            resized.set(state);
        })
        .detach();
    });
    cx.on_window_closed(move |_, _| state::window::save(&last.get()))
        .detach();
}

/// The size that opens the window as it is now, so it neither grows nor shrinks on every run:
/// gpui adds the client decorations' shadow around the size on X11 and takes it out on Wayland.
fn reopening_size(wayland: bool, window: &Window) -> Size<Pixels> {
    if wayland {
        window.bounds().size
    } else {
        window.inner_window_bounds().get_bounds().size
    }
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
