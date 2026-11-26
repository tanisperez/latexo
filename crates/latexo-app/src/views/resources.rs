//! The detail screens of one resource (CPU, Memory, Disk, Network, Temperatures, Energy), all on
//! the design's layout: a big chart card with its value, axis and legend, then a list of bars,
//! an optional list of processes and a Details card.

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::energy::cost;
use latexo_core::process::{top_disk, top_memory};
use latexo_core::{Sample, SensorKind, SeriesKey};
use rust_i18n::t;

use crate::state::format;
use crate::theme::metrics::{AXIS_WIDTH, DETAIL_CHART_HEIGHT, GAP, TEXT_AXIS};
use crate::theme::{LatexoColors, Palette};

use super::chart::Chart;
use super::root::{AppView, range_label};
use super::widgets::{
    badge, bar_row, card, card_header, card_title, chart_well, empty_line, legend_item, name_rows,
    note, stat_rows, value_line,
};

/// A bar of a list: label, value, fraction and color.
pub struct BarItem {
    pub label: String,
    pub value: String,
    pub fraction: f32,
    pub color: Hsla,
}

/// Everything a detail screen shows.
pub struct Detail {
    pub icon: IconName,
    pub chart_title: String,
    pub legend: Vec<(String, Hsla)>,
    pub badge: Option<(String, Hsla, Hsla)>,
    pub value: String,
    pub value_sub: String,
    pub axis: [String; 3],
    pub chart: Chart,
    pub note: Option<String>,
    pub list_title: String,
    pub list: Vec<BarItem>,
    pub list_columns: usize,
    pub top: Option<(String, Vec<(String, String)>)>,
    pub stats: Vec<(String, String)>,
}

impl AppView {
    /// The labels under a detail chart: the start of the range, its middle and now.
    fn time_labels(&self, palette: &Palette) -> impl IntoElement {
        let range = self.range();
        let half = range.secs() / 2.0;
        let half = if half >= 3600.0 {
            t!("charts.ago_h", value = format::decimal(half / 3600.0, 0)).into_owned()
        } else if half >= 60.0 {
            t!("charts.ago_min", value = format::decimal(half / 60.0, 1)).into_owned()
        } else {
            t!("charts.ago_s", value = format::decimal(half, 0)).into_owned()
        };
        h_flex()
            .justify_between()
            .ml(px(AXIS_WIDTH + 8.0))
            .mt(px(6.0))
            .text_size(px(TEXT_AXIS))
            .text_color(palette.fg_muted)
            .child(format!("-{}", range_label(range)))
            .child(half)
            .child(t!("charts.now").into_owned())
    }

