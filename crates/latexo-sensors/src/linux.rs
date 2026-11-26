//! What Linux exposes in `/sys` and `/proc` beyond sysinfo: fans (`hwmon`), the battery
//! (`power_supply`), the processor package power (`powercap`, Intel RAPL and AMD), the user and
//! kernel split of the CPU time (`/proc/stat`) and which physical core each logical processor is
//! (`topology/core_id`). Every reader takes the root folder, so the tests point it at fixtures.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use latexo_core::{Battery, Fan};

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

fn read_number<T: std::str::FromStr>(path: &Path) -> Option<T> {
    read_trimmed(path)?.parse().ok()
}

/// Every fan of every `hwmon` device under `sys` (usually `/sys`).
pub fn read_fans(sys: &Path) -> Vec<Fan> {
    let mut fans = Vec::new();
    let Ok(devices) = fs::read_dir(sys.join("class/hwmon")) else {
        return fans;
    };
    let mut devices: Vec<PathBuf> = devices.flatten().map(|entry| entry.path()).collect();
    devices.sort();
    for device in devices {
        let driver = read_trimmed(&device.join("name")).unwrap_or_default();
        let Ok(entries) = fs::read_dir(&device) else {
            continue;
        };
        let mut inputs: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("fan") && name.ends_with("_input"))
            .collect();
        inputs.sort();
        for input in inputs {
            let Some(rpm) = read_number::<u32>(&device.join(&input)) else {
                continue;
            };
            let base = input.trim_end_matches("_input");
            let label = read_trimmed(&device.join(format!("{base}_label")))
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| format!("{driver} {base}"));
            fans.push(Fan { label, rpm });
        }
    }
    fans
}

/// The system battery (not a mouse's or a phone's, whose `scope` is `Device`), with the power
/// going in or out and whether mains power is plugged in.
pub fn read_battery(sys: &Path) -> Option<Battery> {
    let supplies: Vec<PathBuf> = fs::read_dir(sys.join("class/power_supply"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    let kind = |path: &Path| read_trimmed(&path.join("type")).unwrap_or_default();
    let on_ac = supplies
        .iter()
        .any(|path| kind(path) == "Mains" && read_number::<u8>(&path.join("online")) == Some(1));
    let mut batteries: Vec<&PathBuf> = supplies
        .iter()
        .filter(|path| kind(path) == "Battery")
        .filter(|path| read_trimmed(&path.join("scope")).as_deref() != Some("Device"))
        .collect();
    batteries.sort();
    let battery = batteries.first()?;
    let charge = read_number::<f32>(&battery.join("capacity"))? / 100.0;
    let status = read_trimmed(&battery.join("status")).unwrap_or_default();
    let watts = read_number::<f64>(&battery.join("power_now"))
        .map(|microwatts| microwatts / 1e6)
        .or_else(|| {
            let amps = read_number::<f64>(&battery.join("current_now"))? / 1e6;
            let volts = read_number::<f64>(&battery.join("voltage_now"))? / 1e6;
            Some(amps * volts)
        })
        .map(|watts| match status.as_str() {
            "Discharging" => -watts.abs(),
            _ => watts.abs(),
        })
        .map(|watts| watts as f32);
    Some(Battery {
        charge: charge.clamp(0.0, 1.0),
        watts,
        on_ac,
    })
}

/// The processor package power from the `powercap` energy counter. Its file is readable only by
/// root on most distributions, so this stays `None` for a normal user.
#[derive(Debug)]
pub struct PackagePower {
    energy: PathBuf,
    max_range: Option<u64>,
    last: Option<(u64, Instant)>,
}

impl PackagePower {
    /// The reader of the first package domain under `sys`, `None` when there is none.
    pub fn new(sys: &Path) -> Option<Self> {
        let domain = sys.join("class/powercap/intel-rapl:0");
        let energy = domain.join("energy_uj");
        energy.exists().then(|| Self {
            max_range: read_number(&domain.join("max_energy_range_uj")),
            energy,
            last: None,
        })
    }

    /// Whether the counter can be read by this user.
    pub fn readable(&self) -> bool {
        fs::File::open(&self.energy).is_ok()
    }

    /// The average power since the last call, in watts. `None` the first time and when the
    /// counter cannot be read.
    pub fn watts(&mut self) -> Option<f32> {
        let now = Instant::now();
        let energy: u64 = read_number(&self.energy)?;
        let previous = self.last.replace((energy, now));
        let (before, then) = previous?;
        let secs = now.duration_since(then).as_secs_f64();
        if secs <= 0.0 {
            return None;
        }
        // The counter wraps around at its range.
        let delta = if energy >= before {
            energy - before
        } else {
            self.max_range?.saturating_sub(before) + energy
        };
        Some((delta as f64 / 1e6 / secs) as f32)
    }
}

/// The user and kernel split of the CPU time, from the first line of `/proc/stat`.
#[derive(Debug, Default)]
pub struct CpuTimes {
    last: Option<[u64; 3]>,
}

impl CpuTimes {
    /// The shares of user and kernel time since the last call, 0 to 1 each. `None` the first time.
    pub fn read(&mut self, proc: &Path) -> Option<(f32, f32)> {
        let text = fs::read_to_string(proc.join("stat")).ok()?;
        let now = parse_stat(&text)?;
        let before = self.last.replace(now)?;
        let total = now[2].saturating_sub(before[2]);
        if total == 0 {
            return None;
        }
        let share = |index: usize| now[index].saturating_sub(before[index]) as f32 / total as f32;
        Some((share(0), share(1)))
    }
}

/// `[user, system, total]` jiffies of the `cpu` line.
fn parse_stat(text: &str) -> Option<[u64; 3]> {
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|field| field.parse().ok())
        .collect();
    let get = |index: usize| fields.get(index).copied().unwrap_or(0);
    let user = get(0) + get(1);
    let system = get(2) + get(5) + get(6);
    // Guest time is already counted in user time.
    let total: u64 = fields.iter().take(8).sum();
    Some([user, system, total])
}

/// The physical core id of each logical processor, `None` where the topology is unknown.
pub fn core_ids(sys: &Path, logical: usize) -> Vec<Option<u32>> {
    (0..logical)
        .map(|cpu| read_number(&sys.join(format!("devices/system/cpu/cpu{cpu}/topology/core_id"))))
        .collect()
}

/// The busy time of each engine of one DRM client, from one `fdinfo` file: its client id and
/// `(engine, nanoseconds)`, plus the capacity of the engines that have more than one unit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrmClient {
    /// The client id; several files (threads, duplicated descriptors) can show the same client.
    pub id: u64,
    /// Busy nanoseconds per engine since the client started.
    pub engines: Vec<(String, u64)>,
    /// Units per engine, for the engines with more than one.
    pub capacity: Vec<(String, u32)>,
}

