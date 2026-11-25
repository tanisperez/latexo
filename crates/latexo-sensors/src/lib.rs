//! Reads the machine for Latexo. A [`Sampler`] keeps the platform handles and the previous
//! counters between calls, and [`Sampler::sample`] returns one [`latexo_core::Sample`]: rates are
//! per second over the time since the previous call. No `gpui`: the app calls it on a background
//! thread once per interval.
//!
//! sysinfo covers what Linux and Windows share (CPU, memory, disks, networks, processes,
//! temperature sensors and GPUs). On Linux the [`linux`] readers add the fans, the battery, the
//! processor package power, the user and kernel split and the temperature of each core.

pub mod classify;
pub mod linux;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use latexo_core::process::ProcessInfo;
use latexo_core::{
    CoreSample, CpuSample, DiskIo, GpuSample, Interface, MemorySample, NetworkSample, PowerSample,
    Sample, Sensor, SystemInfo, Volume,
};
use sysinfo::{
    Components, CpuRefreshKind, DiskRefreshKind, Disks, Gpus, MemoryRefreshKind, Networks, Pid,
    ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, Uid, UpdateKind, Users,
};

/// What the platform can read, for the "Sensors" section of Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Temperature sensors were found.
    pub temperatures: bool,
    /// Fans were found.
    pub fans: bool,
    /// The processor package power can be read (Linux needs root for it).
    pub package_power: Option<bool>,
    /// A graphics card with readable usage was found.
    pub gpu: bool,
    /// A battery was found.
    pub battery: bool,
}

/// The platform handles and the counters of the previous call.
pub struct Sampler {
    system: System,
    disks: Option<Disks>,
    networks: Option<Networks>,
    components: Option<Components>,
    gpus: Option<Gpus>,
    users: Option<Users>,
    user_names: HashMap<Uid, String>,
    own_user: Option<Uid>,
    started: Instant,
    last: Option<Instant>,
    sys: PathBuf,
    proc: PathBuf,
    cpu_times: linux::CpuTimes,
    package_power: Option<linux::PackagePower>,
    core_ids: Vec<Option<u32>>,
    disk_read_total: u64,
    disk_written_total: u64,
    net_received_session: u64,
    net_sent_session: u64,
}