    pub(crate) fn render_detail(&self, detail: Detail, palette: &Palette) -> AnyElement {
        let Detail {
            icon,
            chart_title,
            legend,
            badge: header_badge,
            value,
            value_sub,
            axis,
            chart,
            note: detail_note,
            list_title,
            list,
            list_columns,
            top,
            stats,
        } = detail;
        let mut header = card_header(Some(icon), chart_title, palette);
        if let Some((text, fg, bg)) = header_badge {
            header = header.child(badge(text, fg, bg));
        }
        for (label, color) in legend {
            header = header.child(div().ml(px(12.0)).child(legend_item(label, color, palette)));
        }
        let axis_labels = v_flex()
            .w(px(AXIS_WIDTH))
            .flex_none()
            .h(px(DETAIL_CHART_HEIGHT + 20.0))
            .py(px(6.0))
            .justify_between()
            .items_end()
            .text_size(px(TEXT_AXIS))
            .text_color(palette.fg_muted)
            .children(axis);
        let chart_card = card(palette)
            .child(header)
            .child(
                div()
                    .mt(px(10.0))
                    .child(value_line(value, value_sub, palette)),
            )
            .child(
                h_flex()
                    .mt(px(12.0))
                    .gap(px(8.0))
                    .child(axis_labels)
                    .child(div().flex_1().min_w_0().child(chart_well(chart, palette))),
            )
            .child(self.time_labels(palette))
            .when_some(detail_note, |card, text| {
                card.child(
                    div()
                        .mt(px(12.0))
                        .child(note(text, palette.info, palette.info_subtle)),
                )
            });

        let list_card = {
            let rows: Vec<AnyElement> = list
                .into_iter()
                .map(|item| {
                    bar_row(item.label, item.value, item.fraction, item.color, palette)
                        .into_any_element()
                })
                .collect();
            let body = if rows.is_empty() {
                empty_line(t!("common.nothing_found"), palette).into_any_element()
            } else {
                let columns = list_columns.max(1);
                let mut grid = v_flex().gap(px(8.0));
                let mut rows = rows.into_iter().peekable();
                while rows.peek().is_some() {
                    let mut line = h_flex().gap(px(28.0));
                    for _ in 0..columns {
                        line = line.child(div().flex_1().min_w_0().children(rows.next()));
                    }
                    grid = grid.child(line);
                }
                grid.into_any_element()
            };
            card(palette)
                .flex_1()
                .child(card_title(list_title).mb(px(12.0)))
                .child(body)
        };
        let has_top = top.is_some();
        let mut lower = h_flex().items_start().gap(px(GAP)).child(
            div()
                .flex_grow(if has_top { 3.3 } else { 6.0 })
                .flex_basis(relative(0.0))
                .min_w_0()
                .flex()
                .child(list_card),
        );
        if let Some((title, rows)) = top {
            let body = if rows.is_empty() {
                empty_line(t!("common.nothing_found"), palette).into_any_element()
            } else {
                name_rows(rows, palette).into_any_element()
            };
            lower = lower.child(
                div()
                    .flex_grow(3.3)
                    .flex_basis(relative(0.0))
                    .min_w_0()
                    .flex()
                    .child(card(palette).flex_1().child(card_title(title)).child(body)),
            );
        }
        lower = lower.child(
            div()
                .flex_grow(if has_top { 3.3 } else { 4.0 })
                .flex_basis(relative(0.0))
                .min_w_0()
                .flex()
                .child(
                    card(palette)
                        .flex_1()
                        .child(card_title(t!("common.details")))
                        .child(stat_rows(stats, palette)),
                ),
        );
        v_flex()
            .gap(px(GAP))
            .child(chart_card)
            .child(lower)
            .into_any_element()
    }

    pub(crate) fn render_cpu(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let cpu = &sample.cpu;
        let split = cpu.user.zip(cpu.system);
        let chart = if split.is_some() {
            self.stacked_chart(
                &SeriesKey::CpuUser,
                &SeriesKey::CpuSystem,
                1.0,
                DETAIL_CHART_HEIGHT,
                palette,
            )
        } else {
            self.chart(&SeriesKey::CpuUsage, 1.0, DETAIL_CHART_HEIGHT, palette)
        };
        let legend = if split.is_some() {
            vec![
                (t!("cpu.user").into_owned(), palette.accent),
                (t!("cpu.system").into_owned(), palette.series_secondary()),
            ]
        } else {
            vec![(t!("cpu.usage").into_owned(), palette.accent)]
        };
        let mut stats = Vec::new();
        if let Some((user, system)) = split {
            stats.push((t!("cpu.user").into_owned(), format::percent(user, 0)));
            stats.push((t!("cpu.system").into_owned(), format::percent(system, 0)));
        }
        stats.push((
            t!("cpu.idle").into_owned(),
            format::percent(1.0 - cpu.usage, 0),
        ));
        if let Some(mhz) = cpu.average_frequency_mhz() {
            stats.push((t!("cpu.frequency").into_owned(), format::frequency(mhz)));
        }
        if let Some(load) = cpu.load_average {
            stats.push((
                t!("cpu.load").into_owned(),
                load.map(|value| format::decimal(value, 2)).join(" · "),
            ));
        }
        if let Some(watts) = sample.power.cpu_package {
            stats.push((t!("cpu.package_power").into_owned(), format::watts(watts)));
        }
        stats.push((
            t!("nav.processes").into_owned(),
            format::integer(sample.processes.len()),
        ));
        stats.push((
            t!("cpu.uptime").into_owned(),
            format::uptime(sample.uptime_secs),
        ));
        let value_sub = match split {
            Some((user, system)) => t!(
                "overview.cpu_split",
                user = format::percent(user, 0),
                system = format::percent(system, 0)
            )
            .into_owned(),
            None => String::new(),
        };
        let list = cpu
            .cores
            .iter()
            .enumerate()
            .map(|(index, core)| BarItem {
                label: t!("cores.core", number = index + 1).into_owned(),
                value: format::percent(core.usage, 0),
                fraction: core.usage,
                color: palette.accent,
            })
            .collect();
        self.render_detail(
            Detail {
                icon: IconName::Cpu,
                chart_title: t!("cpu.chart").into_owned(),
                legend,
                badge: None,
                value: format::percent(cpu.usage, 0),
                value_sub,
                axis: [format::percent(1.0, 0), format::percent(0.5, 0), "0".into()],
                chart,
                note: None,
                list_title: t!("cpu.processors").into_owned(),
                list,
                list_columns: 2,
                top: None,
                stats,
            },
            palette,
        )
    }

