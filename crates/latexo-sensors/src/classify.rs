//! Which group a temperature sensor belongs to, from the label the platform gives it. Linux labels
//! are `<hwmon driver> <sensor label>` (`coretemp Package id 0`, `nvme Composite`, `amdgpu edge`);
//! Windows ones are WMI thermal zones (`ACPI\ThermalZone\TZ00_0`).

use latexo_core::SensorKind;

/// The group of a sensor by its label.
pub fn sensor_kind(label: &str) -> SensorKind {
    let lower = label.to_ascii_lowercase();
    let driver = lower.split_whitespace().next().unwrap_or_default();
    match driver {
        "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" => {
            if lower.contains("package") || lower.contains("tctl") || lower.contains("tdie") {
                SensorKind::CpuPackage
            } else if lower.contains("core") || lower.contains("tccd") {
                SensorKind::CpuCore
            } else {
                SensorKind::CpuPackage
            }
        }
        "amdgpu" | "radeon" | "nouveau" | "nvidia" | "i915" | "xe" => SensorKind::Gpu,
        "nvme" | "drivetemp" => SensorKind::Storage,
        "acpitz" | "thinkpad" | "dell_smm" | "asus" | "nct6775" | "nct6798" | "it87"
        | "pch_cannonlake" | "pch_skylake" | "pch_cometlake" => SensorKind::Board,
        _ if lower.contains("thermalzone") || lower.contains("cpu") => SensorKind::Board,
        _ if driver.starts_with("pch_") => SensorKind::Board,
        _ => SensorKind::Other,
    }
}

/// Whether a sensor is slow to read: an NVMe drive, an ACPI zone or a laptop's embedded
/// controller, which answer in 0.5 to 2.5 ms each instead of microseconds.
pub fn is_slow(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    let driver = lower.split_whitespace().next().unwrap_or_default();
    matches!(
        driver,
        "nvme" | "drivetemp" | "acpitz" | "thinkpad" | "dell_smm" | "asus"
    ) || lower.contains("thermalzone")
}

/// The physical core id of a `coretemp Core N` label.
pub fn core_id(label: &str) -> Option<u32> {
    let lower = label.to_ascii_lowercase();
    if !lower.starts_with("coretemp") {
        return None;
    }
    let rest = lower.split("core ").nth(1)?;
    rest.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn known_drivers_are_grouped() {
        assert_eq!(sensor_kind("coretemp Package id 0"), SensorKind::CpuPackage);
        assert_eq!(sensor_kind("coretemp Core 4"), SensorKind::CpuCore);
        assert_eq!(sensor_kind("k10temp Tctl"), SensorKind::CpuPackage);
        assert_eq!(sensor_kind("k10temp Tccd1"), SensorKind::CpuCore);
        assert_eq!(sensor_kind("nvme Composite"), SensorKind::Storage);
        assert_eq!(sensor_kind("amdgpu edge"), SensorKind::Gpu);
        assert_eq!(sensor_kind("acpitz temp1"), SensorKind::Board);
        assert_eq!(sensor_kind("iwlwifi_1 temp1"), SensorKind::Other);
        assert_eq!(sensor_kind("ACPI\\ThermalZone\\TZ00_0"), SensorKind::Board);
    }

    #[test]
    fn slow_sensors_are_the_drives_and_firmware_ones() {
        assert!(is_slow("nvme Composite WD PC SN740"));
        assert!(is_slow("acpitz temp1"));
        assert!(!is_slow("coretemp Core 4"));
    }

    #[test]
    fn core_ids_come_from_coretemp_labels() {
        assert_eq!(core_id("coretemp Core 12"), Some(12));
        assert_eq!(core_id("coretemp Package id 0"), None);
        assert_eq!(core_id("k10temp Tccd1"), None);
    }
}
