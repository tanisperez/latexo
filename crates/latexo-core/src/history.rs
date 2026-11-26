//! The history of every series the charts draw, kept in memory as rings of `f32` aligned on one
//! ring of times: entry `i` of every series was read at `times[i]`. A series that appears later
//! (a USB disk's sensor) is padded with `NaN` before its first value, and a sample that misses it
//! adds a `NaN`, which the charts draw as a gap.
//!
//! The history holds at most [`History::capacity`] samples, enough for the longest range at the
//! shortest interval.

use std::collections::{BTreeMap, VecDeque};

use crate::sample::Sample;

/// One series.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeriesKey {
    /// CPU usage, 0 to 1.
    CpuUsage,
    /// CPU user share, 0 to 1.
    CpuUser,
    /// CPU kernel share, 0 to 1.
    CpuSystem,
    /// The processor temperature to show first, °C.
    CpuTemperature,
    /// Average clock, MHz.
    CpuFrequency,
    /// One logical processor's usage, 0 to 1.
    Core(usize),
    /// Memory in use, bytes.
    MemoryUsed,
    /// Swap in use, bytes.
    SwapUsed,
    /// Disk reads, bytes per second.
    DiskRead,
    /// Disk writes, bytes per second.
    DiskWrite,
    /// Network received, bytes per second.
    NetReceived,
    /// Network sent, bytes per second.
    NetSent,
    /// One graphics card's usage, 0 to 1.
    GpuUsage(usize),
    /// One graphics card's memory, bytes.
    GpuMemory(usize),
    /// One graphics card's temperature, °C.
    GpuTemperature(usize),
    /// One graphics card's power, W.
    GpuPower(usize),
    /// A temperature sensor by label, °C.
    Sensor(String),
    /// A fan by label, rpm.
    Fan(String),
    /// The measured power of the whole machine, W.
    PowerTotal,
    /// The processor package power, W.
    PowerCpu,
    /// The battery charge, 0 to 1.
    BatteryCharge,
}

/// The values of one series, oldest first, aligned with [`History::times`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Series {
    values: VecDeque<f32>,
}

impl Series {
    /// Every value, oldest first.
    pub fn values(&self) -> &VecDeque<f32> {
        &self.values
    }

    /// The newest value, `None` when it is missing.
    pub fn last(&self) -> Option<f32> {
        self.values.back().copied().filter(|v| v.is_finite())
    }
}

/// The time ranges the charts offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord)]
pub enum Range {
    /// The last minute.
    OneMinute,
    /// The last five minutes, the default.
    #[default]
    FiveMinutes,
    /// The last fifteen minutes.
    FifteenMinutes,
    /// The last hour.
    OneHour,
    /// The last six hours, from the minute history.
    SixHours,
    /// The last day, from the minute history.
    Day,
    /// The last week, from the minute history.
    Week,
}

impl Range {
    /// Every range, shortest first.
    pub const ALL: [Range; 7] = [
        Range::OneMinute,
        Range::FiveMinutes,
        Range::FifteenMinutes,
        Range::OneHour,
        Range::SixHours,
        Range::Day,
        Range::Week,
    ];

    /// Whether the range reads the minute history instead of the samples of the last hour.
    pub fn is_long(self) -> bool {
        self.secs() > 3600.0
    }

    /// The length in seconds.
    pub fn secs(self) -> f64 {
        match self {
            Range::OneMinute => 60.0,
            Range::FiveMinutes => 300.0,
            Range::FifteenMinutes => 900.0,
            Range::OneHour => 3600.0,
            Range::SixHours => 6.0 * 3600.0,
            Range::Day => 24.0 * 3600.0,
            Range::Week => 7.0 * 24.0 * 3600.0,
        }
    }
}

/// Every series, aligned on one ring of times.
#[derive(Debug, Clone, Default)]
pub struct History {
    capacity: usize,
    times: VecDeque<f64>,
    series: BTreeMap<SeriesKey, Series>,
    generation: u64,
}