impl Sampler {
    /// Opens the platform handles and takes the first readings, which the first [`Sampler::sample`]
    /// compares against. Fails only when the system information cannot be read at all; a missing
    /// part (no sensors, no GPU) is left out of every sample instead.
    pub fn new() -> Result<Self, sysinfo::Error> {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        )?;
        let users = optional("users", Users::new_with_refreshed_list());
        let user_names = user_names(users.as_ref());
        let own_user = sysinfo::get_current_pid().ok().and_then(|pid| {
            let mut probe = System::new().ok()?;
            probe.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                false,
                ProcessRefreshKind::nothing().with_user(UpdateKind::Always),
            );
            probe.process(pid).and_then(|p| p.user_id().cloned())
        });
        let sys = PathBuf::from("/sys");
        let package_power = cfg!(target_os = "linux")
            .then(|| linux::PackagePower::new(&sys))
            .flatten();
        let core_ids = if cfg!(target_os = "linux") {
            linux::core_ids(&sys, system.cpus().len())
        } else {
            Vec::new()
        };
        let mut sampler = Self {
            disks: optional(
                "disks",
                Disks::new_with_refreshed_list_specifics(
                    DiskRefreshKind::nothing().with_storage().with_io_usage(),
                ),
            ),
            networks: optional("networks", Networks::new_with_refreshed_list()),
            components: optional("sensors", Components::new_with_refreshed_list()),
            gpus: optional("GPUs", Gpus::new_with_refreshed_list()),
            system,
            users,
            user_names,
            own_user,
            started: Instant::now(),
            last: None,
            sys,
            proc: PathBuf::from("/proc"),
            cpu_times: linux::CpuTimes::default(),
            package_power,
            core_ids,
            disk_read_total: 0,
            disk_written_total: 0,
            net_received_session: 0,
            net_sent_session: 0,
        };
        // The first readings, so the first sample already has rates and CPU usage.
        sampler.refresh_processes();
        sampler.last = Some(Instant::now());
        if cfg!(target_os = "linux") {
            sampler.cpu_times.read(&sampler.proc.clone());
            if let Some(power) = sampler.package_power.as_mut() {
                power.watts();
            }
        }
        Ok(sampler)
    }

    /// The machine: model, OS, cores and memory.
    pub fn system_info(&self) -> SystemInfo {
        let cpu_brand = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .unwrap_or_default();
        SystemInfo {
            host_name: System::host_name().ok(),
            os_name: System::long_os_version()
                .or_else(|_| System::name())
                .unwrap_or_default(),
            os_version: System::kernel_version().unwrap_or_default(),
            cpu_brand,
            physical_cores: System::physical_core_count().ok(),
            logical_cores: self.system.cpus().len(),
            memory_total: self.system.total_memory(),
        }
    }

    /// What this machine and this user can read.
    pub fn capabilities(&self, sample: &Sample) -> Capabilities {
        Capabilities {
            temperatures: !sample.sensors.is_empty(),
            fans: !sample.fans.is_empty(),
            package_power: self
                .package_power
                .as_ref()
                .map(linux::PackagePower::readable),
            gpu: sample.gpus.iter().any(|gpu| gpu.usage.is_some()),
            battery: sample.power.battery.is_some(),
        }
    }

    fn refresh_processes(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_tasks()
                .with_gpu_usage()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .with_user(UpdateKind::OnlyIfNotSet),
        );
    }

    /// Reads everything once. The rates cover the time since the previous call.
    pub fn sample(&mut self) -> Sample {
        let now = Instant::now();
        let secs = self
            .last
            .replace(now)
            .map_or(1.0, |last| now.duration_since(last).as_secs_f64())
            .max(0.001);

        self.system
            .refresh_cpu_specifics(CpuRefreshKind::everything());
        self.system.refresh_memory();
        self.refresh_processes();
        if let Some(disks) = self.disks.as_mut() {
            disks.refresh_specifics(
                true,
                DiskRefreshKind::nothing().with_storage().with_io_usage(),
            );
        }
        if let Some(networks) = self.networks.as_mut() {
            networks.refresh(true);
        }
        if let Some(components) = self.components.as_mut() {
            components.refresh(true);
        }
        if let Some(gpus) = self.gpus.as_mut() {
            gpus.refresh(true);
        }

        let processes = self.processes(secs);
        let gpus = self.gpus();
        let (sensors, core_temps) = self.sensors();
        Sample {
            elapsed_secs: now.duration_since(self.started).as_secs_f64(),
            uptime_secs: System::uptime().unwrap_or(0),
            cpu: self.cpu(&core_temps),
            memory: self.memory(),
            disk_io: self.disk_io(secs),
            volumes: self.volumes(),
            network: self.network(secs),
            sensors,
            fans: if cfg!(target_os = "linux") {
                linux::read_fans(&self.sys)
            } else {
                Vec::new()
            },
            power: self.power(&gpus),
            gpus,
            processes,
        }
    }

    fn cpu(&mut self, core_temps: &HashMap<u32, f32>) -> CpuSample {
        let cores = self
            .system
            .cpus()
            .iter()
            .enumerate()
            .map(|(index, cpu)| CoreSample {
                usage: (cpu.usage() / 100.0).clamp(0.0, 1.0),
                frequency_mhz: cpu.frequency(),
                temperature: self
                    .core_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .and_then(|id| core_temps.get(&id).copied()),
            })
            .collect();
        let split = if cfg!(target_os = "linux") {
            self.cpu_times.read(&self.proc)
        } else {
            None
        };
        CpuSample {
            usage: (self.system.global_cpu_usage() / 100.0).clamp(0.0, 1.0),
            user: split.map(|(user, _)| user),
            system: split.map(|(_, system)| system),
            cores,
            load_average: cfg!(unix)
                .then(|| System::load_average().ok())
                .flatten()
                .map(|load| [load.one, load.five, load.fifteen]),
        }
    }

    fn memory(&self) -> MemorySample {
        let total = self.system.total_memory();
        let available = self.system.available_memory();
        MemorySample {
            total,
            used: total.saturating_sub(available),
            available,
            free: self.system.free_memory(),
            swap_total: self.system.total_swap(),
            swap_used: self.system.used_swap(),
        }
    }

    /// Disk throughput, each device counted once (btrfs mounts one device many times).
    fn disk_io(&mut self, secs: f64) -> DiskIo {
        let mut seen = HashSet::new();
        let (mut read, mut written) = (0u64, 0u64);
        for disk in self.disks.iter().flat_map(|disks| disks.iter()) {
            if !seen.insert(disk.name().to_os_string()) {
                continue;
            }
            let usage = disk.usage();
            read += usage.read_bytes;
            written += usage.written_bytes;
        }
        self.disk_read_total += read;
        self.disk_written_total += written;
        DiskIo {
            read_per_sec: read as f64 / secs,
            write_per_sec: written as f64 / secs,
            read_total: self.disk_read_total,
            written_total: self.disk_written_total,
        }
    }

    /// The volumes, each device once under its shortest mount point.
    fn volumes(&self) -> Vec<Volume> {
        let mut by_device: HashMap<String, Volume> = HashMap::new();
        for disk in self.disks.iter().flat_map(|disks| disks.iter()) {
            let device = disk.name().to_string_lossy().to_string();
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let volume = Volume {
                mount_point,
                name: device.clone(),
                file_system: disk.file_system().to_string_lossy().to_string(),
                total: disk.total_space(),
                available: disk.available_space(),
                removable: disk.is_removable(),
            };
            if volume.total == 0 {
                continue;
            }
            match by_device.get(&device) {
                Some(kept) if kept.mount_point.len() <= volume.mount_point.len() => {}
                _ => {
                    by_device.insert(device, volume);
                }
            }
        }
        let mut volumes: Vec<Volume> = by_device.into_values().collect();
        volumes.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        volumes
    }

    fn network(&mut self, secs: f64) -> NetworkSample {
        let mut interfaces: Vec<Interface> = self
            .networks
            .iter()
            .flat_map(|networks| networks.iter())
            .filter(|(name, _)| !is_loopback(&name.to_string_lossy()))
            .map(|(name, data)| {
                let address = data
                    .ip_networks()
                    .iter()
                    .map(|network| network.addr)
                    .find(|addr| addr.is_ipv4())
                    .or_else(|| data.ip_networks().first().map(|network| network.addr))
                    .map(|addr| addr.to_string());
                let mac = data.mac_address();
                Interface {
                    name: name.to_string_lossy().to_string(),
                    received_per_sec: data.received() as f64 / secs,
                    sent_per_sec: data.transmitted() as f64 / secs,
                    received_total: data.total_received(),
                    sent_total: data.total_transmitted(),
                    address,
                    mac: (!mac.is_unspecified()).then(|| mac.to_string()),
                }
            })
            .collect();
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));
        let received: f64 = interfaces.iter().map(|i| i.received_per_sec).sum();
        let sent: f64 = interfaces.iter().map(|i| i.sent_per_sec).sum();
        self.net_received_session += (received * secs) as u64;
        self.net_sent_session += (sent * secs) as u64;
        NetworkSample {
            received_per_sec: received,
            sent_per_sec: sent,
            received_session: self.net_received_session,
            sent_session: self.net_sent_session,
            interfaces,
        }
    }

    /// The sensors, and the temperature of each physical core by its id (Linux `coretemp`).
    fn sensors(&self) -> (Vec<Sensor>, HashMap<u32, f32>) {
        let mut core_temps = HashMap::new();
        let mut sensors: Vec<Sensor> = self
            .components
            .iter()
            .flat_map(|components| components.iter())
            .filter_map(|component| {
                let celsius = component.temperature()?;
                if !celsius.is_finite() || celsius <= -40.0 {
                    return None;
                }
                let label = component.label().to_string();
                if let Some(id) = classify::core_id(&label) {
                    core_temps.insert(id, celsius);
                }
                Some(Sensor {
                    kind: Some(classify::sensor_kind(&label)),
                    critical: component.critical().filter(|c| c.is_finite() && *c > 0.0),
                    label,
                    celsius,
                })
            })
            .collect();
        sensors.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.label.cmp(&b.label)));
        (sensors, core_temps)
    }

    fn gpus(&self) -> Vec<GpuSample> {
        let Some(gpus) = self.gpus.as_ref() else {
            return Vec::new();
        };
        let busy: Vec<(u32, f32)> = self
            .system
            .processes()
            .values()
            .filter_map(|process| {
                let usage = process.gpu_usage()?;
                (usage > 0.0).then(|| (process.pid().as_u32(), usage / 100.0))
            })
            .collect();
        let single = gpus.len() == 1;
        gpus.iter()
            .map(|gpu| {
                let name = match (gpu.vendor(), gpu.model()) {
                    (_, Some(model)) if !model.is_empty() => model.to_string(),
                    (Some(vendor), _) => vendor.to_string(),
                    _ => "GPU".to_string(),
                };
                let mut processes = if single { busy.clone() } else { Vec::new() };
                processes.sort_by(|a, b| b.1.total_cmp(&a.1));
                GpuSample {
                    name,
                    usage: gpu.usage().map(|usage| (usage / 100.0).clamp(0.0, 1.0)),
                    engines: Vec::new(),
                    memory_used: gpu.used_memory(),
                    memory_total: gpu.total_memory(),
                    temperature: None,
                    power: None,
                    frequency_mhz: None,
                    processes,
                }
            })
            .collect()
    }

    fn power(&mut self, gpus: &[GpuSample]) -> PowerSample {
        let gpu: Option<f32> = gpus.iter().filter_map(|gpu| gpu.power).reduce(|a, b| a + b);
        if !cfg!(target_os = "linux") {
            return PowerSample {
                gpu,
                ..PowerSample::default()
            };
        }
        PowerSample {
            cpu_package: self
                .package_power
                .as_mut()
                .and_then(linux::PackagePower::watts),
            gpu,
            battery: linux::read_battery(&self.sys),
        }
    }

    fn processes(&mut self, secs: f64) -> Vec<ProcessInfo> {
        let mut unknown_user = false;
        let list: Vec<ProcessInfo> = self
            .system
            .processes()
            .values()
            // Linux lists threads as processes too; they belong to their process.
            .filter(|process| process.thread_kind().is_none())
            .map(|process| {
                let user = process.user_id().and_then(|uid| {
                    let name = self.user_names.get(uid).cloned();
                    unknown_user |= name.is_none();
                    name
                });
                let disk = process.disk_usage();
                ProcessInfo {
                    pid: process.pid().as_u32(),
                    parent: process.parent().map(Pid::as_u32),
                    name: process_name(process),
                    user,
                    cpu_percent: process.cpu_usage(),
                    memory: process.memory(),
                    threads: process.tasks().map(|tasks| tasks.len().max(1) as u32),
                    disk_read_per_sec: disk.read_bytes as f64 / secs,
                    disk_write_per_sec: disk.written_bytes as f64 / secs,
                    own: self.own_user.is_some() && process.user_id() == self.own_user.as_ref(),
                }
            })
            .collect();
        if unknown_user && let Some(users) = self.users.as_mut() {
            users.refresh();
            self.user_names = user_names(Some(users));
        }
        list
    }
}

