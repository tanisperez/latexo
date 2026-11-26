//! The window: the title bar, the side bar with the live value of every screen, the header of
//! the current screen with its selectors, and the screen itself. It owns the settings and the
//! [`Monitor`], and runs the sampling task.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::SystemInfo;
use latexo_core::process::ProcessQuery;
use latexo_core::{Range, Sample};
use latexo_sensors::{Capabilities, Sampler};
use rust_i18n::t;
use telaio_ui::components::{ScrollbarOverlay, SegmentedControl, SegmentedItem};

use crate::state::format;
use crate::state::monitor::{AlertEvent, Monitor};
use crate::state::nav::Screen;
use crate::state::settings::{ChartStyle, Settings, ThemeChoice, save_settings};
use crate::theme::metrics::{
    CONTENT_PADDING_X, CONTENT_PADDING_Y, GAP, HEADER_TITLE_MIN_WIDTH, NAV_ROW_HEIGHT, RADIUS_MD,
    SIDEBAR_LOGO_SIZE, SIDEBAR_PADDING, SIDEBAR_WIDTH, TEXT_GROUP, TEXT_SM, TITLE_BAR_HEIGHT,
    TITLE_BUTTON_SIZE, TITLE_TEXT_SIZE,
};
use crate::theme::{Palette, PaletteExt};

use super::logo::logo;
use super::widgets::{badge, palette_mono};

