//! Values as people read them, in the active language: the number with its separators
//! (`telaio_app::number`) and the unit from the locale files, so `18 %` in Spanish is `18%` in
//! English. Never used in logs.

use latexo_core::units::{Scaled, TemperatureUnit, Uptime, decimals_for, scale_bytes};
use rust_i18n::t;
use telaio_app::number::{format_decimal, format_integer};

/// A share from 0 to 1 as a percentage with `decimals` decimals: `18 %`, `99,1 %`.
pub fn percent(ratio: f32, decimals: usize) -> String {
    percent_value(f64::from(ratio) * 100.0, decimals)
}

/// A value already in percent: `18 %`.
pub fn percent_value(value: f64, decimals: usize) -> String {
    t!("units.percent", value = format_decimal(value, decimals)).into_owned()
}

fn scaled(scaled: Scaled) -> String {
    format!(
        "{} {}",
        format_decimal(scaled.value, scaled.decimals),
        scaled.unit
    )
}

/// A size: `16,6 GB`.
pub fn bytes(bytes: u64) -> String {
    scaled(scale_bytes(bytes as f64))
}

/// A rate: `41,2 KB/s`.
pub fn rate(per_sec: f64) -> String {
    t!("units.per_second", value = scaled(scale_bytes(per_sec))).into_owned()
}

/// A temperature in the unit of the settings: `58 °C`.
pub fn temperature(celsius: f32, unit: TemperatureUnit) -> String {
    format!(
        "{} {}",
        format_decimal(f64::from(unit.convert(celsius)).round(), 0),
        unit.symbol()
    )
}

/// Power: `214 W`, `7,5 W`.
pub fn watts(watts: f32) -> String {
    let value = f64::from(watts.abs());
    let decimals = if value < 10.0 { 1 } else { 0 };
    format!("{} W", format_decimal(value, decimals))
}

/// Energy: `1,84 kWh`, `840 Wh`.
pub fn energy(wh: f64) -> String {
    if wh >= 1000.0 {
        let kwh = wh / 1000.0;
        format!("{} kWh", format_decimal(kwh, decimals_for(kwh)))
    } else {
        format!("{} Wh", format_decimal(wh, if wh < 10.0 { 1 } else { 0 }))
    }
}

/// Money: `0,29 €`.
pub fn money(amount: f64, currency: &str) -> String {
    format!("{} {currency}", format_decimal(amount, 2))
}

/// A clock: `4,62 GHz`, `800 MHz`.
pub fn frequency(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{} GHz", format_decimal(mhz as f64 / 1000.0, 2))
    } else {
        format!("{} MHz", format_integer(mhz))
    }
}

/// A fan speed: `1.240 rpm`.
pub fn rpm(rpm: u32) -> String {
    format!("{} rpm", format_integer(u64::from(rpm)))
}

/// A decimal number with the separators of the language.
pub fn decimal(value: f64, decimals: usize) -> String {
    format_decimal(value, decimals)
}

/// Seconds: `1 s`, `0,5 s`.
pub fn seconds(secs: f64) -> String {
    let decimals = if secs.fract() == 0.0 { 0 } else { 1 };
    format!("{} s", format_decimal(secs, decimals))
}

/// An integer with the separators of the language: `3.412`.
pub fn integer(value: usize) -> String {
    format_integer(value as u64)
}

/// A duration: `13 d 5 h`, `5 h 12 min`, `3 min`.
pub fn uptime(secs: u64) -> String {
    let Uptime {
        days,
        hours,
        minutes,
    } = Uptime::from_secs(secs);
    if days > 0 {
        t!("units.days_hours", days = days, hours = hours).into_owned()
    } else if hours > 0 {
        t!("units.hours_minutes", hours = hours, minutes = minutes).into_owned()
    } else {
        t!("units.minutes", minutes = minutes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_follow_the_values() {
        assert!(watts(214.4).starts_with("214"));
        assert!(frequency(4620).ends_with("GHz"));
        assert!(frequency(800).ends_with("MHz"));
        assert!(energy(1840.0).ends_with("kWh"));
        assert!(energy(840.0).ends_with("Wh"));
        assert!(rpm(1240).ends_with("rpm"));
        assert!(temperature(100.0, TemperatureUnit::Fahrenheit).starts_with("212"));
    }
}
