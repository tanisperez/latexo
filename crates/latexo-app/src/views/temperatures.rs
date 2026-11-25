//! Readable names for the sensors and fans, from the labels the platform gives them
//! (`coretemp Package id 0`, `nvme Composite`, `thinkpad fan1`).

use rust_i18n::t;

/// The name of a temperature sensor.
pub fn sensor_label(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    let driver = lower.split_whitespace().next().unwrap_or_default();
    let rest = label.split_once(' ').map_or("", |(_, rest)| rest).trim();
    match driver {
        "coretemp" | "k10temp" | "zenpower" => {
            if let Some(number) = rest.strip_prefix("Core ") {
                t!("temperatures.core", number = number).into_owned()
            } else if lower.contains("package") || lower.contains("tctl") || lower.contains("tdie")
            {
                t!("temperatures.package").into_owned()
            } else {
                format!("CPU {rest}")
            }
        }
        "nvme" => {
            if rest.eq_ignore_ascii_case("composite") || rest.is_empty() {
                "NVMe SSD".to_string()
            } else {
                format!("NVMe SSD · {rest}")
            }
        }
        "acpitz" => t!("temperatures.acpi").into_owned(),
        "amdgpu" | "radeon" | "nouveau" => format!("GPU {rest}").trim().to_string(),
        "iwlwifi" | "iwlwifi_1" => "Wi-Fi".to_string(),
        "thinkpad" => format!("ThinkPad {rest}").trim().to_string(),
        _ if driver.starts_with("iwlwifi") => "Wi-Fi".to_string(),
        _ if driver.starts_with("r8169") => t!("temperatures.ethernet").into_owned(),
        _ => label.to_string(),
    }
}

/// The name of a fan.
pub fn fan_label(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    match lower.rsplit_once(" fan") {
        Some((_, number)) if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) => {
            t!("fans.fan", number = number).into_owned()
        }
        _ => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensors_get_readable_names() {
        assert_eq!(sensor_label("nvme Composite"), "NVMe SSD");
        assert_eq!(sensor_label("iwlwifi_1 temp1"), "Wi-Fi");
        assert_eq!(sensor_label("something else"), "something else");
        assert!(sensor_label("coretemp Core 4").contains('4'));
    }

    #[test]
    fn fans_get_a_number() {
        assert!(fan_label("thinkpad fan1").contains('1'));
        assert_eq!(fan_label("CPU fan"), "CPU fan");
    }
}