/// The window's root view.
pub struct AppView {
    /// The settings, applied live.
    pub settings: Settings,
    /// What the screens show.
    pub monitor: Monitor,
    /// The screen shown.
    pub screen: Screen,
    /// The sorting, filter and search of the process table.
    pub query: ProcessQuery,
    /// The selected process.
    pub selected_pid: Option<u32>,
    /// The search box of the process table.
    pub search: Entity<InputState>,
    /// The scroll of the process table.
    pub process_scroll: UniformListScrollHandle,
    /// Where the in-app updater stands.
    pub update: crate::state::update::UpdateState,
    /// How this copy was installed, for the updater.
    pub update_env: crate::state::update::UpdateEnv,
    /// The floating pill's window, while it is open.
    pub pill: Option<AnyWindowHandle>,
    /// The tray flyout's window, while it is open.
    pub flyout: Option<AnyWindowHandle>,
    /// The tray icon, while it is shown.
    pub tray: Option<super::tray::TrayIcon>,
    content_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    interval_ms: Arc<AtomicU64>,
    wants_processes: Arc<AtomicBool>,
    _sampling: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    /// The view, which starts sampling at once.
    pub fn new(
        settings: Settings,
        screen: Option<Screen>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder(t!("processes.search_placeholder")));
        let subscription = cx.subscribe_in(&search, window, |view, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                view.query.search = input.read(cx).value().to_string();
                cx.notify();
            }
        });
        let settings_interval = settings.interval;
        let keep_minutes = settings.history_keep.minutes();
        let mut view = Self {
            settings,
            monitor: Monitor::new(keep_minutes),
            screen: screen.unwrap_or_default(),
            query: ProcessQuery::default(),
            selected_pid: None,
            search,
            process_scroll: UniformListScrollHandle::new(),
            update: crate::state::update::UpdateState::default(),
            update_env: crate::state::update::UpdateEnv::detect(),
            pill: None,
            tray: None,
            flyout: None,
            content_scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            interval_ms: Arc::new(AtomicU64::new(interval_ms(&settings_interval))),
            wants_processes: Arc::new(AtomicBool::new(true)),
            _sampling: Task::ready(()),
            _subscriptions: vec![subscription],
        };
        view.wants_processes
            .store(view.screen.shows_processes(), Ordering::Relaxed);
        view.focus_handle.focus(window, cx);
        view._sampling = view.start_sampling(window, cx);
        view.schedule_startup_update_check(cx);
        if view.settings.floating_pill {
            view.set_pill(true, window, cx);
        }
        if view.settings.tray_icon {
            view.set_tray(true, window, cx);
        }
        if crate::state::nav::opens_flyout() {
            view.toggle_flyout(window, cx);
        }
        view
    }

    /// Starts the sampling thread (sysinfo's GPU handles cannot move between threads, so the
    /// sampler lives on one thread of its own) and receives its samples until the window closes.
    /// Nothing else wakes the window: an idle Latexo redraws once per sample.
    fn start_sampling(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        let (sender, receiver) = async_channel::bounded::<Message>(4);
        let interval = self.interval_ms.clone();
        let wants_processes = self.wants_processes.clone();
        let spawned = std::thread::Builder::new()
            .name("latexo-sampler".to_string())
            .spawn(move || sample_loop(&sender, &interval, &wants_processes));
        if let Err(error) = spawned {
            log::error!("could not start the sampling thread: {error}");
        }
        cx.spawn_in(window, async move |view, cx| {
            while let Ok(message) = receiver.recv().await {
                let updated = view.update_in(cx, |view, window, cx| match message {
                    Message::Info(info) => {
                        log::info!(
                            "machine: {} ({} logical processors), {}",
                            info.cpu_brand,
                            info.logical_cores,
                            info.os_name
                        );
                        view.monitor.info = Some(*info);
                        cx.notify();
                    }
                    Message::Sample(sample, capabilities) => {
                        if let Some(capabilities) = capabilities
                            && view.monitor.capabilities != Some(capabilities)
                        {
                            log::info!("sensors: {capabilities:?}");
                            view.monitor.capabilities = Some(capabilities);
                        }
                        view.push_sample(*sample, window, cx);
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
    }

    fn push_sample(&mut self, sample: Sample, window: &mut Window, cx: &mut Context<Self>) {
        let fired = self.monitor.push(sample, &self.settings.alerts);
        for event in fired {
            log::info!(
                "alert: {:?} at {:.1} (threshold {:.1})",
                event.rule.metric,
                event.value,
                event.rule.threshold
            );
            if event.rule.action == latexo_core::alerts::AlertAction::Notify {
                let text = self.alert_text(&event);
                // Out of sight (minimized, behind other windows), the system shows it too.
                if !window.is_window_active() {
                    crate::state::notify::send(
                        t!("alerts.notification_title").into_owned(),
                        text.clone(),
                    );
                }
                window.push_notification(Notification::warning(text), cx);
            }
        }
        cx.notify();
    }

    /// The sentence of an alert, for the notification and the history.
    pub fn alert_text(&self, event: &AlertEvent) -> String {
        let value = self.metric_value(event.rule.metric, event.value);
        t!(
            "alerts.fired",
            rule = super::alerts::rule_name(event.rule.metric),
            value = value
        )
        .into_owned()
    }

    /// Saves the settings and applies them: theme, language, log level.
    pub fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.history_keep.allows(self.range()) {
            self.settings.range = Range::OneHour.into();
        }
        self.monitor.set_keep(self.settings.history_keep.minutes());
        save_settings(&self.settings);
        self.interval_ms
            .store(interval_ms(&self.settings.interval), Ordering::Relaxed);
        telaio_app::logging::set_level(self.settings.log_level.to_filter());
        let language = crate::state::locale::resolve(self.settings.language);
        crate::state::locale::apply(language);
        self.search.update(cx, |search, cx| {
            search.set_placeholder(t!("processes.search_placeholder"), window, cx)
        });
        telaio_ui::ThemeChoice::from(self.settings.theme).apply(Some(window), cx);
        self.set_tray(self.settings.tray_icon, window, cx);
        self.sync_tray();
        cx.notify();
    }

    /// Changes the settings with `change`, then saves and applies them.
    pub fn change(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Settings),
    ) {
        change(&mut self.settings);
        self.apply_settings(window, cx);
    }

    /// Shows `screen`.
    pub fn show(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen != screen {
            log::debug!("screen: {}", screen.code());
            self.screen = screen;
            self.wants_processes
                .store(screen.shows_processes(), Ordering::Relaxed);
            self.content_scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        }
    }

    /// The chart range of the settings.
    pub fn range(&self) -> Range {
        self.settings.range.into()
    }

    /// The time slots of the chart range: one per sample, or one per minute for the long ranges.
    pub fn slots(&self) -> usize {
        let step = if self.range().is_long() {
            latexo_core::minutes::MINUTE
        } else {
            self.settings.interval.secs()
        };
        (self.range().secs() / step).round() as usize + 1
    }

    /// Whether the theme follows the system.
    pub fn follows_system(&self) -> bool {
        self.settings.theme == ThemeChoice::System
    }

    /// Gives the keyboard back to the window, out of the search box, so `Delete` ends the
    /// selected process instead of deleting text.
    pub fn focus_window(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    /// Writes the energy account and the minute history, when the window closes.
    pub fn on_close(&self) {
        self.monitor.save_energy();
        self.monitor.save_minutes();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        log::trace!("render root, screen {}", self.screen.code());
        let palette = cx.palette();
        let body = self.render_screen(window, cx);
        let scroll = self.content_scroll.clone();
        let content = if self.screen == Screen::Processes {
            // The table scrolls on its own and fills the height.
            v_flex()
                .flex_1()
                .min_h_0()
                .px(px(CONTENT_PADDING_X))
                .pb(px(CONTENT_PADDING_Y))
                .child(body)
                .into_any_element()
        } else {
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    v_flex()
                        .id("content")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .px(px(CONTENT_PADDING_X))
                        .pb(px(CONTENT_PADDING_Y))
                        .child(body),
                )
                .child(ScrollbarOverlay::new(&scroll))
                .into_any_element()
        };
        let processes = self.screen == Screen::Processes;
        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .key_context(if processes {
                crate::actions::PROCESSES_CONTEXT
            } else {
                "Latexo"
            })
            .on_action(
                cx.listener(|view, _: &crate::actions::FocusSearch, window, cx| {
                    view.show(Screen::Processes, cx);
                    view.search
                        .update(cx, |search, cx| search.focus(window, cx));
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::actions::EndProcess, window, cx| {
                    view.confirm_end(false, window, cx);
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::actions::OpenSettings, _, cx| {
                    view.show(Screen::Settings, cx);
                }),
            )
            .bg(palette.bg)
            .text_color(palette.fg)
            .text_size(px(13.0))
            .child(self.render_title_bar(&palette, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(self.render_sidebar(&palette, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(self.render_header(&palette, window, cx))
                            .child(content),
                    ),
            )
    }
}

impl AppView {
    fn render_title_bar(&self, palette: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let (hover, pressed) = (palette.hover, palette.pressed);
        let selected = self.screen == Screen::Settings;
        let gear = div()
            .id("title-settings")
            .size(px(TITLE_BUTTON_SIZE))
            .mr(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .text_color(if selected {
                palette.accent_text
            } else {
                palette.fg_muted
            })
            .when(selected, |gear| gear.bg(palette.accent_subtle))
            .hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed))
            .tooltip(|window, cx| {
                tooltip::Tooltip::new(SharedString::from(t!("nav.settings"))).build(window, cx)
            })
            .child(Icon::new(IconName::SlidersHorizontal).with_size(px(16.0)))
            .on_click(cx.listener(|view, _, _, cx| view.show(Screen::Settings, cx)));
        TitleBar::new()
            .h(px(TITLE_BAR_HEIGHT))
            .bg(palette.surface)
            .border_color(palette.border)
            .child(
                h_flex()
                    .size_full()
                    .items_center()
                    .child(div().flex_1())
                    .child(div().occlude().child(gear)),
            )
    }

    fn render_sidebar(&self, palette: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let group = |label: String| {
            div()
                .px(px(10.0))
                .pt(px(18.0))
                .pb(px(6.0))
                .text_size(px(TEXT_GROUP))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.fg_muted)
                .child(label.to_uppercase())
        };
        let interval = format::seconds(self.settings.interval.secs());
        v_flex()
            .id("sidebar")
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .bg(palette.surface)
            .border_r_1()
            .border_color(palette.border)
            .p(px(SIDEBAR_PADDING))
            .gap(px(2.0))
            .overflow_y_scroll()
            .child(
                h_flex()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(10.0))
                    .pt(px(2.0))
                    .pb(px(14.0))
                    .child(logo(SIDEBAR_LOGO_SIZE))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Latexo"),
                    ),
            )
            .children(Screen::TOP.map(|screen| self.nav_row(screen, palette, cx)))
            .child(group(t!("nav.hardware").into_owned()))
            .children(Screen::HARDWARE.map(|screen| self.nav_row(screen, palette, cx)))
            .child(group(t!("nav.system").into_owned()))
            .children(Screen::SYSTEM.map(|screen| self.nav_row(screen, palette, cx)))
            .child(div().flex_1().min_h(px(12.0)))
            .when_some(self.update_ready_button(palette, cx), |column, button| {
                column.child(button)
            })
            .child(div().px(px(10.0)).pb(px(8.0)).child(badge(
                t!("nav.live", interval = interval),
                palette.success,
                palette.success_subtle,
            )))
            .child(self.nav_row(Screen::About, palette, cx))
    }

    /// The "Update ready, restart" entry of the side bar, when an update waits.
    fn update_ready_button(&self, palette: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ready = self.update.ready()?;
        let (hover, pressed) = (palette.hover, palette.pressed);
        let manual = ready.is_manual_download();
        let label = if manual {
            t!("update.available_short", version = ready.version.as_str())
        } else {
            t!("update.ready_short", version = ready.version.as_str())
        };
        let notes = ready.notes_url.clone();
        Some(
            h_flex()
                .id("update-ready")
                .mx(px(4.0))
                .mb(px(8.0))
                .px(px(10.0))
                .py(px(6.0))
                .gap(px(8.0))
                .items_center()
                .rounded(px(RADIUS_MD))
                .bg(palette.accent_subtle)
                .text_color(palette.accent_text)
                .text_size(px(TEXT_SM))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .active(move |style| style.bg(pressed))
                .child(Icon::new(IconName::RefreshCw).with_size(px(14.0)))
                .child(div().flex_1().min_w_0().child(label.into_owned()))
                .on_click(cx.listener(move |view, _, window, cx| {
                    if manual {
                        cx.open_url(&notes);
                    } else {
                        view.request_update_restart(window, cx);
                    }
                }))
                .into_any_element(),
        )
    }

    fn nav_row(&self, screen: Screen, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.screen == screen;
        let (hover, pressed) = (palette.hover, palette.pressed);
        h_flex()
            .id(SharedString::from(format!("nav-{}", screen.code())))
            .flex_none()
            .w_full()
            .h(px(NAV_ROW_HEIGHT))
            .px(px(10.0))
            .gap(px(10.0))
            .items_center()
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(palette.accent_subtle)
                    .text_color(palette.accent_text)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!selected, |row| {
                row.hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(pressed))
            })
            .child(
                Icon::new(screen_icon(screen))
                    .with_size(px(16.0))
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(screen_label(screen)),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(palette_mono())
                    .text_size(px(TEXT_SM))
                    .text_color(palette.fg_muted)
                    .child(self.nav_value(screen)),
            )
            .on_click(cx.listener(move |view, _, _, cx| view.show(screen, cx)))
            .into_any_element()
    }

    /// The live value next to a screen in the side bar.
    fn nav_value(&self, screen: Screen) -> String {
        let Some(sample) = self.monitor.latest.as_deref() else {
            return String::new();
        };
        let unit = self.settings.temperature_unit.into();
        match screen {
            Screen::Processes => format::integer(sample.processes.len()),
            Screen::Cpu => format::percent(sample.cpu.usage, 0),
            Screen::Cores => format::integer(sample.cpu.cores.len()),
            Screen::Gpu => sample
                .gpus
                .first()
                .and_then(|gpu| gpu.usage)
                .map(|usage| format::percent(usage, 0))
                .unwrap_or_default(),
            Screen::Memory => format::percent(sample.memory.usage(), 0),
            Screen::Disk => {
                format::rate(sample.disk_io.read_per_sec + sample.disk_io.write_per_sec)
            }
            Screen::Network => {
                format::rate(sample.network.received_per_sec + sample.network.sent_per_sec)
            }
            Screen::Temperatures => sample
                .cpu_temperature()
                .map(|celsius| format::temperature(celsius, unit))
                .unwrap_or_default(),
            Screen::Fans => sample
                .fans
                .iter()
                .map(|fan| fan.rpm)
                .max()
                .map(format::rpm)
                .unwrap_or_default(),
            Screen::Energy => sample
                .power
                .measured_total()
                .map(format::watts)
                .unwrap_or_default(),
            Screen::Alerts => {
                let active = self.settings.alerts.iter().filter(|r| r.enabled).count();
                format::integer(active)
            }
            Screen::Overview | Screen::Settings | Screen::About => String::new(),
        }
    }

    fn render_header(
        &self,
        palette: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title, subtitle) = self.titles();
        let controls: AnyElement = match self.screen {
            Screen::Processes => self.process_controls(palette, window, cx),
            Screen::Cores => h_flex()
                .gap(px(10.0))
                .flex_wrap()
                .child(self.cores_view_control(cx))
                .child(self.range_control(cx))
                .into_any_element(),
            screen if screen.has_charts() => h_flex()
                .gap(px(10.0))
                .flex_wrap()
                .child(self.chart_style_control(cx))
                .child(self.range_control(cx))
                .child(
                    telaio_ui::components::SecondaryButton::new(
                        "header-export",
                        t!("settings.export"),
                    )
                    .icon(Icon::new(IconName::Download))
                    .on_click(cx.listener(|view, _, window, cx| view.export_history(window, cx))),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        };
        // The controls go under the title when both do not fit side by side.
        h_flex()
            .flex_none()
            .flex_wrap()
            .items_start()
            .justify_between()
            .gap(px(GAP))
            .px(px(CONTENT_PADDING_X))
            .pt(px(CONTENT_PADDING_Y))
            .pb(px(GAP))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(HEADER_TITLE_MIN_WIDTH))
                    .child(
                        div()
                            .text_size(px(TITLE_TEXT_SIZE))
                            .line_height(px(26.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .mt(px(2.0))
                            .text_size(px(12.5))
                            .text_color(palette.fg_muted)
                            .truncate()
                            .child(subtitle),
                    ),
            )
            .child(div().flex_none().max_w_full().child(controls))
    }

    /// The chart style selector of the header.
    pub fn chart_style_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut control = SegmentedControl::new("chart-style");
        for style in ChartStyle::ALL {
            let weak = cx.weak_entity();
            control = control.item(
                SegmentedItem::new(chart_style_label(style))
                    .selected(self.settings.chart_style == style)
                    .on_click(move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.change(window, cx, |settings| settings.chart_style = style)
                        });
                    }),
            );
        }
        control
    }

    /// The range selector of the header.
    pub fn range_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut control = SegmentedControl::new("chart-range");
        let keep = self.settings.history_keep;
        for range in Range::ALL.into_iter().filter(|range| keep.allows(*range)) {
            let weak = cx.weak_entity();
            control = control.item(
                SegmentedItem::new(range_label(range))
                    .selected(self.range() == range)
                    .on_click(move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.change(window, cx, |settings| settings.range = range.into())
                        });
                    }),
            );
        }
        control
    }

    fn titles(&self) -> (String, String) {
        let title = screen_label(self.screen);
        let info = self.monitor.info.as_ref();
        let sample = self.monitor.latest.as_deref();
        let subtitle = match (self.screen, info, sample) {
            (_, None, _) | (_, _, None) if self.screen != Screen::About => {
                t!("common.reading").into_owned()
            }
            (Screen::Overview, Some(info), Some(sample)) => t!(
                "overview.subtitle",
                os = info.os_name.clone(),
                uptime = format::uptime(sample.uptime_secs)
            )
            .into_owned(),
            (Screen::Processes, _, Some(sample)) => match sample.thread_count() {
                0 => crate::state::locale::plural("processes.count", sample.processes.len()),
                threads => t!(
                    "processes.subtitle",
                    processes = format::integer(sample.processes.len()),
                    threads = format::integer(threads)
                )
                .into_owned(),
            },
            (Screen::Cpu, Some(info), _) => {
                let cores = info.physical_cores.unwrap_or(info.logical_cores);
                t!(
                    "cpu.subtitle",
                    model = info.cpu_brand.clone(),
                    cores = cores,
                    threads = info.logical_cores
                )
                .into_owned()
            }
            (Screen::Cores, Some(info), _) => {
                t!("cores.subtitle", count = info.logical_cores).into_owned()
            }
            (Screen::Gpu, _, Some(sample)) => sample
                .gpus
                .first()
                .map(|gpu| gpu.name.clone())
                .unwrap_or_else(|| t!("gpu.none").into_owned()),
            (Screen::Memory, _, Some(sample)) => t!(
                "memory.subtitle",
                total = format::bytes(sample.memory.total),
                swap = format::bytes(sample.memory.swap_total)
            )
            .into_owned(),
            (Screen::Disk, _, Some(sample)) => {
                crate::state::locale::plural("disk.volumes", sample.volumes.len())
            }
            (Screen::Network, _, Some(sample)) => {
                let active: Vec<&str> = sample
                    .network
                    .interfaces
                    .iter()
                    .filter(|interface| interface.address.is_some() && interface.is_active())
                    .map(|interface| interface.name.as_str())
                    .collect();
                if active.is_empty() {
                    t!("network.disconnected").into_owned()
                } else {
                    active.join(" · ")
                }
            }
            (Screen::Temperatures, _, Some(sample)) => t!(
                "temperatures.subtitle",
                sensors =
                    crate::state::locale::plural("temperatures.sensors", sample.sensors.len()),
                fans = crate::state::locale::plural("temperatures.fans", sample.fans.len())
            )
            .into_owned(),
            (Screen::Fans, _, Some(sample)) => {
                crate::state::locale::plural("temperatures.fans", sample.fans.len())
            }
            (Screen::Energy, _, _) => t!(
                "energy.subtitle",
                price = format::money(
                    f64::from(self.settings.energy_price),
                    &self.settings.currency
                )
            )
            .into_owned(),
            (Screen::Alerts, _, _) => {
                let active = self.settings.alerts.iter().filter(|r| r.enabled).count();
                t!(
                    "alerts.subtitle",
                    rules =
                        crate::state::locale::plural("alerts.rules", self.settings.alerts.len()),
                    active = active
                )
                .into_owned()
            }
            (Screen::Settings, Some(info), _) => {
                format!("Latexo {} · {}", latexo_cli::version_label(), info.os_name)
            }
            _ => String::new(),
        };
        (title, subtitle)
    }

    fn render_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = cx.palette();
        match self.screen {
            Screen::Settings => return self.render_settings(&palette, window, cx),
            Screen::About => return self.render_about(&palette, cx),
            Screen::Alerts => return self.render_alerts(&palette, cx),
            _ => {}
        }
        let Some(sample) = self.monitor.latest.clone() else {
            return super::widgets::empty_line(t!("common.reading"), &palette).into_any_element();
        };
        match self.screen {
            Screen::Overview => self.render_overview(&sample, &palette, cx),
            Screen::Processes => self.render_processes(&sample, &palette, window, cx),
            Screen::Cpu => self.render_cpu(&sample, &palette),
            Screen::Cores => self.render_cores(&sample, &palette),
            Screen::Gpu => self.render_gpu(&sample, &palette),
            Screen::Memory => self.render_memory(&sample, &palette),
            Screen::Disk => self.render_disk(&sample, &palette),
            Screen::Network => self.render_network(&sample, &palette),
            Screen::Temperatures => self.render_temperatures(&sample, &palette),
            Screen::Fans => self.render_fans(&sample, &palette),
            Screen::Energy => self.render_energy(&sample, &palette),
            Screen::Alerts | Screen::Settings | Screen::About => div().into_any_element(),
        }
    }
}