/// Reads the DRM client of an `fdinfo` file, `None` when it is not one.
pub fn parse_fdinfo(text: &str) -> Option<DrmClient> {
    let mut client = DrmClient::default();
    let mut found = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if key == "drm-client-id" {
            client.id = value.parse().ok()?;
            found = true;
        } else if let Some(engine) = key.strip_prefix("drm-engine-capacity-") {
            if let Ok(units) = value.parse() {
                client.capacity.push((engine.to_string(), units));
            }
        } else if let Some(engine) = key.strip_prefix("drm-engine-") {
            let nanos = value.trim_end_matches("ns").trim().parse().ok();
            if let Some(nanos) = nanos {
                client.engines.push((engine.to_string(), nanos));
            }
        }
    }
    found.then_some(client)
}

/// The busy share of each GPU engine (`render`, `video`, `copy`, ...), added up over the DRM
/// clients of every process this user can read. Only read while the GPU screen is shown: it
/// looks at every file descriptor of every process.
#[derive(Debug, Default)]
pub struct GpuEngines {
    last: Option<(Instant, EngineTimes)>,
}

/// Busy nanoseconds by DRM client and engine.
type EngineTimes = HashMap<(u64, String), u64>;

impl GpuEngines {
    /// The share of each engine since the previous call, 0 to 1, in a stable order. Empty the
    /// first time and when no client can be read.
    pub fn read(&mut self, proc: &Path) -> Vec<(String, f32)> {
        let now = Instant::now();
        let mut busy: EngineTimes = HashMap::new();
        let mut capacity: HashMap<String, u32> = HashMap::new();
        for client in drm_clients(proc) {
            for (engine, units) in client.capacity {
                capacity.insert(engine, units.max(1));
            }
            for (engine, nanos) in client.engines {
                let entry = busy.entry((client.id, engine)).or_insert(0);
                *entry = (*entry).max(nanos);
            }
        }
        let previous = self.last.replace((now, busy.clone()));
        let Some((then, before)) = previous else {
            return Vec::new();
        };
        let elapsed = now.duration_since(then).as_nanos() as f64;
        if elapsed <= 0.0 {
            return Vec::new();
        }
        let mut totals: Vec<(String, f64)> = Vec::new();
        for ((client, engine), nanos) in &busy {
            let delta = before
                .get(&(*client, engine.clone()))
                .map_or(0, |old| nanos.saturating_sub(*old));
            match totals.iter_mut().find(|(name, _)| name == engine) {
                Some((_, total)) => *total += delta as f64,
                None => totals.push((engine.clone(), delta as f64)),
            }
        }
        let mut shares: Vec<(String, f32)> = totals
            .into_iter()
            .map(|(engine, total)| {
                let units = f64::from(capacity.get(&engine).copied().unwrap_or(1));
                let share = (total / (elapsed * units)).clamp(0.0, 1.0) as f32;
                (engine, share)
            })
            .collect();
        shares.sort_by_key(|(engine, _)| engine_order(engine));
        shares
    }

