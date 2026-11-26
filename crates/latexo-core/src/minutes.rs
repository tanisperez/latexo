//! The minute history: the average of every series over each minute, kept for a day or a week
//! and saved between runs, for the 6 hour, day and week ranges. Its times are Unix seconds (the
//! start of each minute), so a saved history lines up with a new run.
//!
//! [`Aggregator`] turns samples into minute averages, [`fill_gap`] marks the minutes Latexo was
//! not running, and [`to_text`] and [`from_text`] read and write the file: a header with the
//! series names ([`SeriesKey::code`]), then one line per minute.

use std::collections::BTreeMap;

use crate::history::{History, SeriesKey};

/// The length of a minute, in seconds.
pub const MINUTE: f64 = 60.0;

impl SeriesKey {
    /// A stable name for the file: `cpu_usage`, `core:3`, `sensor:coretemp Core 4`.
    pub fn code(&self) -> String {
        match self {
            SeriesKey::CpuUsage => "cpu_usage".into(),
            SeriesKey::CpuUser => "cpu_user".into(),
            SeriesKey::CpuSystem => "cpu_system".into(),
            SeriesKey::CpuTemperature => "cpu_temperature".into(),
            SeriesKey::CpuFrequency => "cpu_frequency".into(),
            SeriesKey::Core(index) => format!("core:{index}"),
            SeriesKey::MemoryUsed => "memory_used".into(),
            SeriesKey::SwapUsed => "swap_used".into(),
            SeriesKey::DiskRead => "disk_read".into(),
            SeriesKey::DiskWrite => "disk_write".into(),
            SeriesKey::NetReceived => "net_received".into(),
            SeriesKey::NetSent => "net_sent".into(),
            SeriesKey::GpuUsage(index) => format!("gpu_usage:{index}"),
            SeriesKey::GpuMemory(index) => format!("gpu_memory:{index}"),
            SeriesKey::GpuTemperature(index) => format!("gpu_temperature:{index}"),
            SeriesKey::GpuPower(index) => format!("gpu_power:{index}"),
            SeriesKey::Sensor(label) => format!("sensor:{}", clean(label)),
            SeriesKey::Fan(label) => format!("fan:{}", clean(label)),
            SeriesKey::PowerTotal => "power_total".into(),
            SeriesKey::PowerCpu => "power_cpu".into(),
            SeriesKey::BatteryCharge => "battery_charge".into(),
        }
    }

    /// The series named `code`, `None` for a name this version does not know.
    pub fn parse(code: &str) -> Option<Self> {
        let simple = match code {
            "cpu_usage" => Some(SeriesKey::CpuUsage),
            "cpu_user" => Some(SeriesKey::CpuUser),
            "cpu_system" => Some(SeriesKey::CpuSystem),
            "cpu_temperature" => Some(SeriesKey::CpuTemperature),
            "cpu_frequency" => Some(SeriesKey::CpuFrequency),
            "memory_used" => Some(SeriesKey::MemoryUsed),
            "swap_used" => Some(SeriesKey::SwapUsed),
            "disk_read" => Some(SeriesKey::DiskRead),
            "disk_write" => Some(SeriesKey::DiskWrite),
            "net_received" => Some(SeriesKey::NetReceived),
            "net_sent" => Some(SeriesKey::NetSent),
            "power_total" => Some(SeriesKey::PowerTotal),
            "power_cpu" => Some(SeriesKey::PowerCpu),
            "battery_charge" => Some(SeriesKey::BatteryCharge),
            _ => None,
        };
        if simple.is_some() {
            return simple;
        }
        let (kind, rest) = code.split_once(':')?;
        let index = || rest.parse::<usize>().ok();
        match kind {
            "core" => index().map(SeriesKey::Core),
            "gpu_usage" => index().map(SeriesKey::GpuUsage),
            "gpu_memory" => index().map(SeriesKey::GpuMemory),
            "gpu_temperature" => index().map(SeriesKey::GpuTemperature),
            "gpu_power" => index().map(SeriesKey::GpuPower),
            "sensor" => Some(SeriesKey::Sensor(rest.to_string())),
            "fan" => Some(SeriesKey::Fan(rest.to_string())),
            _ => None,
        }
    }
}

/// A label without the characters the file uses as separators.
fn clean(label: &str) -> String {
    label.replace(['\t', '\n', '\r'], " ")
}

/// Adds up the samples of the current minute and gives their averages when the minute ends.
#[derive(Debug, Clone, Default)]
pub struct Aggregator {
    minute: Option<i64>,
    sums: BTreeMap<SeriesKey, (f64, u32)>,
}

impl Aggregator {
    /// Adds the values read at Unix time `now`. Returns the averages of the previous minute,
    /// with the time of its start, once `now` falls in a new one.
    pub fn add(
        &mut self,
        now: f64,
        values: Vec<(SeriesKey, f32)>,
    ) -> Option<(f64, Vec<(SeriesKey, f32)>)> {
        let minute = (now / MINUTE).floor() as i64;
        let finished = match self.minute {
            Some(current) if current != minute => Some(self.take(current)),
            _ => None,
        };
        self.minute = Some(minute);
        for (key, value) in values {
            if value.is_finite() {
                let entry = self.sums.entry(key).or_insert((0.0, 0));
                entry.0 += f64::from(value);
                entry.1 += 1;
            }
        }
        finished
    }