/// The icon of a screen in the side bar.
pub fn screen_icon(screen: Screen) -> IconName {
    match screen {
        Screen::Overview => IconName::LayoutGrid,
        Screen::Processes => IconName::List,
        Screen::Cpu => IconName::Cpu,
        Screen::Cores => IconName::Grid3x3,
        Screen::Gpu => IconName::Box,
        Screen::Memory => IconName::MemoryStick,
        Screen::Disk => IconName::HardDrive,
        Screen::Network => IconName::ArrowUpDown,
        Screen::Temperatures => IconName::Thermometer,
        Screen::Fans => IconName::Fan,
        Screen::Energy => IconName::Zap,
        Screen::Alerts => IconName::Bell,
        Screen::Settings => IconName::SlidersHorizontal,
        Screen::About => IconName::Info,
    }
}

/// The name of a screen.
pub fn screen_label(screen: Screen) -> String {
    match screen {
        Screen::Overview => t!("nav.overview"),
        Screen::Processes => t!("nav.processes"),
        Screen::Cpu => t!("nav.cpu"),
        Screen::Cores => t!("nav.cores"),
        Screen::Gpu => t!("nav.gpu"),
        Screen::Memory => t!("nav.memory"),
        Screen::Disk => t!("nav.disk"),
        Screen::Network => t!("nav.network"),
        Screen::Temperatures => t!("nav.temperatures"),
        Screen::Fans => t!("nav.fans"),
        Screen::Energy => t!("nav.energy"),
        Screen::Alerts => t!("nav.alerts"),
        Screen::Settings => t!("nav.settings"),
        Screen::About => t!("nav.about"),
    }
    .into_owned()
}

