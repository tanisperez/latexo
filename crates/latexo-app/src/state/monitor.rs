//! What the window shows, in plain Rust: the machine, the newest sample, the history of every
//! series, the energy account and the alerts that fired. The sampling task pushes one sample per
//! interval with [`Monitor::push`]; views only read.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use chrono::{Datelike, Local};
use latexo_core::alerts::{Fired, Rule, Watcher};
use latexo_core::energy::EnergyAccount;
use latexo_core::history::values_of;
use latexo_core::minutes::{self, Aggregator, MINUTE};
use latexo_core::{History, Sample, SystemInfo};
use latexo_sensors::Capabilities;

use super::settings::config_dir;

/// The samples kept: one hour at the shortest interval (0.5 s).
pub const HISTORY_CAPACITY: usize = 7200;
/// The most alerts kept in the history.
const ALERT_LOG_SIZE: usize = 50;
/// The energy account file, in the config folder.
const ENERGY_FILE: &str = "energy.toml";
/// How often the energy account is written, in samples.
const ENERGY_SAVE_EVERY: u64 = 60;
/// The minute history file, in the config folder.
const MINUTES_FILE: &str = "history.tsv";
/// How often the minute history is written, in minutes.
const MINUTES_SAVE_EVERY: u64 = 10;

/// An alert that fired.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertEvent {
    /// The rule, as it was when it fired.
    pub rule: Rule,
    /// The value that fired it.
    pub value: f32,
    /// When, local time.
    pub at: chrono::DateTime<Local>,
}

/// Everything the screens read.
#[derive(Debug, Default)]
pub struct Monitor {
    /// The machine, known after the sampler starts.
    pub info: Option<SystemInfo>,
    /// What this machine and user can read.
    pub capabilities: Option<Capabilities>,
    /// The newest sample.
    pub latest: Option<Arc<Sample>>,
    /// Every series.
    pub history: History,
    /// Watt hours today and this month.
    pub energy: EnergyAccount,
    /// The alerts that fired, newest first.
    pub alert_log: Vec<AlertEvent>,
    /// The average of every series over each minute, for the long ranges.
    pub minutes: History,
    aggregator: Aggregator,
    keep_minutes: usize,
    minutes_pushed: u64,
    watcher: Watcher,
    last_elapsed: Option<f64>,
    pushed: u64,
}

impl Monitor {
    /// An empty monitor, with the energy account and the minute history of the last runs. The
    /// minute history keeps `keep_minutes` minutes, none for 0.
    pub fn new(keep_minutes: usize) -> Self {
        Self {
            history: History::new(HISTORY_CAPACITY),
            energy: load_energy(),
            minutes: load_minutes(keep_minutes),
            keep_minutes,
            ..Self::default()
        }
    }

    /// Keeps `keep_minutes` minutes from now on: fewer drops the oldest, 0 forgets them all and
    /// deletes the file.
    pub fn set_keep(&mut self, keep_minutes: usize) {
        if keep_minutes == self.keep_minutes {
            return;
        }
        self.keep_minutes = keep_minutes;
        if keep_minutes == 0 {
            self.minutes = History::default();
            self.aggregator.clear();
            delete_minutes();
            return;
        }
        let since = unix_now() - keep_minutes as f64 * MINUTE;
        self.minutes = minutes::from_text(&minutes::to_text(&self.minutes), keep_minutes, since);
        self.save_minutes();
    }