    pub(crate) fn render_memory(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let memory = sample.memory;
        let total = memory.total.max(1);
        let fraction = |bytes: u64| bytes as f32 / total as f32;
        let mut list = vec![
            BarItem {
                label: t!("memory.in_use").into_owned(),
                value: format::bytes(memory.used),
                fraction: fraction(memory.used),
                color: palette.accent,
            },
            BarItem {
                label: t!("memory.cached").into_owned(),
                value: format::bytes(memory.cached()),
                fraction: fraction(memory.cached()),
                color: palette.series_secondary(),
            },
            BarItem {
                label: t!("memory.free").into_owned(),
                value: format::bytes(memory.free),
                fraction: fraction(memory.free),
                color: palette.fg_subtle,
            },
        ];
        if memory.swap_total > 0 {
            list.push(BarItem {
                label: t!("memory.swap").into_owned(),
                value: t!(
                    "common.of",
                    used = format::bytes(memory.swap_used),
                    total = format::bytes(memory.swap_total)
                )
                .into_owned(),
                fraction: memory.swap_used as f32 / memory.swap_total as f32,
                color: palette.accent,
            });
        }
        let usage = memory.usage();
        let badge = if usage >= 0.9 {
            (
                t!("memory.high").into_owned(),
                palette.warning,
                palette.warning_subtle,
            )
        } else {
            (
                t!("memory.normal").into_owned(),
                palette.success,
                palette.success_subtle,
            )
        };
        let top = top_memory(&sample.processes, 5)
            .into_iter()
            .map(|process| (process.name.clone(), format::bytes(process.memory)))
            .collect();
        self.render_detail(
            Detail {
                icon: IconName::MemoryStick,
                chart_title: t!("memory.chart").into_owned(),
                legend: vec![(t!("memory.in_use").into_owned(), palette.accent)],
                badge: Some(badge),
                value: format::bytes(memory.used),
                value_sub: t!(
                    "memory.value_sub",
                    total = format::bytes(memory.total),
                    percent = format::percent(usage, 0)
                )
                .into_owned(),
                axis: [
                    format::bytes(memory.total),
                    format::bytes(memory.total / 2),
                    "0".into(),
                ],
                chart: self.chart(
                    &SeriesKey::MemoryUsed,
                    memory.total as f32,
                    DETAIL_CHART_HEIGHT,
                    palette,
                ),
                note: None,
                list_title: t!("memory.breakdown").into_owned(),
                list,
                list_columns: 1,
                top: Some((t!("memory.top").into_owned(), top)),
                stats: vec![
                    (t!("memory.total").into_owned(), format::bytes(memory.total)),
                    (t!("memory.in_use").into_owned(), format::bytes(memory.used)),
                    (
                        t!("memory.available").into_owned(),
                        format::bytes(memory.available),
                    ),
                    (
                        t!("memory.swap").into_owned(),
                        t!(
                            "common.of",
                            used = format::bytes(memory.swap_used),
                            total = format::bytes(memory.swap_total)
                        )
                        .into_owned(),
                    ),
                ],
            },
            palette,
        )
    }

