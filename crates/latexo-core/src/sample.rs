//! One reading of the whole machine, taken by `latexo-sensors` once per interval. Every rate is
//! per second over the last interval, every ratio is between 0 and 1, and a value the platform
//! cannot read is `None` (or an empty list), never a made up zero.

use crate::process::ProcessInfo;

/// What does not change while the app runs: the machine, read once at startup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemInfo {
    /// The host name, `None` when unknown.
    pub host_name: Option<String>,
    /// The OS as shown to people: `Windows 11 Pro`, `Arch Linux`.
    pub os_name: String,
    /// The OS version or kernel: `24H2 (26100)`, `6.12.1`.
    pub os_version: String,
    /// The CPU model: `12th Gen Intel(R) Core(TM) i5-1235U`.
    pub cpu_brand: String,
    /// Physical cores, `None` when unknown.
    pub physical_cores: Option<usize>,
    /// Logical processors (threads).
    pub logical_cores: usize,
    /// Installed memory in bytes.
    pub memory_total: u64,
}

/// One logical processor.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CoreSample {
    /// Busy share, 0 to 1.
    pub usage: f32,
    /// Current clock in MHz, 0 when unknown.
    pub frequency_mhz: u64,
    /// Temperature of the physical core it runs on, in °C, when the platform exposes one.
    pub temperature: Option<f32>,
}

/// The processor as a whole.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuSample {
    /// Busy share of all processors, 0 to 1.
    pub usage: f32,
    /// Share spent in user mode, 0 to 1, when the platform splits it.
    pub user: Option<f32>,
    /// Share spent in the kernel, 0 to 1, when the platform splits it.
    pub system: Option<f32>,
    /// One entry per logical processor, in the OS order.
    pub cores: Vec<CoreSample>,
    /// Load average over 1, 5 and 15 minutes (Linux), `None` on Windows.
    pub load_average: Option<[f64; 3]>,
}

impl CpuSample {
    /// The average clock of the processors that report one, in MHz.
    pub fn average_frequency_mhz(&self) -> Option<u64> {
        let known: Vec<u64> = self
            .cores
            .iter()
            .map(|core| core.frequency_mhz)
            .filter(|mhz| *mhz > 0)
            .collect();
        (!known.is_empty()).then(|| known.iter().sum::<u64>() / known.len() as u64)
    }

    /// The index and usage of the busiest processor.
    pub fn busiest_core(&self) -> Option<(usize, f32)> {
        self.cores
            .iter()
            .enumerate()
            .map(|(index, core)| (index, core.usage))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The index and temperature of the hottest processor that reports one.
    pub fn hottest_core(&self) -> Option<(usize, f32)> {
        self.cores
            .iter()
            .enumerate()
            .filter_map(|(index, core)| core.temperature.map(|t| (index, t)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// Memory, in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MemorySample {
    /// Installed memory.
    pub total: u64,
    /// In use by programs and the kernel (total minus available).
    pub used: u64,
    /// Free for new programs without swapping, the page cache included.
    pub available: u64,
    /// Completely unused.
    pub free: u64,
    /// Swap or page file size.
    pub swap_total: u64,
    /// Swap or page file in use.
    pub swap_used: u64,
}

impl MemorySample {
    /// Share of memory in use, 0 to 1.
    pub fn usage(&self) -> f32 {
        ratio(self.used, self.total)
    }

    /// The page cache and other reclaimable memory: available but not free.
    pub fn cached(&self) -> u64 {
        self.available.saturating_sub(self.free)
    }
}

/// A mounted volume or drive.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Volume {
    /// `C:\` on Windows, the mount point on Linux.
    pub mount_point: String,
    /// The volume label or device name, may be empty.
    pub name: String,
    /// The file system: `NTFS`, `ext4`, `btrfs`.
    pub file_system: String,
    /// Size in bytes.
    pub total: u64,
    /// Free space in bytes.
    pub available: u64,
    /// A USB stick or external drive.
    pub removable: bool,
}

impl Volume {
    /// Share of the volume in use, 0 to 1.
    pub fn usage(&self) -> f32 {
        ratio(self.total.saturating_sub(self.available), self.total)
    }
}

/// Disk throughput over the last interval, every disk together.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DiskIo {
    /// Bytes read per second.
    pub read_per_sec: f64,
    /// Bytes written per second.
    pub write_per_sec: f64,
    /// Bytes read since the app started.
    pub read_total: u64,
    /// Bytes written since the app started.
    pub written_total: u64,
}

/// One network interface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Interface {
    /// The OS name: `eth0`, `wlan0`, `Ethernet`.
    pub name: String,
    /// Bytes received per second.
    pub received_per_sec: f64,
    /// Bytes sent per second.
    pub sent_per_sec: f64,
    /// Bytes received since the interface came up.
    pub received_total: u64,
    /// Bytes sent since the interface came up.
    pub sent_total: u64,
    /// The first IPv4 address, else the first IPv6 one.
    pub address: Option<String>,
    /// The hardware address, `None` for virtual interfaces without one.
    pub mac: Option<String>,
}

impl Interface {
    /// Whether the interface moved any data since it came up (a down Wi-Fi card did not).
    pub fn is_active(&self) -> bool {
        self.received_total > 0 || self.sent_total > 0
    }
}

/// The network, every interface together.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkSample {
    /// Bytes received per second.
    pub received_per_sec: f64,
    /// Bytes sent per second.
    pub sent_per_sec: f64,
    /// Bytes received since the app started.
    pub received_session: u64,
    /// Bytes sent since the app started.
    pub sent_session: u64,
    /// The interfaces, loopback left out.
    pub interfaces: Vec<Interface>,
}