    /// Adds a sample: the history, the energy account and the alert rules. Returns the alerts that
    /// fire now.
    pub fn push(&mut self, sample: Sample, rules: &[Rule]) -> Vec<AlertEvent> {
        let now = Local::now();
        let secs = self
            .last_elapsed
            .replace(sample.elapsed_secs)
            .map_or(0.0, |last| sample.elapsed_secs - last);
        if let Some(watts) = sample.power.measured_total() {
            let day = i64::from(now.date_naive().num_days_from_ce());
            let month = i64::from(now.year()) * 12 + i64::from(now.month0());
            self.energy.add(f64::from(watts), secs, day, month);
        }
        if self.keep_minutes > 0
            && let Some((time, values)) = self.aggregator.add(unix_now(), values_of(&sample))
        {
            minutes::fill_gap(&mut self.minutes, time);
            self.minutes.push_values(time, values);
            self.minutes_pushed += 1;
            if self.minutes_pushed.is_multiple_of(MINUTES_SAVE_EVERY) {
                self.save_minutes();
            }
        }
        self.history.push(&sample);
        let fired: Vec<AlertEvent> = self
            .watcher
            .check(rules, &sample)
            .into_iter()
            .filter_map(|Fired { rule, value }| {
                Some(AlertEvent {
                    rule: rules.get(rule)?.clone(),
                    value,
                    at: now,
                })
            })
            .collect();
        for event in fired.iter().rev() {
            self.alert_log.insert(0, event.clone());
        }
        self.alert_log.truncate(ALERT_LOG_SIZE);
        self.latest = Some(Arc::new(sample));
        self.pushed += 1;
        if self.pushed.is_multiple_of(ENERGY_SAVE_EVERY) {
            self.save_energy();
        }
        fired
    }

    /// Forgets the rule states, after the rules change.
    pub fn rules_changed(&mut self) {
        self.watcher.reset();
    }

    /// Forgets the history, the minute history and its file (Settings, Data).
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.minutes.clear();
        self.aggregator.clear();
        delete_minutes();
    }

    /// Writes the minute history, when it is kept.
    pub fn save_minutes(&self) {
        if self.keep_minutes == 0 || self.minutes.is_empty() {
            return;
        }
        if let Some(base) = config_dir() {
            let text = minutes::to_text(&self.minutes);
            if let Err(error) =
                fs::create_dir_all(&base).and_then(|()| fs::write(base.join(MINUTES_FILE), text))
            {
                log::warn!("could not save the minute history: {error}");
            }
        }
    }

    /// Writes the energy account.
    pub fn save_energy(&self) {
        if let Some(base) = config_dir() {
            save_energy_at(&base, &self.energy);
        }
    }
}

/// Unix time now, in seconds.
fn unix_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

fn load_minutes(keep_minutes: usize) -> History {
    if keep_minutes == 0 {
        return History::default();
    }
    let since = unix_now() - keep_minutes as f64 * MINUTE;
    let text = config_dir()
        .and_then(|base| fs::read_to_string(base.join(MINUTES_FILE)).ok())
        .unwrap_or_default();
    let mut history = minutes::from_text(&text, keep_minutes, since);
    // Up to the minute in progress, so the newest saved minute sits where it belongs.
    minutes::fill_gap(&mut history, (unix_now() / MINUTE).floor() * MINUTE);
    log::info!("minute history: {} minutes read", history.len());
    history
}

fn delete_minutes() {
    if let Some(base) = config_dir() {
        let _ = fs::remove_file(base.join(MINUTES_FILE));
    }
}

