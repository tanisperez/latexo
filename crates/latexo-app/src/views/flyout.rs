//! The tray flyout of the design: a small window over the tray with the newest alert and a row
//! per resource (its value and a small chart), and the Open Latexo and pill buttons. A click on
//! the tray icon opens and closes it; it also closes when it loses the focus.

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::SeriesKey;
use rust_i18n::t;
use telaio_ui::components::SecondaryButton;

use crate::state::format;
use crate::theme::PaletteExt;
use crate::theme::metrics::{TEXT_ROW, TEXT_SM};

use super::chart::Chart;
use super::logo::logo;
use super::root::{AppView, screen_icon};
use super::widgets::{badge, icon_tile, palette_mono};

/// The flyout's size.
const WIDTH: f32 = 360.0;
const HEIGHT: f32 = 520.0;
/// Its distance from the screen corner.
const MARGIN: f32 = 12.0;
/// The size of a row's chart.
const CHART_WIDTH: f32 = 96.0;
const CHART_HEIGHT: f32 = 24.0;
/// An alert older than this is not shown at the top.
const ALERT_MINUTES: i64 = 15;

/// One row: its screen (for the icon), name, value and chart.
struct Row {
    screen: crate::state::nav::Screen,
    name: String,
    value: String,
    chart: Chart,
}

/// The flyout's view: it reads the main view's monitor and redraws when it changes.
pub struct FlyoutView {
    app: Entity<AppView>,
    main: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl FlyoutView {
    fn new(
        app: Entity<AppView>,
        main: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&app, |_, _, cx| cx.notify());
        // Closes when the user clicks elsewhere.
        let app_for_close = app.clone();
        let activation = cx.observe_window_activation(window, move |_, window, cx| {
            if !window.is_window_active() {
                app_for_close.update(cx, |view, _| view.flyout = None);
                window.remove_window();
            }
        });
        Self {
            app,
            main,
            _subscriptions: vec![observe, activation],
        }
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        use crate::state::nav::Screen;
        let view = self.app.read(cx);
        let Some(sample) = view.monitor.latest.as_deref() else {
            return Vec::new();
        };
        let palette = cx.palette();
        let unit = view.settings.temperature_unit.into();
        let chart = |key: &SeriesKey, max: f32| view.chart(key, max, CHART_HEIGHT, &palette);
        let mut rows = vec![Row {
            screen: Screen::Cpu,
            name: t!("nav.cpu").into_owned(),
            value: format::percent(sample.cpu.usage, 0),
            chart: chart(&SeriesKey::CpuUsage, 1.0),
        }];
        if let Some(usage) = sample.gpus.first().and_then(|gpu| gpu.usage) {
            rows.push(Row {
                screen: Screen::Gpu,
                name: t!("nav.gpu").into_owned(),
                value: format::percent(usage, 0),
                chart: chart(&SeriesKey::GpuUsage(0), 1.0),
            });
        }
        rows.push(Row {
            screen: Screen::Memory,
            name: t!("nav.memory").into_owned(),
            value: t!(
                "common.of",
                used = format::bytes(sample.memory.used),
                total = format::bytes(sample.memory.total)
            )
            .into_owned(),
            chart: chart(&SeriesKey::MemoryUsed, sample.memory.total as f32),
        });
        rows.push(Row {
            screen: Screen::Disk,
            name: t!("nav.disk").into_owned(),
            value: format::rate(sample.disk_io.read_per_sec + sample.disk_io.write_per_sec),
            chart: chart(
                &SeriesKey::DiskWrite,
                view.stacked_max(&SeriesKey::DiskRead, &SeriesKey::DiskWrite, 1_048_576.0),
            ),
        });
        rows.push(Row {
            screen: Screen::Network,
            name: t!("nav.network").into_owned(),
            value: t!(
                "overview.net_split",
                down = format::rate(sample.network.received_per_sec),
                up = format::rate(sample.network.sent_per_sec)
            )
            .into_owned(),
            chart: chart(
                &SeriesKey::NetReceived,
                view.stacked_max(&SeriesKey::NetReceived, &SeriesKey::NetSent, 102_400.0),
            ),
        });
        if let Some(celsius) = sample.cpu_temperature() {
            rows.push(Row {
                screen: Screen::Temperatures,
                name: t!("pill.temperature").into_owned(),
                value: format::temperature(celsius, unit),
                chart: chart(&SeriesKey::CpuTemperature, 110.0).threshold(80.0, palette.warning),
            });
        }
        if let Some(watts) = sample.power.measured_total() {
            rows.push(Row {
                screen: Screen::Energy,
                name: t!("pill.power").into_owned(),
                value: format::watts(watts),
                chart: chart(
                    &SeriesKey::PowerTotal,
                    view.auto_max(&SeriesKey::PowerTotal, 10.0),
                ),
            });
        }
        rows
    }

