//! The domain model of Latexo, with no IO and no `gpui`: what one sample of the machine holds
//! ([`sample`]), the history of every series ([`history`]), the process list with its sorting and
//! filtering ([`process`]), the alert rules ([`alerts`]), the energy account ([`energy`]) and the
//! units the values are shown in ([`units`]).
//!
//! `latexo-sensors` fills a [`sample::Sample`] once per interval; the app keeps the history and
//! draws it. Everything here is plain data and plain functions, unit tested without a machine.

pub mod alerts;
pub mod energy;
pub mod history;
pub mod process;
pub mod sample;
pub mod units;

pub use history::{History, Range, Series, SeriesKey};
pub use sample::{
    Battery, CoreSample, CpuSample, DiskIo, Fan, GpuEngine, GpuSample, Interface, MemorySample,
    NetworkSample, PowerSample, Sample, Sensor, SensorKind, SystemInfo, Volume,
};