/// The name of a chart style.
pub fn chart_style_label(style: ChartStyle) -> String {
    match style {
        ChartStyle::Line => t!("charts.line"),
        ChartStyle::Bars => t!("charts.bars"),
        ChartStyle::Steps => t!("charts.steps"),
        ChartStyle::Dots => t!("charts.dots"),
    }
    .into_owned()
}

/// The name of a range: `1 min`, `1 h`.
pub fn range_label(range: Range) -> String {
    match range {
        Range::OneMinute => t!("charts.range_1m"),
        Range::FiveMinutes => t!("charts.range_5m"),
        Range::FifteenMinutes => t!("charts.range_15m"),
        Range::OneHour => t!("charts.range_1h"),
        Range::SixHours => t!("charts.range_6h"),
        Range::Day => t!("charts.range_24h"),
        Range::Week => t!("charts.range_7d"),
    }
    .into_owned()
}

/// How often the sampling thread checks again what the machine can read, in samples.
const CAPABILITIES_EVERY: u64 = 30;

/// What the sampling thread sends.
enum Message {
    /// The machine, once, when the sampler opens.
    Info(Box<SystemInfo>),
    /// A sample, with what the machine can read on the first one.
    Sample(Box<Sample>, Option<Capabilities>),
}

fn interval_ms(interval: &crate::state::settings::Interval) -> u64 {
    (interval.secs() * 1000.0).round() as u64
}

