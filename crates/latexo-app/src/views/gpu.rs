//! The GPU and Fans screens: a card with a chart per measure, then the details.

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::{Sample, SeriesKey};
use rust_i18n::t;

use crate::state::format;
use crate::theme::Palette;
use crate::theme::metrics::{GAP, SMALL_CHART_HEIGHT, TEXT_AXIS, VALUE_TEXT_SIZE_SM};

use super::chart::Chart;
use super::root::AppView;
use super::widgets::{
    big_value, card, card_header, card_title, chart_well, empty_line, muted, name_rows, note,
    stat_rows,
};

/// One measure card: icon, title, scale, value, the line beside it and its chart.
struct Measure {
    icon: IconName,
    title: String,
    scale: String,
    value: String,
    sub: String,
    chart: Chart,
}

fn measure_card(measure: Measure, palette: &Palette) -> AnyElement {
    card(palette)
        .flex_1()
        .child(
            card_header(Some(measure.icon), measure.title, palette).child(
                div()
                    .text_size(px(TEXT_AXIS))
                    .text_color(palette.fg_muted)
                    .child(measure.scale),
            ),
        )
        .child(
            h_flex()
                .mt(px(8.0))
                .h(px(30.0))
                .items_baseline()
                .gap(px(10.0))
                .child(big_value(measure.value, VALUE_TEXT_SIZE_SM))
                .child(muted(measure.sub, palette)),
        )
        .child(div().mt(px(8.0)).child(chart_well(measure.chart, palette)))
        .into_any_element()
}

/// `cards` two (or `columns`) per row.
fn rows_of(cards: Vec<AnyElement>, columns: usize) -> impl IntoElement {
    let mut grid = v_flex().gap(px(GAP));
    let mut cards = cards.into_iter().peekable();
    while cards.peek().is_some() {
        let mut row = h_flex().gap(px(GAP)).items_stretch();
        for _ in 0..columns {
            row = row.child(div().flex_1().min_w_0().flex().children(cards.next()));
        }
        grid = grid.child(row);
    }
    grid
}

