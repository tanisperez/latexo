//! The Alerts screen: the rules, each with its switch, threshold and action, a New alert menu, and
//! the history of the alerts that fired in this session.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::alerts::{AlertAction, Metric, Rule};
use rust_i18n::t;
use telaio_ui::components::{
    IconButton, MenuEntries, NumberStepper, OpenOnClickExt, SegmentedControl, SegmentedItem,
    switch_halo,
};

use crate::state::format;
use crate::theme::Palette;
use crate::theme::metrics::{GAP, RULE_ROW_HEIGHT, TEXT_ROW, TEXT_SM};

use super::root::AppView;
use super::widgets::{card, card_title, empty_line, muted};

/// The name of a rule by its metric.
pub fn rule_name(metric: Metric) -> String {
    match metric {
        Metric::CpuTemperature => t!("alerts.metric_cpu_temperature"),
        Metric::GpuTemperature => t!("alerts.metric_gpu_temperature"),
        Metric::CpuUsage => t!("alerts.metric_cpu_usage"),
        Metric::MemoryUsage => t!("alerts.metric_memory_usage"),
        Metric::DiskUsage => t!("alerts.metric_disk_usage"),
        Metric::Power => t!("alerts.metric_power"),
        Metric::FanSpeed => t!("alerts.metric_fan"),
    }
    .into_owned()
}

/// The threshold a new rule of `metric` starts with, its step and its range.
fn threshold_scale(metric: Metric) -> (f32, f32, f32, f32) {
    match metric {
        Metric::CpuTemperature => (90.0, 1.0, 40.0, 110.0),
        Metric::GpuTemperature => (83.0, 1.0, 40.0, 110.0),
        Metric::CpuUsage | Metric::MemoryUsage | Metric::DiskUsage => (90.0, 1.0, 5.0, 100.0),
        Metric::Power => (100.0, 5.0, 5.0, 2000.0),
        Metric::FanSpeed => (0.0, 100.0, 0.0, 5000.0),
    }
}

/// The durations a rule can wait, in seconds.
const DURATIONS: [u32; 6] = [0, 10, 30, 60, 120, 300];

fn duration_label(secs: u32) -> String {
    match secs {
        0 => t!("alerts.at_once").into_owned(),
        secs if secs < 60 => format!("{secs} s"),
        secs => format!("{} min", secs / 60),
    }
}

impl AppView {
    /// `value` of `metric` written with its unit.
    pub fn metric_value(&self, metric: Metric, value: f32) -> String {
        match metric {
            Metric::CpuTemperature | Metric::GpuTemperature => {
                format::temperature(value, self.settings.temperature_unit.into())
            }
            Metric::CpuUsage | Metric::MemoryUsage | Metric::DiskUsage => {
                format::percent_value(f64::from(value), 0)
            }
            Metric::Power => format::watts(value),
            Metric::FanSpeed => format::rpm(value.max(0.0) as u32),
        }
    }

    /// The condition of a rule in words.
    fn condition(&self, rule: &Rule) -> String {
        let value = self.metric_value(rule.metric, rule.threshold);
        let key = if rule.below {
            "alerts.condition_below"
        } else {
            "alerts.condition_above"
        };
        let mut text = t!(key, value = value).into_owned();
        if rule.duration_secs > 0 {
            text.push_str(&t!(
                "alerts.condition_for",
                duration = duration_label(rule.duration_secs)
            ));
        }
        text
    }