    fn take(&mut self, minute: i64) -> (f64, Vec<(SeriesKey, f32)>) {
        let values = std::mem::take(&mut self.sums)
            .into_iter()
            .map(|(key, (sum, count))| (key, (sum / f64::from(count)) as f32))
            .collect();
        (minute as f64 * MINUTE, values)
    }

    /// Forgets the minute in progress.
    pub fn clear(&mut self) {
        self.minute = None;
        self.sums.clear();
    }
}

/// Pushes empty minutes into `history` up to (not including) the minute at `time`, so the
/// minutes Latexo was not running show as a gap. At most `history.capacity()` of them.
pub fn fill_gap(history: &mut History, time: f64) {
    let Some(last) = history.times().back().copied() else {
        return;
    };
    let missing = ((time - last) / MINUTE).round() as i64 - 1;
    let missing = missing.clamp(0, history.capacity() as i64) as usize;
    let start = (time - missing as f64 * MINUTE).max(last + MINUTE);
    for index in 0..missing {
        history.push_values(start + index as f64 * MINUTE, Vec::new());
    }
}

/// The history as text: a header with the series names, then one line per minute, the time
/// first. Values are separated by tabs; a gap is an empty field.
pub fn to_text(history: &History) -> String {
    let keys: Vec<&SeriesKey> = history.keys().collect();
    let mut out = String::from("time");
    for key in &keys {
        out.push('\t');
        out.push_str(&key.code());
    }
    out.push('\n');
    for (index, time) in history.times().iter().enumerate() {
        out.push_str(&format!("{time:.0}"));
        for key in &keys {
            out.push('\t');
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

/// Reads a history written by [`to_text`] into one of `capacity` minutes, dropping the minutes
/// before `since` (Unix seconds). Missing minutes become gaps; unknown series and broken lines
/// are skipped.
pub fn from_text(text: &str, capacity: usize, since: f64) -> History {
    let mut history = History::new(capacity);
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return history;
    };
    let keys: Vec<Option<SeriesKey>> = header.split('\t').skip(1).map(SeriesKey::parse).collect();
    for line in lines {
        let mut fields = line.split('\t');
        let Some(time) = fields.next().and_then(|time| time.parse::<f64>().ok()) else {
            continue;
        };
        if time < since || history.times().back().is_some_and(|last| time <= *last) {
            continue;
        }
        let values = keys
            .iter()
            .zip(fields)
            .filter_map(|(key, field)| Some((key.clone()?, field.parse::<f32>().ok()?)))
            .collect();
        fill_gap(&mut history, time);
        history.push_values(time, values);
    }
    history
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn every_key_parses_back_from_its_code() {
        for key in [
            SeriesKey::CpuUsage,
            SeriesKey::Core(7),
            SeriesKey::GpuUsage(1),
            SeriesKey::Sensor("coretemp Core 4".into()),
            SeriesKey::Fan("thinkpad fan1".into()),
            SeriesKey::BatteryCharge,
        ] {
            assert_eq!(SeriesKey::parse(&key.code()), Some(key));
        }
        assert_eq!(SeriesKey::parse("nope"), None);
        assert_eq!(SeriesKey::parse("core:x"), None);
    }

    #[test]
    fn the_aggregator_averages_each_minute() {
        let mut aggregator = Aggregator::default();
        assert_eq!(aggregator.add(60.0, vec![(SeriesKey::CpuUsage, 0.2)]), None);
        assert_eq!(aggregator.add(90.0, vec![(SeriesKey::CpuUsage, 0.4)]), None);
        let (time, values) = aggregator
            .add(125.0, vec![(SeriesKey::CpuUsage, 1.0)])
            .unwrap_or_default();
        assert_eq!(time, 60.0);
        assert_eq!(values.len(), 1);
        assert!((values[0].1 - 0.3).abs() < 1e-6);
    }

    #[test]
    fn gaps_become_empty_minutes() {
        let mut history = History::new(100);
        history.push_values(0.0, vec![(SeriesKey::CpuUsage, 0.5)]);
        fill_gap(&mut history, 300.0);
        history.push_values(300.0, vec![(SeriesKey::CpuUsage, 0.7)]);
        assert_eq!(history.times().len(), 6);
        assert_eq!(history.times()[1], 60.0);
        let values = history
            .series(&SeriesKey::CpuUsage)
            .map(|s| s.values().clone());
        let values = values.unwrap_or_default();
        assert!(values[2].is_nan());
        assert_eq!(values[5], 0.7);
    }

    #[test]
    fn the_text_round_trips_and_old_minutes_are_dropped() {
        let mut history = History::new(10);
        history.push_values(60.0, vec![(SeriesKey::CpuUsage, 0.5)]);
        history.push_values(120.0, vec![(SeriesKey::Sensor("a b".into()), 40.0)]);
        history.push_values(180.0, vec![(SeriesKey::CpuUsage, 0.25)]);
        let text = to_text(&history);
        let read = from_text(&text, 10, 0.0);
        assert_eq!(read.times(), history.times());
        assert_eq!(read.last(&SeriesKey::CpuUsage), Some(0.25));
        assert_eq!(
            read.max_in(&SeriesKey::Sensor("a b".into()), crate::Range::Week),
            Some(40.0)
        );
        let recent = from_text(&text, 10, 100.0);
        assert_eq!(recent.len(), 2);
        let gappy = from_text("time\tcpu_usage\n60\t0.5\n300\t0.5\n", 10, 0.0);
        assert_eq!(gappy.len(), 5);
        assert_eq!(from_text("", 10, 0.0).len(), 0);
    }
}