impl History {
    /// An empty history of at most `capacity` samples.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(2),
            ..Self::default()
        }
    }

    /// The most samples kept.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of samples kept.
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Whether no sample was pushed yet.
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Grows with every push, a cheap key for caches of derived data.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// When each sample was taken, in seconds since sampling started, oldest first.
    pub fn times(&self) -> &VecDeque<f64> {
        &self.times
    }

    /// Every series kept, in a stable order.
    pub fn keys(&self) -> impl Iterator<Item = &SeriesKey> {
        self.series.keys()
    }

    /// One series, `None` when it was never seen.
    pub fn series(&self, key: &SeriesKey) -> Option<&Series> {
        self.series.get(key)
    }

    /// The newest value of a series.
    pub fn last(&self, key: &SeriesKey) -> Option<f32> {
        self.series(key).and_then(Series::last)
    }

    /// The values of a series in the last `range`, oldest first, `NaN` for gaps.
    pub fn window(&self, key: &SeriesKey, range: Range) -> Vec<f32> {
        let Some(series) = self.series(key) else {
            return Vec::new();
        };
        let start = self.window_start(range);
        series.values.iter().skip(start).copied().collect()
    }

    /// The highest value of a series in the last `range`.
    pub fn max_in(&self, key: &SeriesKey, range: Range) -> Option<f32> {
        self.window(key, range)
            .into_iter()
            .filter(|v| v.is_finite())
            .max_by(f32::total_cmp)
    }

    /// The index of the first sample inside the last `range`.
    fn window_start(&self, range: Range) -> usize {
        let Some(newest) = self.times.back() else {
            return 0;
        };
        let since = newest - range.secs();
        self.times.partition_point(|time| *time < since)
    }

    /// Adds one value per series read from `sample`, taken at `sample.elapsed_secs`.
    pub fn push(&mut self, sample: &Sample) {
        self.push_values(sample.elapsed_secs, values_of(sample));
    }

    /// Adds `values` taken at `time`; series left out get a gap.
    pub fn push_values(&mut self, time: f64, values: Vec<(SeriesKey, f32)>) {
        if self.times.len() == self.capacity {
            self.times.pop_front();
            for series in self.series.values_mut() {
                series.values.pop_front();
            }
        }
        let padding = self.times.len();
        self.times.push_back(time);
        for (key, value) in values {
            let series = self.series.entry(key).or_insert_with(|| Series {
                values: std::iter::repeat_n(f32::NAN, padding).collect(),
            });
            series.values.push_back(value);
        }
        let len = self.times.len();
        for series in self.series.values_mut() {
            if series.values.len() < len {
                series.values.push_back(f32::NAN);
            }
        }
        // A series missing for the whole window is dropped (an unplugged disk).
        self.series
            .retain(|_, series| series.values.iter().any(|v| v.is_finite()));
        self.generation += 1;
    }

    /// Forgets every sample.
    pub fn clear(&mut self) {
        self.times.clear();
        self.series.clear();
        self.generation += 1;
    }
}

/// The values of one sample, one per series.
pub fn values_of(sample: &Sample) -> Vec<(SeriesKey, f32)> {
    let mut values = vec![
        (SeriesKey::CpuUsage, sample.cpu.usage),
        (SeriesKey::MemoryUsed, sample.memory.used as f32),
        (SeriesKey::SwapUsed, sample.memory.swap_used as f32),
        (SeriesKey::DiskRead, sample.disk_io.read_per_sec as f32),
        (SeriesKey::DiskWrite, sample.disk_io.write_per_sec as f32),
        (
            SeriesKey::NetReceived,
            sample.network.received_per_sec as f32,
        ),
        (SeriesKey::NetSent, sample.network.sent_per_sec as f32),
    ];
    let mut optional = |key, value: Option<f32>| {
        if let Some(value) = value {
            values.push((key, value));
        }
    };
    optional(SeriesKey::CpuUser, sample.cpu.user);
    optional(SeriesKey::CpuSystem, sample.cpu.system);
    optional(SeriesKey::CpuTemperature, sample.cpu_temperature());
    optional(
        SeriesKey::CpuFrequency,
        sample.cpu.average_frequency_mhz().map(|mhz| mhz as f32),
    );
    optional(SeriesKey::PowerTotal, sample.power.measured_total());
    optional(SeriesKey::PowerCpu, sample.power.cpu_package);
    optional(
        SeriesKey::BatteryCharge,
        sample.power.battery.map(|battery| battery.charge),
    );
    for (index, gpu) in sample.gpus.iter().enumerate() {
        optional(SeriesKey::GpuUsage(index), gpu.usage);
        optional(
            SeriesKey::GpuMemory(index),
            gpu.memory_used.map(|b| b as f32),
        );
        optional(SeriesKey::GpuTemperature(index), gpu.temperature);
        optional(SeriesKey::GpuPower(index), gpu.power);
    }
    for (index, core) in sample.cpu.cores.iter().enumerate() {
        values.push((SeriesKey::Core(index), core.usage));
    }
    for sensor in &sample.sensors {
        values.push((SeriesKey::Sensor(sensor.label.clone()), sensor.celsius));
    }
    for fan in &sample.fans {
        values.push((SeriesKey::Fan(fan.label.clone()), fan.rpm as f32));
    }
    values
}