impl AppView {
    pub(crate) fn render_gpu(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        if sample.gpus.is_empty() {
            return card(palette)
                .child(empty_line(t!("gpu.none"), palette))
                .into_any_element();
        }
        let unit = self.settings.temperature_unit.into();
        let mut column = v_flex().gap(px(GAP));
        for (index, gpu) in sample.gpus.iter().enumerate() {
            let mut cards = vec![measure_card(
                Measure {
                    icon: IconName::Box,
                    title: t!("gpu.usage").into_owned(),
                    scale: format::percent(1.0, 0),
                    value: gpu
                        .usage
                        .map(|usage| format::percent(usage, 0))
                        .unwrap_or_else(|| t!("common.not_available").into_owned()),
                    sub: if sample.gpus.len() > 1 {
                        gpu.name.clone()
                    } else {
                        String::new()
                    },
                    chart: self.chart(
                        &SeriesKey::GpuUsage(index),
                        1.0,
                        SMALL_CHART_HEIGHT,
                        palette,
                    ),
                },
                palette,
            )];
            if let (Some(used), Some(total)) = (gpu.memory_used, gpu.memory_total)
                && total > 0
            {
                cards.push(measure_card(
                    Measure {
                        icon: IconName::MemoryStick,
                        title: t!("gpu.memory").into_owned(),
                        scale: format::bytes(total),
                        value: format::bytes(used),
                        sub: t!(
                            "gpu.memory_sub",
                            total = format::bytes(total),
                            percent = format::percent(used as f32 / total as f32, 0)
                        )
                        .into_owned(),
                        chart: self.chart(
                            &SeriesKey::GpuMemory(index),
                            total as f32,
                            SMALL_CHART_HEIGHT,
                            palette,
                        ),
                    },
                    palette,
                ));
            }
            if let Some(celsius) = gpu.temperature {
                cards.push(measure_card(
                    Measure {
                        icon: IconName::Thermometer,
                        title: t!("gpu.temperature").into_owned(),
                        scale: format::temperature(110.0, unit),
                        value: format::temperature(celsius, unit),
                        sub: String::new(),
                        chart: self
                            .chart(
                                &SeriesKey::GpuTemperature(index),
                                110.0,
                                SMALL_CHART_HEIGHT,
                                palette,
                            )
                            .threshold(83.0, palette.warning),
                    },
                    palette,
                ));
            }
            if let Some(watts) = gpu.power {
                let max = self.auto_max(&SeriesKey::GpuPower(index), 10.0);
                cards.push(measure_card(
                    Measure {
                        icon: IconName::Zap,
                        title: t!("gpu.power").into_owned(),
                        scale: format::watts(max),
                        value: format::watts(watts),
                        sub: String::new(),
                        chart: self.chart(
                            &SeriesKey::GpuPower(index),
                            max,
                            SMALL_CHART_HEIGHT,
                            palette,
                        ),
                    },
                    palette,
                ));
            }
            column = column.child(rows_of(cards, 2));
        }

        let gpu = &sample.gpus[0];
        let names = |pid: u32| {
            sample
                .processes
                .iter()
                .find(|process| process.pid == pid)
                .map(|process| process.name.clone())
                .unwrap_or_else(|| pid.to_string())
        };
        let processes: Vec<(String, String)> = gpu
            .processes
            .iter()
            .take(6)
            .map(|(pid, usage)| (names(*pid), format::percent(*usage, 0)))
            .collect();
        let mut stats = vec![(t!("gpu.model").into_owned(), gpu.name.clone())];
        if let Some(total) = gpu.memory_total {
            stats.push((t!("gpu.memory").into_owned(), format::bytes(total)));
        }
        if let Some(mhz) = gpu.frequency_mhz {
            stats.push((
                t!("gpu.clock").into_owned(),
                format::frequency(u64::from(mhz)),
            ));
        }
        let mut lower = h_flex().items_start().gap(px(GAP)).child(
            card(palette)
                .flex_1()
                .child(card_title(t!("gpu.processes")))
                .child(if processes.is_empty() {
                    empty_line(t!("common.nothing_found"), palette).into_any_element()
                } else {
                    name_rows(processes, palette).into_any_element()
                }),
        );
        lower = lower.child(
            card(palette)
                .flex_1()
                .child(card_title(t!("common.details")))
                .child(stat_rows(stats, palette)),
        );
        column = column.child(lower);
        if gpu.usage.is_none() {
            column = column.child(note(
                t!("gpu.no_usage_note"),
                palette.info,
                palette.info_subtle,
            ));
        }
        column.into_any_element()
    }

    pub(crate) fn render_fans(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        if sample.fans.is_empty() {
            return v_flex()
                .gap(px(GAP))
                .child(card(palette).child(empty_line(t!("fans.none"), palette)))
                .child(note(
                    if cfg!(windows) {
                        t!("fans.none_windows")
                    } else {
                        t!("fans.none_linux")
                    },
                    palette.info,
                    palette.info_subtle,
                ))
                .into_any_element();
        }
        let cards: Vec<AnyElement> = sample
            .fans
            .iter()
            .map(|fan| {
                let key = SeriesKey::Fan(fan.label.clone());
                let max = self.auto_max(&key, 1000.0);
                measure_card(
                    Measure {
                        icon: IconName::Fan,
                        title: super::temperatures::fan_label(&fan.label),
                        scale: format::rpm(max as u32),
                        value: format::integer(fan.rpm as usize),
                        sub: "rpm".to_string(),
                        chart: self.chart(&key, max, SMALL_CHART_HEIGHT, palette),
                    },
                    palette,
                )
            })
            .collect();
        v_flex()
            .gap(px(GAP))
            .child(rows_of(cards, 3))
            .child(note(
                t!("fans.read_only"),
                palette.info,
                palette.info_subtle,
            ))
            .into_any_element()
    }
}