/// The sampling thread: opens the sampler, then reads the machine once per interval and sends
/// the sample, until the window drops the receiver. The processes, the most expensive part, are
/// read on every sample only while the screen shown needs them.
fn sample_loop(
    sender: &async_channel::Sender<Message>,
    interval: &AtomicU64,
    wants_processes: &AtomicBool,
) {
    let mut sampler = match Sampler::new() {
        Ok(sampler) => sampler,
        Err(error) => {
            log::error!("could not open the system information: {error}");
            return;
        }
    };
    if sender
        .send_blocking(Message::Info(Box::new(sampler.system_info())))
        .is_err()
    {
        return;
    }
    let mut count: u64 = 0;
    loop {
        let started = Instant::now();
        let sample = sampler.sample_with(wants_processes.load(Ordering::Relaxed));
        // Again now and then: some readings only show up later (the GPU usage of its processes).
        let capabilities = count
            .is_multiple_of(CAPABILITIES_EVERY)
            .then(|| sampler.capabilities(&sample));
        count += 1;
        if sender
            .send_blocking(Message::Sample(Box::new(sample), capabilities))
            .is_err()
        {
            log::debug!("sampling stopped");
            return;
        }
        let wait = Duration::from_millis(interval.load(Ordering::Relaxed));
        std::thread::sleep(wait.saturating_sub(started.elapsed()));
    }
}