/// What a temperature sensor measures, for grouping and for the alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SensorKind {
    /// The whole processor package (`Package id 0`, `Tctl`).
    CpuPackage,
    /// One physical core.
    CpuCore,
    /// A graphics card.
    Gpu,
    /// An SSD or hard disk.
    Storage,
    /// The motherboard, chipset, ACPI zones and anything else.
    Board,
    /// A network card or another device.
    Other,
}

/// One temperature sensor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sensor {
    /// The label the platform gives it: `Package id 0`, `Composite`, `acpitz`.
    pub label: String,
    /// The group it belongs to.
    pub kind: Option<SensorKind>,
    /// Temperature in °C.
    pub celsius: f32,
    /// The critical temperature the hardware reports, in °C.
    pub critical: Option<f32>,
}

/// One fan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fan {
    /// The label the platform gives it: `fan1`, `CPU fan`.
    pub label: String,
    /// Speed in revolutions per minute.
    pub rpm: u32,
}

/// A battery.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Battery {
    /// Charge, 0 to 1.
    pub charge: f32,
    /// Power going in (charging, positive) or out (discharging, negative), in watts.
    pub watts: Option<f32>,
    /// Plugged in.
    pub on_ac: bool,
}

/// Power readings, in watts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PowerSample {
    /// The processor package (Intel RAPL, AMD energy counters), when readable.
    pub cpu_package: Option<f32>,
    /// The graphics cards together, when readable.
    pub gpu: Option<f32>,
    /// The battery, on a laptop.
    pub battery: Option<Battery>,
}

impl PowerSample {
    /// The best known figure for the whole machine: the battery discharge on a laptop running on
    /// battery (it feeds everything), else the measured parts added up. `None` when nothing is
    /// measured.
    pub fn measured_total(&self) -> Option<f32> {
        if let Some(Battery {
            watts: Some(watts),
            on_ac: false,
            ..
        }) = self.battery
            && watts < 0.0
        {
            return Some(-watts);
        }
        match (self.cpu_package, self.gpu) {
            (None, None) => None,
            (cpu, gpu) => Some(cpu.unwrap_or(0.0) + gpu.unwrap_or(0.0)),
        }
    }
}

/// One engine of a graphics card (3D, copy, video decode).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuEngine {
    /// The engine name as the driver gives it: `3D`, `render`, `video`, `Copy`.
    pub name: String,
    /// Busy share, 0 to 1.
    pub usage: f32,
}

/// One graphics card.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuSample {
    /// The model: `Intel Iris Xe Graphics`, `NVIDIA GeForce RTX 4070`.
    pub name: String,
    /// Busy share of the busiest engine, 0 to 1, when readable.
    pub usage: Option<f32>,
    /// The engines, when the driver splits them.
    pub engines: Vec<GpuEngine>,
    /// Video memory in use, in bytes.
    pub memory_used: Option<u64>,
    /// Video memory size, in bytes.
    pub memory_total: Option<u64>,
    /// Temperature in °C.
    pub temperature: Option<f32>,
    /// Power draw in watts.
    pub power: Option<f32>,
    /// Core clock in MHz.
    pub frequency_mhz: Option<u32>,
    /// The processes using it, by PID, with their busy share of the card.
    pub processes: Vec<(u32, f32)>,
}

