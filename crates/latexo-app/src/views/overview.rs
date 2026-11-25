//! The Overview screen: one card per resource with its value and chart (a click opens its
//! screen), the busiest processes and the power card.

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::energy::cost;
use latexo_core::process::busiest;
use latexo_core::{Sample, SeriesKey};
use rust_i18n::t;

use crate::state::format;
use crate::state::nav::Screen;
use crate::theme::metrics::{
    BUSY_ROW_HEIGHT, CARD_PADDING, GAP, OVERVIEW_CHART_HEIGHT, OVERVIEW_CHART_HEIGHT_SM, RADIUS_LG,
    TEXT_ROW, TEXT_SM, VALUE_TEXT_SIZE,
};
use crate::theme::{LatexoColors, Palette};

use super::chart::Chart;
use super::root::{AppView, screen_icon, screen_label};
use super::widgets::{bar, big_value, card, card_header, chart_well, muted, palette_mono};

/// One resource card.
struct ResourceCard {
    screen: Screen,
    value: String,
    sub: String,
    chart: Chart,
}

impl AppView {
    pub(crate) fn render_overview(
        &self,
        sample: &Sample,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unit = self.settings.temperature_unit.into();
        let mut cards = Vec::new();

        let cpu_sub = match (sample.cpu.user, sample.cpu.system) {
            (Some(user), Some(system)) => t!(
                "overview.cpu_split",
                user = format::percent(user, 0),
                system = format::percent(system, 0)
            )
            .into_owned(),
            _ => sample
                .cpu
                .average_frequency_mhz()
                .map(format::frequency)
                .unwrap_or_default(),
        };
        let cpu_chart = if sample.cpu.user.is_some() {
            self.stacked_chart(
                &SeriesKey::CpuUser,
                &SeriesKey::CpuSystem,
                1.0,
                OVERVIEW_CHART_HEIGHT,
                palette,
            )
        } else {
            self.chart(&SeriesKey::CpuUsage, 1.0, OVERVIEW_CHART_HEIGHT, palette)
        };
        cards.push(ResourceCard {
            screen: Screen::Cpu,
            value: format::percent(sample.cpu.usage, 0),
            sub: cpu_sub,
            chart: cpu_chart,
        });

        if let Some(gpu) = sample.gpus.first() {
            let memory = match (gpu.memory_used, gpu.memory_total) {
                (Some(used), Some(total)) if total > 0 => t!(
                    "overview.gpu_memory",
                    used = format::bytes(used),
                    total = format::bytes(total)
                )
                .into_owned(),
                _ => String::new(),
            };
            cards.push(ResourceCard {
                screen: Screen::Gpu,
                value: gpu
                    .usage
                    .map(|usage| format::percent(usage, 0))
                    .unwrap_or_else(|| t!("common.not_available").into_owned()),
                sub: [gpu.name.clone(), memory]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
                chart: self.chart(&SeriesKey::GpuUsage(0), 1.0, OVERVIEW_CHART_HEIGHT, palette),
            });
        }

        cards.push(ResourceCard {
            screen: Screen::Memory,
            value: format::percent(sample.memory.usage(), 0),
            sub: t!(
                "overview.memory_used",
                used = format::bytes(sample.memory.used),
                total = format::bytes(sample.memory.total)
            )
            .into_owned(),
            chart: self.chart(
                &SeriesKey::MemoryUsed,
                sample.memory.total as f32,
                OVERVIEW_CHART_HEIGHT,
                palette,
            ),
        });

        let disk_max = self.stacked_max(&SeriesKey::DiskRead, &SeriesKey::DiskWrite, 1_048_576.0);
        cards.push(ResourceCard {
            screen: Screen::Disk,
            value: format::rate(sample.disk_io.read_per_sec + sample.disk_io.write_per_sec),
            sub: t!(
                "overview.disk_split",
                read = format::rate(sample.disk_io.read_per_sec),
                write = format::rate(sample.disk_io.write_per_sec)
            )
            .into_owned(),
            chart: self.stacked_chart(
                &SeriesKey::DiskRead,
                &SeriesKey::DiskWrite,
                disk_max,
                OVERVIEW_CHART_HEIGHT_SM,
                palette,
            ),
        });

        let net_max = self.stacked_max(&SeriesKey::NetReceived, &SeriesKey::NetSent, 102_400.0);
        cards.push(ResourceCard {
            screen: Screen::Network,
            value: format::rate(sample.network.received_per_sec + sample.network.sent_per_sec),
            sub: t!(
                "overview.net_split",
                down = format::rate(sample.network.received_per_sec),
                up = format::rate(sample.network.sent_per_sec)
            )
            .into_owned(),
            chart: self.stacked_chart(
                &SeriesKey::NetReceived,
                &SeriesKey::NetSent,
                net_max,
                OVERVIEW_CHART_HEIGHT_SM,
                palette,
            ),
        });

        let hottest = sample.hottest_sensor().map(|sensor| {
            t!(
                "overview.hottest",
                sensor = super::temperatures::sensor_label(&sensor.label),
                value = format::temperature(sensor.celsius, unit)
            )
            .into_owned()
        });
        cards.push(ResourceCard {
            screen: Screen::Temperatures,
            value: sample
                .cpu_temperature()
                .map(|celsius| format::temperature(celsius, unit))
                .unwrap_or_else(|| t!("common.not_available").into_owned()),
            sub: hottest.unwrap_or_default(),
            chart: self
                .chart(
                    &SeriesKey::CpuTemperature,
                    110.0,
                    OVERVIEW_CHART_HEIGHT_SM,
                    palette,
                )
                .threshold(80.0, palette.warning),
        });

        let columns = 3;
        let card_elements: Vec<AnyElement> = cards
            .into_iter()
            .map(|resource| self.resource_card(resource, palette, cx))
            .collect();
        let mut rows: Vec<AnyElement> = card_elements
            .into_iter()
            .collect::<Vec<_>>()
            .chunks_mut(columns)
            .map(|chunk| {
                let mut row = h_flex().gap(px(GAP)).items_stretch();
                let filled = chunk.len();
                for element in chunk.iter_mut() {
                    let element = std::mem::replace(element, div().into_any_element());
                    row = row.child(div().flex_1().min_w_0().flex().child(element));
                }
                for _ in filled..columns {
                    row = row.child(div().flex_1().min_w_0());
                }
                row.into_any_element()
            })
            .collect();
        rows.push(
            h_flex()
                .gap(px(GAP))
                .items_stretch()
                .child(
                    div()
                        .flex_grow(2.0)
                        .flex_basis(relative(0.0))
                        .min_w_0()
                        .flex()
                        .child(self.busiest_card(sample, palette)),
                )
                .child(
                    div()
                        .flex_grow(1.0)
                        .flex_basis(relative(0.0))
                        .min_w_0()
                        .flex()
                        .child(self.power_card(sample, palette, cx)),
                )
                .into_any_element(),
        );
        v_flex().gap(px(GAP)).children(rows).into_any_element()
    }