    /// The newest alert of the last minutes, with how long ago it fired.
    fn alert(&self, cx: &App) -> Option<String> {
        let view = self.app.read(cx);
        let event = view.monitor.alert_log.first()?;
        let minutes = (chrono::Local::now() - event.at).num_minutes();
        (minutes <= ALERT_MINUTES).then(|| {
            t!(
                "tray.alert_ago",
                alert = view.alert_text(event),
                minutes = minutes.max(0)
            )
            .into_owned()
        })
    }
}

impl Render for FlyoutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.palette();
        let rows = self.rows(cx);
        let alert = self.alert(cx);
        let interval = format::seconds(self.app.read(cx).settings.interval.secs());
        let main = self.main;
        let app = self.app.clone();
        v_flex()
            .size_full()
            .bg(palette.overlay)
            .border_1()
            .border_color(palette.border_strong)
            .text_color(palette.fg)
            .text_size(px(13.0))
            .child(
                h_flex()
                    .h(px(44.0))
                    .flex_none()
                    .px(px(14.0))
                    .gap(px(10.0))
                    .items_center()
                    .border_b_1()
                    .border_color(palette.border)
                    .child(logo(22.0))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Latexo"),
                    )
                    .child(badge(
                        t!("nav.live", interval = interval),
                        palette.success,
                        palette.success_subtle,
                    )),
            )
            .child(
                v_flex()
                    .id("flyout-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(14.0))
                    .pt(px(12.0))
                    .gap(px(8.0))
                    .when_some(alert, |column, text| {
                        column.child(
                            h_flex()
                                .flex_none()
                                .h(px(36.0))
                                .px(px(12.0))
                                .gap(px(8.0))
                                .items_center()
                                .rounded(px(8.0))
                                .bg(palette.warning_subtle)
                                .text_color(palette.warning)
                                .text_size(px(TEXT_SM))
                                .child(Icon::new(IconName::TriangleAlert).with_size(px(14.0)))
                                .child(div().flex_1().min_w_0().truncate().child(text)),
                        )
                    })
                    .child(
                        v_flex().children(rows.into_iter().enumerate().map(|(index, row)| {
                            h_flex()
                                .h(px(50.0))
                                .gap(px(10.0))
                                .items_center()
                                .when(index > 0, |line| {
                                    line.border_t_1().border_color(palette.border)
                                })
                                .child(icon_tile(screen_icon(row.screen), &palette).size(px(24.0)))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(px(TEXT_ROW))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(row.name),
                                        )
                                        .child(
                                            div()
                                                .font_family(palette_mono())
                                                .text_size(px(TEXT_SM))
                                                .text_color(palette.fg_muted)
                                                .truncate()
                                                .child(row.value),
                                        ),
                                )
                                .child(div().w(px(CHART_WIDTH)).flex_none().child(row.chart))
                        })),
                    ),
            )
            .child(
                h_flex()
                    .h(px(52.0))
                    .flex_none()
                    .px(px(14.0))
                    .gap(px(8.0))
                    .items_center()
                    .border_t_1()
                    .border_color(palette.border)
                    .bg(palette.modal_footer)
                    .child(
                        div().flex_1().child(
                            SecondaryButton::new("flyout-open", t!("tray.open"))
                                .full_width()
                                .on_click(move |_, window, cx| {
                                    let _ =
                                        main.update(cx, |_, window, _| window.activate_window());
                                    window.remove_window();
                                }),
                        ),
                    )
                    .child(
                        SecondaryButton::new("flyout-pill", t!("tray.pill")).on_click(
                            move |_, window, cx| {
                                let app = app.clone();
                                let _ = main.update(cx, move |_, main_window, cx| {
                                    app.update(cx, |view, cx| {
                                        let on = !view.settings.floating_pill;
                                        view.change(main_window, cx, |settings| {
                                            settings.floating_pill = on
                                        });
                                        view.set_pill(on, main_window, cx);
                                    });
                                });
                                window.remove_window();
                            },
                        ),
                    ),
            )
    }
}

