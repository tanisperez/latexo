//! The Settings screen, inside the window instead of a modal: two columns of cards, every change
//! applied and saved at once.

use gpui_kit::assets::IconName;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use telaio_app::language::LanguageChoice;
use telaio_ui::components::{
    NumberStepper, SecondaryButton, SegmentedControl, SegmentedItem, switch_halo,
};

use crate::state::format;
use crate::state::monitor::history_csv;
use crate::state::settings::{
    ChartStyle, HistoryKeep, Interval, LogLevel, PRICE_RANGE, PRICE_STEP, Settings,
    TemperatureChoice, ThemeChoice,
};
use crate::theme::Palette;
use crate::theme::metrics::{GAP, SETTINGS_ROW_HEIGHT, TEXT_ROW};

use super::root::{AppView, chart_style_label};
use super::widgets::{badge, card, card_title};

/// One settings row: its label, an optional description and its control.
fn row(
    label: impl Into<SharedString>,
    description: Option<String>,
    control: impl IntoElement,
    first: bool,
    palette: &Palette,
) -> AnyElement {
    h_flex()
        .min_h(px(SETTINGS_ROW_HEIGHT))
        .py(px(6.0))
        .items_center()
        .gap(px(12.0))
        .when(!first, |row| row.border_t_1().border_color(palette.border))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_size(px(TEXT_ROW)).child(label.into()))
                .when_some(description, |column, text| {
                    column.child(
                        div()
                            .text_size(px(11.5))
                            .text_color(palette.fg_muted)
                            .child(text),
                    )
                }),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A card of rows.
fn section(title: String, rows: Vec<AnyElement>, palette: &Palette) -> AnyElement {
    card(palette)
        .child(card_title(title))
        .children(rows)
        .into_any_element()
}

/// A segmented control over `choices`, which calls `set` with the chosen one.
fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    choices: Vec<(T, String)>,
    current: T,
    weak: WeakEntity<AppView>,
    set: fn(&mut Settings, T),
) -> SegmentedControl {
    let mut control = SegmentedControl::new(id);
    for (choice, label) in choices {
        let weak = weak.clone();
        control = control.item(
            SegmentedItem::new(label)
                .selected(choice == current)
                .on_click(move |window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.change(window, cx, |settings| set(settings, choice))
                    });
                }),
        );
    }
    control
}

/// The state of one sensor family as a badge: available, missing or needing more rights.
fn sensor_badge(state: Option<bool>, needs_admin: bool, palette: &Palette) -> AnyElement {
    match state {
        Some(true) => badge(
            t!("settings.available"),
            palette.success,
            palette.success_subtle,
        ),
        Some(false) if needs_admin => badge(
            t!("settings.needs_admin"),
            palette.warning,
            palette.warning_subtle,
        ),
        _ => badge(
            t!("settings.not_available"),
            palette.fg_muted,
            palette.hover,
        ),
    }
    .into_any_element()
}