    pub(crate) fn render_disk(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let io = sample.disk_io;
        let max = self.stacked_max(&SeriesKey::DiskRead, &SeriesKey::DiskWrite, 1_048_576.0);
        let list = sample
            .volumes
            .iter()
            .map(|volume| {
                let usage = volume.usage();
                BarItem {
                    label: volume_label(volume),
                    value: t!(
                        "common.of",
                        used = format::bytes(volume.total.saturating_sub(volume.available)),
                        total = format::bytes(volume.total)
                    )
                    .into_owned(),
                    fraction: usage,
                    color: if usage >= 0.85 {
                        palette.warning
                    } else {
                        palette.accent
                    },
                }
            })
            .collect();
        let top = top_disk(&sample.processes, 5)
            .into_iter()
            .map(|process| (process.name.clone(), format::rate(process.disk_per_sec())))
            .collect();
        let fullest = sample
            .volumes
            .iter()
            .map(|volume| volume.usage())
            .fold(0.0f32, f32::max);
        let badge = (fullest >= 0.9).then(|| {
            (
                t!("disk.almost_full").into_owned(),
                palette.warning,
                palette.warning_subtle,
            )
        });
        self.render_detail(
            Detail {
                icon: IconName::HardDrive,
                chart_title: t!("disk.chart").into_owned(),
                legend: vec![
                    (t!("disk.read").into_owned(), palette.accent),
                    (t!("disk.write").into_owned(), palette.series_secondary()),
                ],
                badge,
                value: format::rate(io.read_per_sec + io.write_per_sec),
                value_sub: t!(
                    "overview.disk_split",
                    read = format::rate(io.read_per_sec),
                    write = format::rate(io.write_per_sec)
                )
                .into_owned(),
                axis: rate_axis(max),
                chart: self.stacked_chart(
                    &SeriesKey::DiskRead,
                    &SeriesKey::DiskWrite,
                    max,
                    DETAIL_CHART_HEIGHT,
                    palette,
                ),
                note: None,
                list_title: t!("disk.volumes_title").into_owned(),
                list,
                list_columns: 1,
                top: Some((t!("disk.top").into_owned(), top)),
                stats: vec![
                    (t!("disk.read").into_owned(), format::rate(io.read_per_sec)),
                    (
                        t!("disk.write").into_owned(),
                        format::rate(io.write_per_sec),
                    ),
                    (
                        t!("disk.read_session").into_owned(),
                        format::bytes(io.read_total),
                    ),
                    (
                        t!("disk.written_session").into_owned(),
                        format::bytes(io.written_total),
                    ),
                ],
            },
            palette,
        )
    }

    pub(crate) fn render_network(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let network = &sample.network;
        let max = self.stacked_max(&SeriesKey::NetReceived, &SeriesKey::NetSent, 102_400.0);
        let busiest = network
            .interfaces
            .iter()
            .map(|i| i.received_per_sec + i.sent_per_sec)
            .fold(0.0f64, f64::max)
            .max(1.0);
        let list = network
            .interfaces
            .iter()
            .map(|interface| {
                let rate = interface.received_per_sec + interface.sent_per_sec;
                BarItem {
                    label: interface.name.clone(),
                    value: if interface.is_active() {
                        format::rate(rate)
                    } else {
                        t!("network.inactive").into_owned()
                    },
                    fraction: (rate / busiest) as f32,
                    color: palette.accent,
                }
            })
            .collect();
        let main = network
            .interfaces
            .iter()
            .filter(|interface| interface.address.is_some())
            .max_by_key(|interface| interface.received_total);
        let mut stats = vec![
            (
                t!("network.down").into_owned(),
                format::rate(network.received_per_sec),
            ),
            (
                t!("network.up").into_owned(),
                format::rate(network.sent_per_sec),
            ),
            (
                t!("network.received_session").into_owned(),
                format::bytes(network.received_session),
            ),
            (
                t!("network.sent_session").into_owned(),
                format::bytes(network.sent_session),
            ),
        ];
        if let Some(main) = main {
            if let Some(address) = &main.address {
                stats.push((t!("network.address").into_owned(), address.clone()));
            }
            if let Some(mac) = &main.mac {
                stats.push((t!("network.mac").into_owned(), mac.clone()));
            }
        }
        self.render_detail(
            Detail {
                icon: IconName::ArrowUpDown,
                chart_title: t!("network.chart").into_owned(),
                legend: vec![
                    (t!("network.down").into_owned(), palette.accent),
                    (t!("network.up").into_owned(), palette.series_secondary()),
                ],
                badge: None,
                value: format::rate(network.received_per_sec + network.sent_per_sec),
                value_sub: t!(
                    "overview.net_split",
                    down = format::rate(network.received_per_sec),
                    up = format::rate(network.sent_per_sec)
                )
                .into_owned(),
                axis: rate_axis(max),
                chart: self.stacked_chart(
                    &SeriesKey::NetReceived,
                    &SeriesKey::NetSent,
                    max,
                    DETAIL_CHART_HEIGHT,
                    palette,
                ),
                note: None,
                list_title: t!("network.interfaces").into_owned(),
                list,
                list_columns: 1,
                top: None,
                stats,
            },
            palette,
        )
    }