    fn edit_rule(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Vec<Rule>),
    ) {
        if index <= self.settings.alerts.len() {
            edit(&mut self.settings.alerts);
            self.monitor.rules_changed();
            self.apply_settings(window, cx);
        }
    }

    pub(crate) fn render_alerts(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let add = Button::new("alerts-add")
            .primary()
            .icon(Icon::new(IconName::Plus))
            .label(t!("alerts.new").into_owned())
            .open_on_click()
            .dropdown_menu(move |menu, window, cx| {
                let mut entries = MenuEntries::new();
                for metric in Metric::ALL {
                    let weak = weak.clone();
                    entries = entries.handler(rule_name(metric), move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            let (threshold, _, _, _) = threshold_scale(metric);
                            let index = view.settings.alerts.len();
                            view.edit_rule(index, window, cx, |rules| {
                                rules.push(Rule {
                                    enabled: true,
                                    metric,
                                    below: metric == Metric::FanSpeed,
                                    threshold,
                                    duration_secs: 30,
                                    action: AlertAction::Notify,
                                })
                            });
                        });
                    });
                }
                entries.render(menu, window, cx)
            });

        let rules: Vec<AnyElement> = self
            .settings
            .alerts
            .iter()
            .enumerate()
            .map(|(index, rule)| self.rule_row(index, rule, palette, cx))
            .collect();
        let rules_body = if rules.is_empty() {
            empty_line(t!("alerts.no_rules"), palette).into_any_element()
        } else {
            v_flex().children(rules).into_any_element()
        };
        let log: Vec<AnyElement> = self
            .monitor
            .alert_log
            .iter()
            .enumerate()
            .map(|(index, event)| {
                h_flex()
                    .items_start()
                    .gap(px(10.0))
                    .py(px(10.0))
                    .when(index > 0, |row| {
                        row.border_t_1().border_color(palette.border)
                    })
                    .child(
                        div()
                            .mt(px(5.0))
                            .size(px(8.0))
                            .flex_none()
                            .rounded_full()
                            .bg(palette.warning),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(TEXT_ROW)).child(self.alert_text(event)))
                            .child(muted(event.at.format("%H:%M:%S").to_string(), palette)),
                    )
                    .into_any_element()
            })
            .collect();
        let log_body = if log.is_empty() {
            empty_line(t!("alerts.no_history"), palette).into_any_element()
        } else {
            v_flex().children(log).into_any_element()
        };
        h_flex()
            .items_start()
            .gap(px(GAP))
            .child(
                card(palette)
                    .flex_grow(6.0)
                    .flex_basis(relative(0.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .mb(px(6.0))
                            .child(card_title(t!("alerts.rules_title")).mb(px(0.0)))
                            .child(add),
                    )
                    .child(rules_body),
            )
            .child(
                card(palette)
                    .flex_grow(4.0)
                    .flex_basis(relative(0.0))
                    .child(card_title(t!("alerts.history_title")))
                    .child(log_body),
            )
            .into_any_element()
    }

    fn rule_row(
        &self,
        index: usize,
        rule: &Rule,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let toggle = switch_halo(
            SharedString::from(format!("rule-{index}-halo")),
            Switch::new(SharedString::from(format!("rule-{index}-switch")))
                .checked(rule.enabled)
                .on_click({
                    let weak = weak.clone();
                    move |checked, window, cx| {
                        let on = *checked;
                        let _ = weak.update(cx, |view, cx| {
                            view.edit_rule(index, window, cx, |rules| {
                                if let Some(rule) = rules.get_mut(index) {
                                    rule.enabled = on;
                                }
                            })
                        });
                    }
                }),
            palette,
        );
        let (_, step, min, max) = threshold_scale(rule.metric);
        let metric = rule.metric;
        let stepper = NumberStepper::new(
            SharedString::from(format!("rule-{index}-threshold")),
            rule.threshold,
        )
        .step(step)
        .range(min, max)
        .format({
            let unit = self.settings.temperature_unit.into();
            move |value| match metric {
                Metric::CpuTemperature | Metric::GpuTemperature => format::temperature(value, unit),
                Metric::CpuUsage | Metric::MemoryUsage | Metric::DiskUsage => {
                    format::percent_value(f64::from(value), 0)
                }
                Metric::Power => format::watts(value),
                Metric::FanSpeed => format::rpm(value as u32),
            }
        })
        .on_change({
            let weak = weak.clone();
            move |value, window, cx| {
                let _ = weak.update(cx, |view, cx| {
                    view.edit_rule(index, window, cx, |rules| {
                        if let Some(rule) = rules.get_mut(index) {
                            rule.threshold = value;
                        }
                    })
                });
            }
        });
        let mut duration =
            SegmentedControl::new(SharedString::from(format!("rule-{index}-duration")));
        for secs in DURATIONS {
            let weak = weak.clone();
            duration = duration.item(
                SegmentedItem::new(duration_label(secs))
                    .selected(rule.duration_secs == secs)
                    .on_click(move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.edit_rule(index, window, cx, |rules| {
                                if let Some(rule) = rules.get_mut(index) {
                                    rule.duration_secs = secs;
                                }
                            })
                        });
                    }),
            );
        }
        let notify = rule.action == AlertAction::Notify;
        let action = Button::new(SharedString::from(format!("rule-{index}-action")))
            .ghost()
            .small()
            .label(if notify {
                t!("alerts.action_notify").into_owned()
            } else {
                t!("alerts.action_log").into_owned()
            })
            .icon(Icon::new(if notify {
                IconName::Bell
            } else {
                IconName::BellOff
            }))
            .on_click({
                let weak = weak.clone();
                move |_, window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.edit_rule(index, window, cx, |rules| {
                            if let Some(rule) = rules.get_mut(index) {
                                rule.action = if rule.action == AlertAction::Notify {
                                    AlertAction::LogOnly
                                } else {
                                    AlertAction::Notify
                                };
                            }
                        })
                    });
                }
            });
        let remove = IconButton::new(
            SharedString::from(format!("rule-{index}-remove")),
            IconName::Close,
        )
        .tooltip(t!("alerts.remove").into_owned())
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |view, cx| {
                view.edit_rule(index, window, cx, |rules| {
                    if index < rules.len() {
                        rules.remove(index);
                    }
                })
            });
        });
        v_flex()
            .py(px(8.0))
            .gap(px(8.0))
            .min_h(px(RULE_ROW_HEIGHT))
            .when(index > 0, |row| {
                row.border_t_1().border_color(palette.border)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(toggle)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(rule_name(rule.metric)),
                            )
                            .child(
                                div()
                                    .text_size(px(TEXT_SM))
                                    .text_color(palette.fg_muted)
                                    .truncate()
                                    .child(self.condition(rule)),
                            ),
                    )
                    .child(action)
                    .child(remove),
            )
            .child(
                h_flex()
                    .ml(px(52.0))
                    .gap(px(12.0))
                    .items_center()
                    .flex_wrap()
                    .child(stepper)
                    .child(duration),
            )
            .into_any_element()
    }
}