/// A part of the system information, `None` (logged) when the platform cannot read it.
fn optional<T>(what: &str, result: Result<T, sysinfo::Error>) -> Option<T> {
    result
        .map_err(|error| log::warn!("could not read the {what}: {error}"))
        .ok()
}

fn user_names(users: Option<&Users>) -> HashMap<Uid, String> {
    users
        .iter()
        .flat_map(|users| users.iter())
        .map(|user| (user.id().clone(), user.name().to_string()))
        .collect()
}

/// The name of a process. Linux cuts the name it reports at 15 characters (`systemd-timesyn`):
/// when the executable's file name starts with that name, it is the full one.
fn process_name(process: &sysinfo::Process) -> String {
    let name = process.name().to_string_lossy();
    full_name(
        &name,
        process
            .exe()
            .and_then(|exe| exe.file_name())
            .map(|f| f.to_string_lossy())
            .as_deref(),
    )
}

fn full_name(name: &str, exe: Option<&str>) -> String {
    match exe {
        Some(exe) if name.len() >= 15 && exe.len() > name.len() && exe.starts_with(name) => {
            exe.to_string()
        }
        _ => name.to_string(),
    }
}

fn is_loopback(name: &str) -> bool {
    name == "lo" || name.starts_with("Loopback")
}

/// Why a process could not be ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillError {
    /// It is gone already.
    NotFound,
    /// The OS refused (another user's process, a protected one).
    Denied,
}