    pub(crate) fn render_temperatures(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let unit = self.settings.temperature_unit.into();
        let list = sample
            .sensors
            .iter()
            .map(|sensor| BarItem {
                label: super::temperatures::sensor_label(&sensor.label),
                value: format::temperature(sensor.celsius, unit),
                fraction: sensor.celsius / 110.0,
                color: palette.temperature(sensor.celsius),
            })
            .collect();
        let hot = sample
            .sensors
            .iter()
            .filter(|sensor| sensor.celsius >= 80.0)
            .count();
        let badge = (hot > 0).then(|| {
            (
                crate::state::locale::plural("temperatures.hot", hot),
                palette.warning,
                palette.warning_subtle,
            )
        });
        let cpu = sample.cpu_temperature();
        let max = self.max_in_range(&SeriesKey::CpuTemperature);
        let mut stats = Vec::new();
        if let Some(cpu) = cpu {
            stats.push((
                t!("temperatures.cpu").into_owned(),
                format::temperature(cpu, unit),
            ));
        }
        if let Some(max) = max {
            stats.push((
                t!(
                    "temperatures.max_in_range",
                    range = range_label(self.range())
                )
                .into_owned(),
                format::temperature(max, unit),
            ));
        }
        if let Some(sensor) = sample
            .sensors
            .iter()
            .filter(|sensor| sensor.kind == Some(SensorKind::Storage))
            .max_by(|a, b| a.celsius.total_cmp(&b.celsius))
        {
            stats.push((
                t!("temperatures.storage").into_owned(),
                format::temperature(sensor.celsius, unit),
            ));
        }
        if let Some(critical) = sample
            .sensors
            .iter()
            .filter(|sensor| {
                matches!(
                    sensor.kind,
                    Some(SensorKind::CpuPackage | SensorKind::CpuCore)
                )
            })
            .find_map(|sensor| sensor.critical)
        {
            stats.push((
                t!("temperatures.critical").into_owned(),
                format::temperature(critical, unit),
            ));
        }
        for fan in &sample.fans {
            stats.push((
                super::temperatures::fan_label(&fan.label),
                format::rpm(fan.rpm),
            ));
        }
        let note = sample
            .sensors
            .is_empty()
            .then(|| t!("temperatures.none_note").into_owned());
        let axis_top = format::temperature(110.0, unit);
        let axis_mid = format::temperature(55.0, unit);
        self.render_detail(
            Detail {
                icon: IconName::Thermometer,
                chart_title: t!("temperatures.chart").into_owned(),
                legend: vec![
                    (t!("temperatures.normal").into_owned(), palette.accent),
                    (t!("temperatures.high").into_owned(), palette.warning),
                ],
                badge,
                value: cpu
                    .map(|celsius| format::temperature(celsius, unit))
                    .unwrap_or_else(|| t!("common.not_available").into_owned()),
                value_sub: max
                    .map(|max| {
                        t!(
                            "temperatures.max_sub",
                            range = range_label(self.range()),
                            value = format::temperature(max, unit)
                        )
                        .into_owned()
                    })
                    .unwrap_or_default(),
                axis: [axis_top, axis_mid, "0".into()],
                chart: self
                    .chart(
                        &SeriesKey::CpuTemperature,
                        110.0,
                        DETAIL_CHART_HEIGHT,
                        palette,
                    )
                    .threshold(80.0, palette.warning),
                note,
                list_title: t!("temperatures.sensors_title").into_owned(),
                list,
                list_columns: 2,
                top: None,
                stats,
            },
            palette,
        )
    }

