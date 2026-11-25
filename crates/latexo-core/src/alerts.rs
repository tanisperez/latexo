//! Alert rules: a metric over (or under) a threshold for a while fires once, and fires again only
//! after the condition has cleared. [`Watcher`] keeps that state between samples.

use serde::{Deserialize, Serialize};

use crate::sample::Sample;

/// What a rule watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// The processor temperature, °C.
    CpuTemperature,
    /// The hottest graphics card, °C.
    GpuTemperature,
    /// CPU usage, percent.
    CpuUsage,
    /// Memory usage, percent.
    MemoryUsage,
    /// The fullest volume, percent used.
    DiskUsage,
    /// The measured power of the machine, W.
    Power,
    /// The slowest fan, rpm (a stopped fan reads 0).
    FanSpeed,
}

impl Metric {
    /// Every metric, in the order of the rule editor.
    pub const ALL: [Metric; 7] = [
        Metric::CpuTemperature,
        Metric::GpuTemperature,
        Metric::CpuUsage,
        Metric::MemoryUsage,
        Metric::DiskUsage,
        Metric::Power,
        Metric::FanSpeed,
    ];

    /// The current value in `sample`, `None` when the machine does not report it.
    pub fn value(self, sample: &Sample) -> Option<f32> {
        match self {
            Metric::CpuTemperature => sample.cpu_temperature(),
            Metric::GpuTemperature => sample
                .gpus
                .iter()
                .filter_map(|gpu| gpu.temperature)
                .max_by(f32::total_cmp),
            Metric::CpuUsage => Some(sample.cpu.usage * 100.0),
            Metric::MemoryUsage => Some(sample.memory.usage() * 100.0),
            Metric::DiskUsage => sample
                .volumes
                .iter()
                .map(|volume| volume.usage() * 100.0)
                .max_by(f32::total_cmp),
            Metric::Power => sample.power.measured_total(),
            Metric::FanSpeed => sample
                .fans
                .iter()
                .map(|fan| fan.rpm as f32)
                .min_by(f32::total_cmp),
        }
    }
}

/// What happens when a rule fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertAction {
    /// A notification in the window and in the history.
    #[default]
    Notify,
    /// Only a line in the history.
    LogOnly,
}

/// One rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// Whether it is watched.
    pub enabled: bool,
    /// The metric.
    pub metric: Metric,
    /// Fires below the threshold instead of above it.
    #[serde(default)]
    pub below: bool,
    /// The threshold, in the metric's unit (°C, percent, W, rpm).
    pub threshold: f32,
    /// How long the condition must hold, in seconds.
    pub duration_secs: u32,
    /// What happens when it fires.
    #[serde(default)]
    pub action: AlertAction,
}

impl Rule {
    /// Whether `value` meets the condition.
    pub fn holds(&self, value: f32) -> bool {
        if self.below {
            value <= self.threshold
        } else {
            value >= self.threshold
        }
    }
}

/// The rules of a new install, from the design.
pub fn default_rules() -> Vec<Rule> {
    let rule = |metric, below, threshold, duration_secs, action| Rule {
        enabled: true,
        metric,
        below,
        threshold,
        duration_secs,
        action,
    };
    vec![
        rule(Metric::CpuTemperature, false, 90.0, 30, AlertAction::Notify),
        rule(Metric::GpuTemperature, false, 83.0, 60, AlertAction::Notify),
        rule(Metric::MemoryUsage, false, 90.0, 120, AlertAction::Notify),
        rule(Metric::DiskUsage, false, 90.0, 0, AlertAction::Notify),
        Rule {
            enabled: false,
            ..rule(Metric::Power, false, 300.0, 60, AlertAction::LogOnly)
        },
    ]
}

/// A rule that fired.
#[derive(Debug, Clone, PartialEq)]
pub struct Fired {
    /// The index of the rule.
    pub rule: usize,
    /// The value that fired it.
    pub value: f32,
}

/// The state of every rule between samples: since when each condition holds and whether it
/// already fired.
#[derive(Debug, Clone, Default)]
pub struct Watcher {
    states: Vec<RuleState>,
}

#[derive(Debug, Clone, Copy, Default)]
struct RuleState {
    since: Option<f64>,
    fired: bool,
}

impl Watcher {
    /// Checks `rules` against `sample` and returns the ones that fire now.
    pub fn check(&mut self, rules: &[Rule], sample: &Sample) -> Vec<Fired> {
        self.states.resize(rules.len(), RuleState::default());
        let now = sample.elapsed_secs;
        let mut fired = Vec::new();
        for (index, (rule, state)) in rules.iter().zip(self.states.iter_mut()).enumerate() {
            let value = rule
                .enabled
                .then(|| rule.metric.value(sample))
                .flatten()
                .filter(|value| rule.holds(*value));
            let Some(value) = value else {
                *state = RuleState::default();
                continue;
            };
            let since = *state.since.get_or_insert(now);
            if !state.fired && now - since >= f64::from(rule.duration_secs) {
                state.fired = true;
                fired.push(Fired { rule: index, value });
            }
        }
        fired
    }

    /// Forgets every state, after the rules change.
    pub fn reset(&mut self) {
        self.states.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn at(secs: f64, cpu: f32) -> Sample {
        let mut sample = Sample {
            elapsed_secs: secs,
            ..Sample::default()
        };
        sample.cpu.usage = cpu;
        sample
    }

    fn cpu_rule() -> Rule {
        Rule {
            enabled: true,
            metric: Metric::CpuUsage,
            below: false,
            threshold: 80.0,
            duration_secs: 2,
            action: AlertAction::Notify,
        }
    }

    #[test]
    fn fires_once_after_the_duration_and_rearms_when_cleared() {
        let rules = [cpu_rule()];
        let mut watcher = Watcher::default();
        assert!(watcher.check(&rules, &at(0.0, 0.9)).is_empty());
        assert!(watcher.check(&rules, &at(1.0, 0.9)).is_empty());
        assert_eq!(watcher.check(&rules, &at(2.0, 0.95)).len(), 1);
        assert!(watcher.check(&rules, &at(3.0, 0.95)).is_empty());
        assert!(watcher.check(&rules, &at(4.0, 0.1)).is_empty());
        assert!(watcher.check(&rules, &at(5.0, 0.9)).is_empty());
        assert_eq!(watcher.check(&rules, &at(7.0, 0.9)).len(), 1);
    }

    #[test]
    fn a_disabled_rule_or_a_missing_metric_never_fires() {
        let mut rule = cpu_rule();
        rule.enabled = false;
        rule.duration_secs = 0;
        let mut watcher = Watcher::default();
        assert!(watcher.check(&[rule], &at(0.0, 1.0)).is_empty());
        let gpu = Rule {
            metric: Metric::GpuTemperature,
            duration_secs: 0,
            ..cpu_rule()
        };
        assert!(watcher.check(&[gpu], &at(0.0, 1.0)).is_empty());
    }

    #[test]
    fn below_rules_fire_under_the_threshold() {
        let rule = Rule {
            below: true,
            threshold: 10.0,
            duration_secs: 0,
            ..cpu_rule()
        };
        let mut watcher = Watcher::default();
        assert_eq!(watcher.check(&[rule], &at(0.0, 0.05)).len(), 1);
    }
}