/// Ends the processes in `pids`, in order: a polite request first where the OS has one (`SIGTERM`
/// on Linux), the hard way on Windows. Returns the first error, after trying every PID.
pub fn end_processes(pids: &[u32]) -> Result<(), KillError> {
    let targets: Vec<Pid> = pids.iter().map(|pid| Pid::from_u32(*pid)).collect();
    let Ok(mut system) = System::new() else {
        return Err(KillError::NotFound);
    };
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&targets),
        false,
        ProcessRefreshKind::nothing(),
    );
    let mut result = Ok(());
    for pid in targets {
        let Some(process) = system.process(pid) else {
            result = result.and(Err(KillError::NotFound));
            continue;
        };
        let ended = process
            .kill_with(sysinfo::Signal::Term)
            .unwrap_or_else(|| process.kill());
        log::info!(
            "ending process {}: {}",
            pid.as_u32(),
            if ended { "sent" } else { "refused" }
        );
        if !ended {
            result = result.and(Err(KillError::Denied));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_name_takes_the_executable_name() {
        assert_eq!(
            full_name("systemd-timesyn", Some("systemd-timesyncd")),
            "systemd-timesyncd"
        );
        assert_eq!(full_name("bash", Some("bash")), "bash");
        assert_eq!(full_name("Web Content", Some("firefox")), "Web Content");
    }

    #[test]
    fn loopback_interfaces_are_left_out() {
        assert!(is_loopback("lo"));
        assert!(is_loopback("Loopback Pseudo-Interface 1"));
        assert!(!is_loopback("eth0"));
    }

    #[test]
    fn sensor_kinds_are_ordered_cpu_first() {
        use latexo_core::SensorKind;
        assert!(SensorKind::CpuPackage < SensorKind::Gpu);
        assert!(SensorKind::Storage < SensorKind::Other);
    }
}
