//! The Cores screen, in the four ways of the design: bars of usage and temperature per core, a
//! heat map of usage over time, a small chart per core and a map of tiles.

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::{CpuSample, Sample, SeriesKey};
use rust_i18n::t;

use crate::state::format;
use crate::state::settings::CoresView;
use crate::theme::metrics::{
    CORE_CHART_HEIGHT, CORE_ROW_HEIGHT, CORE_TILE_HEIGHT, GAP, HEAT_ROW_HEIGHT, RADIUS_MD,
    TEXT_AXIS, TEXT_ROW, TEXT_SM,
};
use crate::theme::{LatexoColors, Palette};

use super::chart::buckets;
use super::root::AppView;
use super::widgets::{
    badge, bar, big_value, card, card_header, card_title, legend_item, muted, palette_mono,
};
use telaio_ui::components::{SegmentedControl, SegmentedItem};

/// The opacity steps of the heat map, from idle to full.
const HEAT_LEVELS: [f32; 7] = [0.1, 0.22, 0.36, 0.52, 0.68, 0.84, 1.0];
/// Width of a heat map cell and its gap.
const HEAT_CELL: f32 = 10.0;
const HEAT_GAP: f32 = 2.0;

impl AppView {
    /// The cores view selector of the header.
    pub fn cores_view_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut control = SegmentedControl::new("cores-view");
        for view_kind in CoresView::ALL {
            let weak = cx.weak_entity();
            let label = match view_kind {
                CoresView::Bars => t!("cores.view_bars"),
                CoresView::Heatmap => t!("cores.view_heatmap"),
                CoresView::Charts => t!("cores.view_charts"),
                CoresView::Map => t!("cores.view_map"),
            };
            control = control.item(
                SegmentedItem::new(label.into_owned())
                    .selected(self.settings.cores_view == view_kind)
                    .on_click(move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.change(window, cx, |settings| settings.cores_view = view_kind)
                        });
                    }),
            );
        }
        control
    }

    pub(crate) fn render_cores(&self, sample: &Sample, palette: &Palette) -> AnyElement {
        match self.settings.cores_view {
            CoresView::Bars => self.cores_bars(&sample.cpu, palette),
            CoresView::Heatmap => self.cores_heatmap(&sample.cpu, palette),
            CoresView::Charts => self.cores_charts(&sample.cpu, palette),
            CoresView::Map => self.cores_map(&sample.cpu, palette),
        }
    }

    fn temperature_text(&self, celsius: Option<f32>) -> String {
        celsius
            .map(|t| format::temperature(t, self.settings.temperature_unit.into()))
            .unwrap_or_else(|| t!("common.dash").into_owned())
    }

    fn cores_bars(&self, cpu: &CpuSample, palette: &Palette) -> AnyElement {
        let has_temps = cpu.cores.iter().any(|core| core.temperature.is_some());
        let header = h_flex()
            .h(px(CORE_ROW_HEIGHT))
            .items_center()
            .gap(px(14.0))
            .text_size(px(TEXT_SM))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.fg_muted)
            .child(
                div()
                    .w(px(76.0))
                    .child(t!("cores.core_column").into_owned()),
            )
            .child(div().flex_1().child(t!("cores.usage").into_owned()))
            .child(div().w(px(48.0)))
            .when(has_temps, |row| {
                row.child(div().flex_1().child(t!("cores.temperature").into_owned()))
                    .child(div().w(px(56.0)))
            })
            .child(
                div()
                    .w(px(76.0))
                    .text_right()
                    .child(t!("cores.clock").into_owned()),
            );
        let rows = cpu.cores.iter().enumerate().map(|(index, core)| {
            let mono = |text: String| {
                div()
                    .text_right()
                    .font_family(palette_mono())
                    .text_size(px(TEXT_SM))
                    .child(text)
            };
            h_flex()
                .h(px(CORE_ROW_HEIGHT))
                .items_center()
                .gap(px(14.0))
                .border_t_1()
                .border_color(palette.border)
                .text_size(px(TEXT_ROW))
                .child(
                    div()
                        .w(px(76.0))
                        .child(t!("cores.core", number = index + 1).into_owned()),
                )
                .child(
                    div()
                        .flex_1()
                        .child(bar(core.usage, palette.accent, palette)),
                )
                .child(mono(format::percent(core.usage, 0)).w(px(48.0)))
                .when(has_temps, |row| {
                    let celsius = core.temperature.unwrap_or(0.0);
                    row.child(div().flex_1().child(bar(
                        celsius / 110.0,
                        palette.temperature(celsius),
                        palette,
                    )))
                    .child(mono(self.temperature_text(core.temperature)).w(px(56.0)))
                })
                .child(
                    mono(if core.frequency_mhz > 0 {
                        format::frequency(core.frequency_mhz)
                    } else {
                        t!("common.dash").into_owned()
                    })
                    .w(px(76.0))
                    .text_color(palette.fg_muted),
                )
        });
        card(palette)
            .child(
                card_header(
                    Some(super::root::screen_icon(crate::state::nav::Screen::Cores)),
                    t!("cores.bars_title"),
                    palette,
                )
                .mb(px(8.0))
                .when(has_temps, |header| {
                    header.child(legend_item(t!("cores.hot"), palette.warning, palette))
                }),
            )
            .child(header)
            .children(rows)
            .into_any_element()
    }

    fn cores_heatmap(&self, cpu: &CpuSample, palette: &Palette) -> AnyElement {
        let rows: Vec<Vec<f32>> = (0..cpu.cores.len())
            .map(|index| self.window(&SeriesKey::Core(index)))
            .collect();
        let slots = self.slots();
        let accent = palette.accent;
        let off = palette.dot_off();
        let height = rows.len() as f32 * HEAT_ROW_HEIGHT;
        let heat = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let width = f32::from(bounds.size.width);
                let columns = ((width + HEAT_GAP) / (HEAT_CELL + HEAT_GAP)) as usize;
                if columns == 0 {
                    return;
                }
                let right = bounds.origin.x + bounds.size.width;
                for (row, values) in rows.iter().enumerate() {
                    let points = buckets(values, slots.max(columns), columns);
                    let y = bounds.origin.y + px(row as f32 * HEAT_ROW_HEIGHT + 1.0);
                    for column in 0..columns {
                        let x = right - px((column + 1) as f32 * (HEAT_CELL + HEAT_GAP) - HEAT_GAP);
                        let value = points
                            .len()
                            .checked_sub(column + 1)
                            .map(|i| points[i].1)
                            .filter(|v| v.is_finite());
                        let color = match value {
                            Some(value) => {
                                let level = ((value.clamp(0.0, 1.0) * 7.0) as usize).min(6);
                                accent.opacity(HEAT_LEVELS[level])
                            }
                            None => off,
                        };
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x, y),
                                size(px(HEAT_CELL), px(HEAT_ROW_HEIGHT - 2.0)),
                            ),
                            color,
                        ));
                    }
                }
            },
        )
        .flex_1()
        .h(px(height));
        let labels = v_flex()
            .w(px(40.0))
            .flex_none()
            .children((0..cpu.cores.len()).map(|index| {
                div()
                    .h(px(HEAT_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .font_family(palette_mono())
                    .text_size(px(11.0))
                    .text_color(palette.fg_muted)
                    .child(t!("cores.short", number = index + 1).into_owned())
            }));
        let temps = cpu.cores.iter().any(|core| core.temperature.is_some());
        let temp_column = v_flex()
            .w(px(150.0))
            .flex_none()
            .children(cpu.cores.iter().map(|core| {
                let celsius = core.temperature.unwrap_or(0.0);
                h_flex()
                    .h(px(HEAT_ROW_HEIGHT))
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1().child(bar(
                        celsius / 110.0,
                        palette.temperature(celsius),
                        palette,
                    )))
                    .child(
                        div()
                            .w(px(44.0))
                            .text_right()
                            .font_family(palette_mono())
                            .text_size(px(11.0))
                            .child(self.temperature_text(core.temperature)),
                    )
            }));
        let scale = h_flex()
            .items_center()
            .gap(px(4.0))
            .child(muted(format::percent(0.0, 0), palette).mr(px(4.0)))
            .children(HEAT_LEVELS.map(|level| {
                div()
                    .w(px(16.0))
                    .h(px(8.0))
                    .rounded(px(2.0))
                    .bg(palette.accent.opacity(level))
            }))
            .child(muted(format::percent(1.0, 0), palette).ml(px(4.0)));
        let busiest = cpu.busiest_core();
        let hottest = cpu.hottest_core();
        let coolest = cpu
            .cores
            .iter()
            .filter_map(|core| core.temperature)
            .min_by(f32::total_cmp);
        let mut stats = vec![(
            t!("cores.busiest").into_owned(),
            busiest
                .map(|(index, usage)| {
                    format!(
                        "{} · {}",
                        t!("cores.core", number = index + 1),
                        format::percent(usage, 0)
                    )
                })
                .unwrap_or_default(),
        )];
        if let Some((index, celsius)) = hottest {
            stats.push((
                t!("cores.hottest").into_owned(),
                format!(
                    "{} · {}",
                    t!("cores.core", number = index + 1),
                    self.temperature_text(Some(celsius))
                ),
            ));
            if let Some(coolest) = coolest {
                stats.push((
                    t!("cores.spread").into_owned(),
                    format!(
                        "{} {}",
                        format::decimal(f64::from(celsius - coolest), 0),
                        latexo_core::units::TemperatureUnit::from(self.settings.temperature_unit)
                            .symbol()
                    ),
                ));
            }
        }
        v_flex()
            .gap(px(GAP))
            .child(
                card(palette)
                    .child(
                        card_header(
                            Some(super::root::screen_icon(crate::state::nav::Screen::Cores)),
                            t!("cores.heatmap_title"),
                            palette,
                        )
                        .mb(px(12.0))
                        .child(scale),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .gap(px(12.0))
                            .child(labels)
                            .child(heat)
                            .when(temps, |row| row.child(temp_column)),
                    )
                    .child(
                        h_flex()
                            .mt(px(6.0))
                            .ml(px(52.0))
                            .when(temps, |row| row.mr(px(162.0)))
                            .justify_between()
                            .text_size(px(TEXT_AXIS))
                            .text_color(palette.fg_muted)
                            .child(format!("-{}", super::root::range_label(self.range())))
                            .child(t!("charts.now").into_owned()),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(GAP))
                    .children(stats.into_iter().map(|(key, value)| {
                        card(palette)
                            .flex_1()
                            .py(px(12.0))
                            .child(muted(key, palette))
                            .child(div().mt(px(2.0)).child(big_value(value, 18.0)))
                    })),
            )
            .into_any_element()
    }

    fn cores_charts(&self, cpu: &CpuSample, palette: &Palette) -> AnyElement {
        let tiles: Vec<AnyElement> = cpu
            .cores
            .iter()
            .enumerate()
            .map(|(index, core)| {
                let celsius = core.temperature;
                let (fg, bg) = match celsius {
                    Some(t) if t >= 95.0 => (palette.danger, palette.danger_subtle),
                    Some(t) if t >= 80.0 => (palette.warning, palette.warning_subtle),
                    _ => (palette.fg_muted, palette.hover),
                };
                card(palette)
                    .p(px(12.0))
                    .child(
                        h_flex()
                            .h(px(22.0))
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(TEXT_ROW))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t!("cores.core", number = index + 1).into_owned()),
                            )
                            .when(celsius.is_some(), |row| {
                                row.child(badge(self.temperature_text(celsius), fg, bg))
                            }),
                    )
                    .child(
                        h_flex()
                            .mt(px(4.0))
                            .items_baseline()
                            .gap(px(8.0))
                            .child(big_value(format::percent(core.usage, 0), 18.0))
                            .child(
                                div()
                                    .font_family(palette_mono())
                                    .text_size(px(TEXT_SM))
                                    .text_color(palette.fg_muted)
                                    .child(if core.frequency_mhz > 0 {
                                        format::frequency(core.frequency_mhz)
                                    } else {
                                        String::new()
                                    }),
                            ),
                    )
                    .child(div().mt(px(6.0)).child(self.chart(
                        &SeriesKey::Core(index),
                        1.0,
                        CORE_CHART_HEIGHT,
                        palette,
                    )))
                    .into_any_element()
            })
            .collect();
        grid_of(tiles, 4)
    }

    fn cores_map(&self, cpu: &CpuSample, palette: &Palette) -> AnyElement {
        let tiles: Vec<AnyElement> = cpu
            .cores
            .iter()
            .enumerate()
            .map(|(index, core)| {
                let mix = 0.08 + core.usage.clamp(0.0, 1.0) * 0.72;
                let background = palette.surface.blend(palette.accent.opacity(mix));
                let light_text = !palette.dark && mix > 0.6;
                let hot = core.temperature.is_some_and(|t| t >= 80.0);
                div()
                    .h(px(CORE_TILE_HEIGHT))
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(RADIUS_MD))
                    .bg(background)
                    .text_color(if light_text {
                        palette.accent_fg
                    } else {
                        palette.fg
                    })
                    .border_color(if hot { palette.warning } else { palette.border })
                    .when(hot, |tile| tile.border_2())
                    .when(!hot, |tile| tile.border_1())
                    .child(
                        h_flex()
                            .justify_between()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("cores.core", number = index + 1).into_owned())
                            .child(
                                div()
                                    .font_family(palette_mono())
                                    .child(self.temperature_text(core.temperature)),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(6.0))
                            .child(big_value(format::percent(core.usage, 0), 20.0)),
                    )
                    .child(
                        div()
                            .mt(px(2.0))
                            .font_family(palette_mono())
                            .text_size(px(11.0))
                            .opacity(0.85)
                            .child(if core.frequency_mhz > 0 {
                                format::frequency(core.frequency_mhz)
                            } else {
                                String::new()
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        let columns = if cpu.cores.len() > 16 { 8 } else { 4 };
        let temps: Vec<(usize, f32)> = cpu
            .cores
            .iter()
            .enumerate()
            .filter_map(|(index, core)| core.temperature.map(|t| (index, t)))
            .collect();
        let mut column = v_flex().gap(px(GAP)).child(
            card(palette)
                .child(card_title(t!("cores.map_title")).mb(px(12.0)))
                .child(grid_of(tiles, columns)),
        );
        if !temps.is_empty() {
            let bars = h_flex()
                .items_end()
                .gap(px(8.0))
                .h(px(130.0))
                .children(temps.iter().map(|(index, celsius)| {
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .justify_end()
                        .gap(px(4.0))
                        .h_full()
                        .child(
                            div()
                                .font_family(palette_mono())
                                .text_size(px(11.0))
                                .text_color(palette.fg_muted)
                                .child(format::decimal(
                                    f64::from(
                                        latexo_core::units::TemperatureUnit::from(
                                            self.settings.temperature_unit,
                                        )
                                        .convert(*celsius),
                                    )
                                    .round(),
                                    0,
                                )),
                        )
                        .child(
                            div()
                                .w_full()
                                .max_w(px(36.0))
                                .h(px((celsius / 110.0).clamp(0.02, 1.0) * 90.0))
                                .rounded_t(px(4.0))
                                .bg(palette.temperature(*celsius)),
                        )
                        .child(
                            div()
                                .font_family(palette_mono())
                                .text_size(px(11.0))
                                .text_color(palette.fg_muted)
                                .child(t!("cores.short", number = index + 1).into_owned()),
                        )
                }));
            column = column.child(
                card(palette)
                    .child(
                        card_header(None, t!("cores.temperature_title"), palette)
                            .mb(px(12.0))
                            .child(legend_item(t!("cores.hot"), palette.warning, palette)),
                    )
                    .child(bars),
            );
        }
        column.into_any_element()
    }
}

/// `items` in rows of `columns`, each the same width.
fn grid_of(items: Vec<AnyElement>, columns: usize) -> AnyElement {
    let mut grid = v_flex().gap(px(12.0));
    let mut items = items.into_iter().peekable();
    while items.peek().is_some() {
        let mut row = h_flex().gap(px(12.0)).items_stretch();
        for _ in 0..columns {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .children(items.next()),
            );
        }
        grid = grid.child(row);
    }
    grid.into_any_element()
}