    fn resource_card(
        &self,
        resource: ResourceCard,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let screen = resource.screen;
        let (border_hover, pressed) = (palette.border_strong, palette.pressed);
        card(palette)
            .id(SharedString::from(format!("card-{}", screen.code())))
            .flex_1()
            .cursor_pointer()
            .hover(move |style| style.border_color(border_hover))
            .active(move |style| style.bg(pressed))
            .child(
                card_header(Some(screen_icon(screen)), screen_label(screen), palette).child(
                    Icon::new(IconName::ChevronRight)
                        .with_size(px(16.0))
                        .text_color(palette.fg_subtle),
                ),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .child(big_value(resource.value, VALUE_TEXT_SIZE)),
            )
            .child(div().mt(px(2.0)).child(muted(resource.sub, palette)))
            .child(
                div()
                    .mt(px(12.0))
                    .child(chart_well(resource.chart, palette)),
            )
            .on_click(cx.listener(move |view, _, _, cx| view.show(screen, cx)))
            .into_any_element()
    }

    fn busiest_card(&self, sample: &Sample, palette: &Palette) -> impl IntoElement {
        let rows = busiest(&sample.processes, 5);
        card(palette)
            .flex_1()
            .child(
                card_header(Some(IconName::List), t!("overview.busiest"), palette)
                    .mb(px(6.0))
                    .child(muted(t!("overview.busiest_unit"), palette)),
            )
            .children(rows.into_iter().map(|process| {
                h_flex()
                    .h(px(BUSY_ROW_HEIGHT))
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .w(px(210.0))
                            .flex_none()
                            .truncate()
                            .text_size(px(TEXT_ROW))
                            .child(process.name.clone()),
                    )
                    .child(div().flex_1().child(bar(
                        process.cpu_percent / 100.0,
                        palette.accent,
                        palette,
                    )))
                    .child(
                        div()
                            .w(px(60.0))
                            .flex_none()
                            .text_right()
                            .font_family(palette_mono())
                            .text_size(px(TEXT_SM))
                            .child(format::percent_value(f64::from(process.cpu_percent), 1)),
                    )
            }))
    }

    fn power_card(
        &self,
        sample: &Sample,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let power = sample.power;
        let total = power.measured_total();
        let today = self.monitor.energy.today_wh;
        let price = f64::from(self.settings.energy_price);
        let sub = t!(
            "overview.energy_today",
            energy = format::energy(today),
            cost = format::money(cost(today, price), &self.settings.currency)
        )
        .into_owned();
        let mut rows: Vec<(String, String, Hsla)> = Vec::new();
        if let Some(cpu) = power.cpu_package {
            rows.push((
                t!("energy.cpu").into_owned(),
                format::watts(cpu),
                palette.accent,
            ));
        }
        if let Some(gpu) = power.gpu {
            rows.push((
                t!("energy.gpu").into_owned(),
                format::watts(gpu),
                palette.series_secondary(),
            ));
        }
        if let Some(battery) = power.battery {
            rows.push((
                t!("energy.battery").into_owned(),
                format::percent(battery.charge, 0),
                palette.fg_subtle,
            ));
        }
        let (border_hover, pressed) = (palette.border_strong, palette.pressed);
        card(palette)
            .id("card-energy")
            .flex_1()
            .cursor_pointer()
            .hover(move |style| style.border_color(border_hover))
            .active(move |style| style.bg(pressed))
            .child(
                card_header(Some(IconName::Zap), t!("nav.energy"), palette).child(
                    Icon::new(IconName::ChevronRight)
                        .with_size(px(16.0))
                        .text_color(palette.fg_subtle),
                ),
            )
            .child(
                div().mt(px(10.0)).child(big_value(
                    total
                        .map(format::watts)
                        .unwrap_or_else(|| t!("energy.not_measured").into_owned()),
                    VALUE_TEXT_SIZE,
                )),
            )
            .child(div().mt(px(2.0)).child(muted(sub, palette)))
            .child(
                v_flex()
                    .mt(px(10.0))
                    .children(rows.into_iter().map(|(label, value, color)| {
                        h_flex()
                            .h(px(22.0))
                            .justify_between()
                            .items_center()
                            .child(super::widgets::legend_item(label, color, palette))
                            .child(
                                div()
                                    .font_family(palette_mono())
                                    .text_size(px(TEXT_SM))
                                    .child(value),
                            )
                    })),
            )
            .on_click(cx.listener(|view, _, _, cx| view.show(Screen::Energy, cx)))
            .rounded(px(RADIUS_LG))
            .p(px(CARD_PADDING))
    }
}