fn load_energy() -> EnergyAccount {
    config_dir()
        .and_then(|base| fs::read_to_string(base.join(ENERGY_FILE)).ok())
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_energy_at(base: &Path, energy: &EnergyAccount) {
    let Ok(text) = toml::to_string(energy) else {
        return;
    };
    if let Err(error) =
        fs::create_dir_all(base).and_then(|()| fs::write(base.join(ENERGY_FILE), text))
    {
        log::warn!("could not save the energy account: {error}");
    }
}

/// The history as CSV: one row per sample, one column per series, for Settings, Export.
pub fn history_csv(history: &History) -> String {
    let keys: Vec<_> = history_keys(history);
    let mut out = String::from("seconds");
    for key in &keys {
        out.push(',');
        out.push_str(&csv_name(key));
    }
    out.push('\n');
    for (index, time) in history.times().iter().enumerate() {
        out.push_str(&format!("{time:.1}"));
        for key in &keys {
            out.push(',');
            if let Some(value) = history
                .series(key)
                .and_then(|series| series.values().get(index))
                .filter(|value| value.is_finite())
            {
                out.push_str(&value.to_string());
            }
        }
        out.push('\n');
    }
    out
}

fn history_keys(history: &History) -> Vec<latexo_core::SeriesKey> {
    use latexo_core::SeriesKey;
    let mut keys = vec![
        SeriesKey::CpuUsage,
        SeriesKey::CpuUser,
        SeriesKey::CpuSystem,
        SeriesKey::CpuTemperature,
        SeriesKey::CpuFrequency,
        SeriesKey::MemoryUsed,
        SeriesKey::SwapUsed,
        SeriesKey::DiskRead,
        SeriesKey::DiskWrite,
        SeriesKey::NetReceived,
        SeriesKey::NetSent,
        SeriesKey::PowerTotal,
        SeriesKey::PowerCpu,
        SeriesKey::BatteryCharge,
    ];
    keys.retain(|key| history.series(key).is_some());
    keys
}

fn csv_name(key: &latexo_core::SeriesKey) -> String {
    use latexo_core::SeriesKey;
    match key {
        SeriesKey::CpuUsage => "cpu_usage".into(),
        SeriesKey::CpuUser => "cpu_user".into(),
        SeriesKey::CpuSystem => "cpu_system".into(),
        SeriesKey::CpuTemperature => "cpu_temperature_c".into(),
        SeriesKey::CpuFrequency => "cpu_frequency_mhz".into(),
        SeriesKey::MemoryUsed => "memory_used_bytes".into(),
        SeriesKey::SwapUsed => "swap_used_bytes".into(),
        SeriesKey::DiskRead => "disk_read_bytes_per_s".into(),
        SeriesKey::DiskWrite => "disk_write_bytes_per_s".into(),
        SeriesKey::NetReceived => "net_received_bytes_per_s".into(),
        SeriesKey::NetSent => "net_sent_bytes_per_s".into(),
        SeriesKey::PowerTotal => "power_total_w".into(),
        SeriesKey::PowerCpu => "power_cpu_w".into(),
        SeriesKey::BatteryCharge => "battery_charge".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn the_energy_account_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let account = EnergyAccount {
            day: 3,
            month: 4,
            today_wh: 12.5,
            month_wh: 100.0,
        };
        save_energy_at(dir.path(), &account);
        let text = fs::read_to_string(dir.path().join(ENERGY_FILE)).unwrap();
        assert_eq!(toml::from_str::<EnergyAccount>(&text).unwrap(), account);
    }

    #[test]
    fn the_csv_has_a_header_and_one_row_per_sample() {
        let mut history = History::new(10);
        let mut sample = Sample::default();
        sample.cpu.usage = 0.5;
        history.push(&sample);
        sample.elapsed_secs = 1.0;
        history.push(&sample);
        let csv = history_csv(&history);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("seconds,cpu_usage,"));
        assert!(lines[2].starts_with("1.0,0.5,"));
    }

    #[test]
    fn alerts_fire_into_the_log() {
        let mut monitor = Monitor {
            history: History::new(10),
            ..Monitor::default()
        };
        let rule = Rule {
            enabled: true,
            metric: latexo_core::alerts::Metric::CpuUsage,
            below: false,
            threshold: 50.0,
            duration_secs: 0,
            action: latexo_core::alerts::AlertAction::Notify,
        };
        let mut sample = Sample::default();
        sample.cpu.usage = 0.9;
        let fired = monitor.push(sample, &[rule]);
        assert_eq!(fired.len(), 1);
        assert_eq!(monitor.alert_log.len(), 1);
        assert!(monitor.latest.is_some());
    }
}