/// Reduces `values` to at most `buckets` points, each the average of its bucket (`NaN` when the
/// whole bucket is a gap), so a chart never draws more points than it has pixels for.
pub fn downsample(values: &[f32], buckets: usize) -> Vec<f32> {
    if buckets == 0 || values.len() <= buckets {
        return values.to_vec();
    }
    (0..buckets)
        .map(|bucket| {
            let start = bucket * values.len() / buckets;
            let end = ((bucket + 1) * values.len() / buckets).max(start + 1);
            let finite: Vec<f32> = values[start..end]
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect();
            if finite.is_empty() {
                f32::NAN
            } else {
                finite.iter().sum::<f32>() / finite.len() as f32
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{Sensor, SensorKind};
    use pretty_assertions::assert_eq;

    fn sample(at: f64, cpu: f32) -> Sample {
        let mut sample = Sample {
            elapsed_secs: at,
            ..Sample::default()
        };
        sample.cpu.usage = cpu;
        sample
    }

    #[test]
    fn keeps_at_most_its_capacity() {
        let mut history = History::new(3);
        for i in 0..5 {
            history.push(&sample(i as f64, i as f32 / 10.0));
        }
        assert_eq!(history.len(), 3);
        assert_eq!(history.times().front(), Some(&2.0));
        assert_eq!(
            history
                .series(&SeriesKey::CpuUsage)
                .map(|s| s.values().len()),
            Some(3)
        );
        assert_eq!(history.last(&SeriesKey::CpuUsage), Some(0.4));
    }

    #[test]
    fn a_late_series_is_padded_and_a_missing_value_is_a_gap() {
        let mut history = History::new(10);
        history.push(&sample(0.0, 0.1));
        let mut with_sensor = sample(1.0, 0.2);
        with_sensor.sensors.push(Sensor {
            label: "disk".to_string(),
            kind: Some(SensorKind::Storage),
            celsius: 40.0,
            critical: None,
        });
        history.push(&with_sensor);
        history.push(&sample(2.0, 0.3));
        let key = SeriesKey::Sensor("disk".to_string());
        let values = history.window(&key, Range::OneHour);
        assert_eq!(values.len(), 3);
        assert!(values[0].is_nan());
        assert_eq!(values[1], 40.0);
        assert!(values[2].is_nan());
        assert_eq!(history.last(&key), None);
        assert_eq!(history.max_in(&key, Range::OneHour), Some(40.0));
    }

    #[test]
    fn the_window_keeps_only_the_range() {
        let mut history = History::new(1000);
        for i in 0..200 {
            history.push(&sample(i as f64, 0.5));
        }
        assert_eq!(
            history.window(&SeriesKey::CpuUsage, Range::OneMinute).len(),
            61
        );
        assert_eq!(
            history.window(&SeriesKey::CpuUsage, Range::OneHour).len(),
            200
        );
    }

    #[test]
    fn downsampling_averages_buckets_and_keeps_gaps() {
        assert_eq!(downsample(&[1.0, 2.0], 4), vec![1.0, 2.0]);
        assert_eq!(downsample(&[1.0, 3.0, 5.0, 7.0], 2), vec![2.0, 6.0]);
        let gaps = downsample(&[f32::NAN, f32::NAN, 1.0, f32::NAN], 2);
        assert!(gaps[0].is_nan());
        assert_eq!(gaps[1], 1.0);
    }
}