impl AppView {
    /// Opens the flyout, or closes it when it is open.
    pub(crate) fn toggle_flyout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A flyout closed by its own buttons leaves a handle to a window that is gone: then
        // this click opens a new one.
        if let Some(flyout) = self.flyout.take()
            && flyout
                .update(cx, |_, window, _| window.remove_window())
                .is_ok()
        {
            return;
        }
        let app = cx.entity();
        let main = window.window_handle();
        let options = flyout_options(cx);
        let theme = self.settings.theme;
        cx.spawn_in(window, async move |view, cx| {
            let opened = cx.update(|_, cx| {
                cx.open_window(options, move |window, cx| {
                    telaio_ui::ThemeChoice::from(theme).apply(Some(window), cx);
                    let flyout = cx.new(|cx| FlyoutView::new(app, main, window, cx));
                    cx.new(|cx| gpui_kit::component::Root::new(flyout, window, cx))
                })
            });
            match opened {
                Ok(Ok(handle)) => {
                    let _ = view.update(cx, |view, _| view.flyout = Some(handle.into()));
                }
                Ok(Err(error)) | Err(error) => {
                    log::warn!("could not open the tray flyout: {error}");
                }
            }
        })
        .detach();
    }
}

/// The flyout at the bottom right corner of the primary display, over the tray: a layer shell
/// surface on Wayland, a pop-up elsewhere.
fn flyout_options(cx: &App) -> WindowOptions {
    let size = size(px(WIDTH), px(HEIGHT));
    let origin = cx
        .primary_display()
        .map(|display| {
            let area = display.visible_bounds();
            point(
                area.origin.x + area.size.width - size.width - px(MARGIN),
                area.origin.y + area.size.height - size.height - px(MARGIN),
            )
        })
        .unwrap_or_else(|| point(px(0.0), px(0.0)));
    WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size))),
        focus: true,
        show: true,
        kind: flyout_kind(cx),
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        app_id: Some(format!("{}.flyout", latexo_cli::IDENTITY.app_id)),
        ..WindowOptions::default()
    }
}

#[cfg(target_os = "linux")]
fn flyout_kind(cx: &App) -> WindowKind {
    use gpui_kit::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
    if cx.compositor_name() == "Wayland" {
        WindowKind::LayerShell(LayerShellOptions {
            namespace: "latexo-flyout".to_string(),
            layer: Layer::Overlay,
            anchor: Anchor::BOTTOM | Anchor::RIGHT,
            exclusive_zone: None,
            exclusive_edge: None,
            margin: Some((px(0.0), px(MARGIN), px(MARGIN), px(0.0))),
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
        })
    } else {
        WindowKind::PopUp
    }
}

#[cfg(not(target_os = "linux"))]
fn flyout_kind(_cx: &App) -> WindowKind {
    WindowKind::PopUp
}