    /// Forgets the previous reading, when the GPU screen is left.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// The order engines are listed in: 3D first, then video, copy and the rest.
fn engine_order(engine: &str) -> (u8, String) {
    let rank = match engine {
        "render" | "gfx" => 0,
        "compute" => 1,
        "video" | "dec" | "enc" => 2,
        "video-enhance" => 3,
        "copy" => 4,
        _ => 5,
    };
    (rank, engine.to_string())
}

/// Every DRM client in the `fdinfo` of the processes under `proc` this user can read.
fn drm_clients(proc: &Path) -> Vec<DrmClient> {
    let mut clients = Vec::new();
    let Ok(entries) = fs::read_dir(proc) else {
        return clients;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let is_drm =
                fs::read_link(fd.path()).is_ok_and(|target| target.starts_with("/dev/dri/"));
            if !is_drm {
                continue;
            }
            let info = entry.path().join("fdinfo").join(fd.file_name());
            if let Some(client) = fs::read_to_string(info)
                .ok()
                .as_deref()
                .and_then(parse_fdinfo)
            {
                clients.push(client);
            }
        }
    }
    clients
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn fans_are_read_with_their_labels() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "class/hwmon/hwmon4/name", "thinkpad\n");
        write(dir.path(), "class/hwmon/hwmon4/fan1_input", "2400\n");
        write(dir.path(), "class/hwmon/hwmon7/name", "nct6798\n");
        write(dir.path(), "class/hwmon/hwmon7/fan2_input", "820\n");
        write(dir.path(), "class/hwmon/hwmon7/fan2_label", "CPU fan\n");
        assert_eq!(
            read_fans(dir.path()),
            vec![
                Fan {
                    label: "thinkpad fan1".to_string(),
                    rpm: 2400
                },
                Fan {
                    label: "CPU fan".to_string(),
                    rpm: 820
                },
            ]
        );
    }

    #[test]
    fn the_system_battery_is_read_and_a_mouse_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "class/power_supply/AC/type", "Mains\n");
        write(dir.path(), "class/power_supply/AC/online", "0\n");
        write(dir.path(), "class/power_supply/BAT0/type", "Battery\n");
        write(dir.path(), "class/power_supply/BAT0/capacity", "81\n");
        write(
            dir.path(),
            "class/power_supply/BAT0/status",
            "Discharging\n",
        );
        write(dir.path(), "class/power_supply/BAT0/power_now", "7500000\n");
        write(
            dir.path(),
            "class/power_supply/hidpp_battery_0/type",
            "Battery\n",
        );
        write(
            dir.path(),
            "class/power_supply/hidpp_battery_0/scope",
            "Device\n",
        );
        let battery = read_battery(dir.path()).unwrap();
        assert_eq!(battery.charge, 0.81);
        assert_eq!(battery.watts, Some(-7.5));
        assert!(!battery.on_ac);
    }

    #[test]
    fn the_cpu_split_comes_from_the_deltas() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "stat",
            "cpu  100 0 50 850 0 0 0 0 0 0\ncpu0 1 2 3\n",
        );
        let mut times = CpuTimes::default();
        assert_eq!(times.read(dir.path()), None);
        write(dir.path(), "stat", "cpu  130 0 60 910 0 0 0 0 0 0\n");
        assert_eq!(times.read(dir.path()), Some((0.3, 0.1)));
    }

    #[test]
    fn package_power_uses_the_counter_delta() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "class/powercap/intel-rapl:0/energy_uj",
            "1000000\n",
        );
        let mut power = PackagePower::new(dir.path()).unwrap();
        assert!(power.readable());
        assert_eq!(power.watts(), None);
        write(
            dir.path(),
            "class/powercap/intel-rapl:0/energy_uj",
            "3000000\n",
        );
        let watts = power.watts().unwrap();
        assert!(watts > 0.0);
    }

    #[test]
    fn a_drm_fdinfo_gives_its_client_and_engines() {
        let text = "pos:\t0\ndrm-driver:\ti915\ndrm-client-id:\t638\ndrm-engine-render:\t213512 ns\ndrm-engine-copy:\t0 ns\ndrm-engine-capacity-video:\t2\ndrm-engine-video:\t40 ns\n";
        let client = parse_fdinfo(text).unwrap();
        assert_eq!(client.id, 638);
        assert_eq!(
            client.engines,
            vec![
                ("render".to_string(), 213_512),
                ("copy".to_string(), 0),
                ("video".to_string(), 40)
            ]
        );
        assert_eq!(client.capacity, vec![("video".to_string(), 2)]);
        assert_eq!(parse_fdinfo("pos:\t0\nflags:\t02\n"), None);
    }

    #[test]
    fn engines_are_listed_3d_first() {
        let mut engines = vec!["copy", "video", "render"];
        engines.sort_by_key(|engine| engine_order(engine));
        assert_eq!(engines, vec!["render", "video", "copy"]);
    }

    #[test]
    fn core_ids_follow_the_topology() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "devices/system/cpu/cpu0/topology/core_id",
            "0\n",
        );
        write(
            dir.path(),
            "devices/system/cpu/cpu1/topology/core_id",
            "8\n",
        );
        assert_eq!(core_ids(dir.path(), 3), vec![Some(0), Some(8), None]);
    }
}
