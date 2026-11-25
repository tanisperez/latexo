//! The units values are shown in. Only the choice of unit and of decimals lives here; the app
//! writes the number with the separators of the active language (`telaio_app::number`).

/// A size or rate scaled to a readable unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaled {
    /// The value in [`Scaled::unit`].
    pub value: f64,
    /// The unit: `B`, `KB`, `MB`, `GB` or `TB` (binary multiples, as Windows names them).
    pub unit: &'static str,
    /// The decimals worth showing: about three significant digits, none for bytes.
    pub decimals: usize,
}

const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// `bytes` in the biggest unit that keeps the value at 1 or more.
pub fn scale_bytes(bytes: f64) -> Scaled {
    let mut value = bytes.max(0.0);
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    // 1023.6 KB rounds to "1024 KB": show the next unit instead.
    if unit < UNITS.len() - 1 && value >= 999.5 {
        value /= 1024.0;
        unit += 1;
    }
    let decimals = if unit == 0 { 0 } else { decimals_for(value) };
    Scaled {
        value,
        unit: UNITS[unit],
        decimals,
    }
}

/// About three significant digits: 2 decimals under 10, 1 under 100, none above.
pub fn decimals_for(value: f64) -> usize {
    let value = value.abs();
    if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    }
}

/// The temperature unit of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureUnit {
    /// Degrees Celsius, the default.
    #[default]
    Celsius,
    /// Degrees Fahrenheit.
    Fahrenheit,
}

impl TemperatureUnit {
    /// `celsius` in this unit.
    pub fn convert(self, celsius: f32) -> f32 {
        match self {
            TemperatureUnit::Celsius => celsius,
            TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
        }
    }

    /// The symbol written after the number.
    pub fn symbol(self) -> &'static str {
        match self {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        }
    }
}

/// A duration split for display: `13 d 5 h`, `5 h 12 min`, `3 min`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uptime {
    /// Whole days.
    pub days: u64,
    /// Hours after the days.
    pub hours: u64,
    /// Minutes after the hours.
    pub minutes: u64,
}

impl Uptime {
    /// Splits `secs`.
    pub fn from_secs(secs: u64) -> Self {
        Self {
            days: secs / 86_400,
            hours: secs % 86_400 / 3600,
            minutes: secs % 3600 / 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bytes_scale_to_the_biggest_unit() {
        assert_eq!(scale_bytes(512.0).unit, "B");
        assert_eq!(scale_bytes(512.0).decimals, 0);
        let kb = scale_bytes(42_188.8);
        assert_eq!(kb.unit, "KB");
        assert!((kb.value - 41.2).abs() < 0.01);
        assert_eq!(kb.decimals, 1);
        let gb = scale_bytes(16.6 * 1024.0 * 1024.0 * 1024.0);
        assert_eq!(gb.unit, "GB");
        assert_eq!(gb.decimals, 1);
        assert_eq!(scale_bytes(1023.9 * 1024.0).unit, "MB");
        assert_eq!(scale_bytes(-3.0).value, 0.0);
    }

    #[test]
    fn fahrenheit_converts() {
        assert_eq!(TemperatureUnit::Fahrenheit.convert(100.0), 212.0);
        assert_eq!(TemperatureUnit::Celsius.convert(58.0), 58.0);
    }

    #[test]
    fn uptime_splits() {
        assert_eq!(
            Uptime::from_secs(13 * 86_400 + 5 * 3600 + 61),
            Uptime {
                days: 13,
                hours: 5,
                minutes: 1
            }
        );
    }
}