/// One reading of the whole machine.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    /// Seconds since the app started taking samples.
    pub elapsed_secs: f64,
    /// Seconds since the machine booted.
    pub uptime_secs: u64,
    /// The processor.
    pub cpu: CpuSample,
    /// Memory.
    pub memory: MemorySample,
    /// Disk throughput.
    pub disk_io: DiskIo,
    /// Mounted volumes, sorted by mount point.
    pub volumes: Vec<Volume>,
    /// The network.
    pub network: NetworkSample,
    /// Temperature sensors.
    pub sensors: Vec<Sensor>,
    /// Fans.
    pub fans: Vec<Fan>,
    /// Power.
    pub power: PowerSample,
    /// Graphics cards.
    pub gpus: Vec<GpuSample>,
    /// Every process.
    pub processes: Vec<ProcessInfo>,
}

impl Sample {
    /// The processor temperature to show first: the package sensor, else the hottest core, else
    /// the hottest board sensor (an ACPI zone is often the only one on Windows).
    pub fn cpu_temperature(&self) -> Option<f32> {
        let hottest = |kind| {
            self.sensors
                .iter()
                .filter(|sensor| sensor.kind == Some(kind))
                .map(|sensor| sensor.celsius)
                .max_by(f32::total_cmp)
        };
        hottest(SensorKind::CpuPackage)
            .or_else(|| hottest(SensorKind::CpuCore))
            .or_else(|| self.cpu.hottest_core().map(|(_, t)| t))
            .or_else(|| hottest(SensorKind::Board))
    }

    /// The hottest sensor of any kind.
    pub fn hottest_sensor(&self) -> Option<&Sensor> {
        self.sensors
            .iter()
            .max_by(|a, b| a.celsius.total_cmp(&b.celsius))
    }

    /// The number of threads of every process together.
    pub fn thread_count(&self) -> usize {
        self.processes
            .iter()
            .map(|process| process.threads.unwrap_or(0) as usize)
            .sum()
    }
}

/// `part / whole`, 0 when `whole` is 0.
pub fn ratio(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 / whole as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sensor(label: &str, kind: SensorKind, celsius: f32) -> Sensor {
        Sensor {
            label: label.to_string(),
            kind: Some(kind),
            celsius,
            critical: None,
        }
    }

    #[test]
    fn the_cpu_temperature_prefers_the_package() {
        let mut sample = Sample {
            sensors: vec![
                sensor("acpitz", SensorKind::Board, 50.0),
                sensor("Core 0", SensorKind::CpuCore, 70.0),
                sensor("Package id 0", SensorKind::CpuPackage, 65.0),
            ],
            ..Sample::default()
        };
        assert_eq!(sample.cpu_temperature(), Some(65.0));
        sample
            .sensors
            .retain(|s| s.kind != Some(SensorKind::CpuPackage));
        assert_eq!(sample.cpu_temperature(), Some(70.0));
        sample
            .sensors
            .retain(|s| s.kind != Some(SensorKind::CpuCore));
        assert_eq!(sample.cpu_temperature(), Some(50.0));
        sample.sensors.clear();
        assert_eq!(sample.cpu_temperature(), None);
    }

    #[test]
    fn memory_usage_and_cache() {
        let memory = MemorySample {
            total: 100,
            used: 40,
            available: 60,
            free: 20,
            ..MemorySample::default()
        };
        assert_eq!(memory.usage(), 0.4);
        assert_eq!(memory.cached(), 40);
        assert_eq!(MemorySample::default().usage(), 0.0);
    }

    #[test]
    fn average_frequency_skips_unknown_cores() {
        let cpu = CpuSample {
            cores: vec![
                CoreSample {
                    usage: 0.5,
                    frequency_mhz: 4000,
                    temperature: Some(60.0),
                },
                CoreSample {
                    usage: 0.9,
                    frequency_mhz: 0,
                    temperature: Some(80.0),
                },
                CoreSample {
                    usage: 0.1,
                    frequency_mhz: 2000,
                    temperature: None,
                },
            ],
            ..CpuSample::default()
        };
        assert_eq!(cpu.average_frequency_mhz(), Some(3000));
        assert_eq!(cpu.busiest_core(), Some((1, 0.9)));
        assert_eq!(cpu.hottest_core(), Some((1, 80.0)));
        assert_eq!(CpuSample::default().average_frequency_mhz(), None);
    }

    #[test]
    fn the_measured_total_uses_the_battery_only_when_discharging() {
        let mut power = PowerSample {
            cpu_package: Some(10.0),
            gpu: None,
            battery: Some(Battery {
                charge: 0.5,
                watts: Some(-12.5),
                on_ac: false,
            }),
        };
        assert_eq!(power.measured_total(), Some(12.5));
        power.battery = Some(Battery {
            charge: 0.5,
            watts: Some(20.0),
            on_ac: true,
        });
        assert_eq!(power.measured_total(), Some(10.0));
        assert_eq!(PowerSample::default().measured_total(), None);
    }
}