impl AppView {
    pub(crate) fn render_settings(
        &self,
        palette: &Palette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let settings = &self.settings;

        let sampling = section(
            t!("settings.sampling").into_owned(),
            vec![
                row(
                    t!("settings.interval"),
                    Some(t!("settings.interval_description").into_owned()),
                    segmented(
                        "settings-interval",
                        Interval::ALL
                            .into_iter()
                            .map(|interval| (interval, format::seconds(interval.secs())))
                            .collect(),
                        settings.interval,
                        weak.clone(),
                        |settings, value| settings.interval = value,
                    ),
                    true,
                    palette,
                ),
                row(
                    t!("settings.history_keep"),
                    Some(t!("settings.history_keep_description").into_owned()),
                    segmented(
                        "settings-history-keep",
                        vec![
                            (HistoryKeep::OneHour, t!("charts.range_1h").into_owned()),
                            (HistoryKeep::Day, t!("charts.range_24h").into_owned()),
                            (HistoryKeep::Week, t!("settings.week").into_owned()),
                        ],
                        settings.history_keep,
                        weak.clone(),
                        |settings, value| settings.history_keep = value,
                    ),
                    false,
                    palette,
                ),
            ],
            palette,
        );

        let languages = LanguageChoice::ALL
            .into_iter()
            .map(|choice| {
                let label = match choice.language() {
                    Some(language) => language.native_name().to_string(),
                    None => t!("settings.language_auto").into_owned(),
                };
                (choice, label)
            })
            .collect();
        let appearance = section(
            t!("settings.appearance").into_owned(),
            vec![
                row(
                    t!("settings.theme"),
                    None,
                    segmented(
                        "settings-theme",
                        vec![
                            (
                                ThemeChoice::System,
                                t!("settings.theme_system").into_owned(),
                            ),
                            (ThemeChoice::Light, t!("settings.theme_light").into_owned()),
                            (ThemeChoice::Dark, t!("settings.theme_dark").into_owned()),
                        ],
                        settings.theme,
                        weak.clone(),
                        |settings, value| settings.theme = value,
                    ),
                    true,
                    palette,
                ),
                row(
                    t!("settings.language"),
                    None,
                    segmented(
                        "settings-language",
                        languages,
                        settings.language,
                        weak.clone(),
                        |settings, value| settings.language = value,
                    ),
                    false,
                    palette,
                ),
                row(
                    t!("settings.chart_style"),
                    None,
                    segmented(
                        "settings-chart-style",
                        ChartStyle::ALL
                            .into_iter()
                            .map(|style| (style, chart_style_label(style)))
                            .collect(),
                        settings.chart_style,
                        weak.clone(),
                        |settings, value| settings.chart_style = value,
                    ),
                    false,
                    palette,
                ),
                row(
                    t!("settings.temperature"),
                    None,
                    segmented(
                        "settings-temperature",
                        vec![
                            (TemperatureChoice::Celsius, "°C".to_string()),
                            (TemperatureChoice::Fahrenheit, "°F".to_string()),
                        ],
                        settings.temperature_unit,
                        weak.clone(),
                        |settings, value| settings.temperature_unit = value,
                    ),
                    false,
                    palette,
                ),
            ],
            palette,
        );

        let price = NumberStepper::new("settings-price", settings.energy_price)
            .step(PRICE_STEP)
            .range(*PRICE_RANGE.start(), *PRICE_RANGE.end())
            .format({
                let currency = settings.currency.clone();
                move |value| format!("{} {currency}", format::decimal(f64::from(value), 2))
            })
            .on_change({
                let weak = weak.clone();
                move |value, window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.change(window, cx, |settings| {
                            settings.energy_price = (value * 100.0).round() / 100.0
                        })
                    });
                }
            });
        let energy = section(
            t!("settings.energy").into_owned(),
            vec![row(
                t!("settings.price"),
                Some(t!("settings.price_description").into_owned()),
                price,
                true,
                palette,
            )],
            palette,
        );

        let login = switch_halo(
            "settings-login-halo",
            Switch::new("settings-login")
                .checked(settings.start_at_login)
                .on_click({
                    let weak = weak.clone();
                    move |checked, window, cx| {
                        let on = *checked;
                        let _ = weak.update(cx, |view, cx| view.set_start_at_login(on, window, cx));
                    }
                }),
            palette,
        );
        let pill = switch_halo(
            "settings-pill-halo",
            Switch::new("settings-pill")
                .checked(settings.floating_pill)
                .on_click({
                    let weak = weak.clone();
                    move |checked, window, cx| {
                        let on = *checked;
                        let _ = weak.update(cx, |view, cx| view.set_floating_pill(on, window, cx));
                    }
                }),
            palette,
        );
        let tray = switch_halo(
            "settings-tray-halo",
            Switch::new("settings-tray")
                .checked(settings.tray_icon)
                .on_click({
                    let weak = weak.clone();
                    move |checked, window, cx| {
                        let on = *checked;
                        let _ = weak.update(cx, |view, cx| {
                            view.change(window, cx, |settings| settings.tray_icon = on)
                        });
                    }
                }),
            palette,
        );
        let startup = section(
            t!("settings.startup").into_owned(),
            vec![
                row(
                    if cfg!(windows) {
                        t!("settings.start_with_windows")
                    } else {
                        t!("settings.start_with_session")
                    },
                    None,
                    login,
                    true,
                    palette,
                ),
                row(
                    t!("settings.tray_icon"),
                    Some(t!("settings.tray_icon_description").into_owned()),
                    tray,
                    false,
                    palette,
                ),
                row(
                    t!("settings.floating_pill"),
                    Some(t!("settings.floating_pill_description").into_owned()),
                    pill,
                    false,
                    palette,
                ),
            ],
            palette,
        );

        let capabilities = self.monitor.capabilities;
        let sensors = section(
            t!("settings.sensors").into_owned(),
            vec![
                row(
                    t!("settings.sensor_temperatures"),
                    None,
                    sensor_badge(capabilities.map(|c| c.temperatures), cfg!(windows), palette),
                    true,
                    palette,
                ),
                row(
                    t!("settings.sensor_fans"),
                    None,
                    sensor_badge(capabilities.map(|c| c.fans), false, palette),
                    false,
                    palette,
                ),
                row(
                    t!("settings.sensor_cpu_power"),
                    (capabilities.and_then(|c| c.package_power) != Some(true))
                        .then(|| self.power_note()),
                    sensor_badge(capabilities.and_then(|c| c.package_power), true, palette),
                    false,
                    palette,
                ),
                row(
                    t!("settings.sensor_gpu"),
                    None,
                    sensor_badge(capabilities.map(|c| c.gpu), false, palette),
                    false,
                    palette,
                ),
                row(
                    t!("settings.sensor_battery"),
                    None,
                    sensor_badge(capabilities.map(|c| c.battery), false, palette),
                    false,
                    palette,
                ),
            ],
            palette,
        );

        let export = SecondaryButton::new("settings-export", t!("settings.export"))
            .icon(Icon::new(IconName::Download))
            .on_click({
                let weak = weak.clone();
                move |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| view.export_history(window, cx));
                }
            });
        let clear = SecondaryButton::new("settings-clear", t!("settings.clear")).on_click({
            let weak = weak.clone();
            move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| {
                    view.monitor.clear_history();
                    window.push_notification(
                        notification::Notification::success(t!("settings.cleared").into_owned()),
                        cx,
                    );
                    cx.notify();
                });
            }
        });
        let data = section(
            t!("settings.data").into_owned(),
            vec![
                row(
                    t!("settings.export_label"),
                    Some(t!("settings.export_description").into_owned()),
                    export,
                    true,
                    palette,
                ),
                row(
                    t!("settings.clear_label"),
                    Some(t!("settings.clear_description").into_owned()),
                    clear,
                    false,
                    palette,
                ),
            ],
            palette,
        );

        let log_level = segmented(
            "settings-log-level",
            [
                LogLevel::Off,
                LogLevel::Error,
                LogLevel::Warn,
                LogLevel::Info,
                LogLevel::Debug,
            ]
            .into_iter()
            .map(|level| (level, level.label()))
            .collect(),
            settings.log_level,
            weak.clone(),
            |settings, value| settings.log_level = value,
        );
        let mut advanced_rows = vec![row(
            t!("settings.log_level"),
            telaio_app::logging::log_path(&latexo_cli::IDENTITY)
                .map(|path| path.display().to_string()),
            log_level,
            true,
            palette,
        )];
        if crate::state::update::updater_enabled() {
            advanced_rows.push(self.update_rows(palette, cx));
        }
        let advanced = section(t!("settings.advanced").into_owned(), advanced_rows, palette);

        h_flex()
            .items_start()
            .gap(px(GAP))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(GAP))
                    .child(sampling)
                    .child(appearance)
                    .child(energy),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(GAP))
                    .child(startup)
                    .child(sensors)
                    .child(data)
                    .child(advanced),
            )
            .into_any_element()
    }

    fn set_start_at_login(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        match crate::state::autostart::set_enabled(on) {
            Ok(()) => self.change(window, cx, |settings| settings.start_at_login = on),
            Err(error) => {
                window.push_notification(
                    notification::Notification::error(
                        t!("settings.startup_failed", error = error).into_owned(),
                    ),
                    cx,
                );
                cx.notify();
            }
        }
    }

    fn set_floating_pill(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.change(window, cx, |settings| settings.floating_pill = on);
        self.set_pill(on, window, cx);
    }

    /// Asks where to save the history as CSV and writes it.
    fn export_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let csv = history_csv(&self.monitor.history);
        let directory = std::env::home_dir().unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&directory, Some("latexo-history.csv"));
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let written = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move { std::fs::write(&path, csv) }
                })
                .await;
            let _ = view.update_in(cx, |_, window, cx| {
                let notification = match written {
                    Ok(()) => {
                        log::info!("history exported to {}", path.display());
                        notification::Notification::success(
                            t!("settings.exported", path = path.display().to_string()).into_owned(),
                        )
                    }
                    Err(error) => {
                        log::warn!("could not export the history: {error}");
                        notification::Notification::error(
                            t!("settings.export_failed", error = error.to_string()).into_owned(),
                        )
                    }
                };
                window.push_notification(notification, cx);
            });
        })
        .detach();
    }
}