    pub(crate) fn render_energy(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        let power = sample.power;
        let total = power.measured_total();
        let max = self.auto_max(&SeriesKey::PowerTotal, 10.0);
        let price = f64::from(self.settings.energy_price);
        let currency = &self.settings.currency;
        let account = self.monitor.energy;
        let mut list = Vec::new();
        let reference = total.unwrap_or(0.0).max(1.0);
        if let Some(cpu) = power.cpu_package {
            list.push(BarItem {
                label: t!("energy.cpu").into_owned(),
                value: t!("energy.measured", value = format::watts(cpu)).into_owned(),
                fraction: cpu / reference,
                color: palette.accent,
            });
        }
        if let Some(gpu) = power.gpu {
            list.push(BarItem {
                label: t!("energy.gpu").into_owned(),
                value: t!("energy.measured", value = format::watts(gpu)).into_owned(),
                fraction: gpu / reference,
                color: palette.series_secondary(),
            });
        }
        if let Some(battery) = power.battery {
            let state = match battery.watts {
                Some(watts) if watts < -0.05 => {
                    t!("energy.discharging", value = format::watts(watts)).into_owned()
                }
                Some(watts) if watts > 0.05 => {
                    t!("energy.charging", value = format::watts(watts)).into_owned()
                }
                _ if battery.on_ac => t!("energy.plugged").into_owned(),
                _ => String::new(),
            };
            list.push(BarItem {
                label: t!("energy.battery").into_owned(),
                value: [format::percent(battery.charge, 0), state]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
                fraction: battery.charge,
                color: if battery.charge < 0.15 {
                    palette.warning
                } else {
                    palette.success
                },
            });
        }
        let mut stats = vec![
            (
                t!("energy.now").into_owned(),
                total
                    .map(format::watts)
                    .unwrap_or_else(|| t!("energy.not_measured").into_owned()),
            ),
            (
                t!("energy.today").into_owned(),
                format!(
                    "{} · {}",
                    format::energy(account.today_wh),
                    format::money(cost(account.today_wh, price), currency)
                ),
            ),
            (
                t!("energy.month").into_owned(),
                format!(
                    "{} · {}",
                    format::energy(account.month_wh),
                    format::money(cost(account.month_wh, price), currency)
                ),
            ),
            (
                t!("energy.price").into_owned(),
                format!("{} {currency}/kWh", format::decimal(price, 2)),
            ),
        ];
        if let Some(max) = self.max_in_range(&SeriesKey::PowerTotal) {
            stats.insert(
                1,
                (
                    t!("energy.max_in_range", range = range_label(self.range())).into_owned(),
                    format::watts(max),
                ),
            );
        }
        let note = if total.is_none() {
            Some(self.power_note())
        } else {
            None
        };
        self.render_detail(
            Detail {
                icon: IconName::Zap,
                chart_title: t!("energy.chart").into_owned(),
                legend: vec![(t!("energy.measured_total").into_owned(), palette.accent)],
                badge: None,
                value: total
                    .map(format::watts)
                    .unwrap_or_else(|| t!("energy.not_measured").into_owned()),
                value_sub: t!(
                    "overview.energy_today",
                    energy = format::energy(account.today_wh),
                    cost = format::money(cost(account.today_wh, price), currency)
                )
                .into_owned(),
                axis: [format::watts(max), format::watts(max / 2.0), "0".into()],
                chart: self.chart(&SeriesKey::PowerTotal, max, DETAIL_CHART_HEIGHT, palette),
                note,
                list_title: t!("energy.by_part").into_owned(),
                list,
                list_columns: 1,
                top: None,
                stats,
            },
            palette,
        )
    }

    /// Why there is no power reading on this machine.
    pub fn power_note(&self) -> String {
        let package = self
            .monitor
            .capabilities
            .and_then(|capabilities| capabilities.package_power);
        if cfg!(windows) {
            t!("energy.note_windows").into_owned()
        } else if package == Some(false) {
            t!("energy.note_root").into_owned()
        } else {
            t!("energy.note_none").into_owned()
        }
    }
}

/// The axis labels of a rate chart from 0 to `max` bytes per second.
fn rate_axis(max: f32) -> [String; 3] {
    [
        format::rate(f64::from(max)),
        format::rate(f64::from(max) / 2.0),
        "0".into(),
    ]
}

/// The label of a volume: its mount point, with its device when they differ.
fn volume_label(volume: &latexo_core::Volume) -> String {
    let device = volume.name.trim_start_matches("/dev/");
    if device.is_empty() || volume.mount_point.trim_end_matches('\\').ends_with(':') {
        volume.mount_point.clone()
    } else {
        format!("{} · {device}", volume.mount_point)
    }
}
