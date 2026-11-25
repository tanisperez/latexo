//! The energy account: the measured power added up over time into watt hours for today and for
//! this month, and their cost at the price of the settings. The app passes the local day and
//! month (as plain numbers), so this stays free of clocks and time zones.

use serde::{Deserialize, Serialize};

/// Watt hours used today and this month. Saved between runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct EnergyAccount {
    /// The local day the `today` figure belongs to, as days since 1970-01-01.
    pub day: i64,
    /// The local month the `month` figure belongs to, as `year * 12 + month - 1`.
    pub month: i64,
    /// Watt hours today.
    pub today_wh: f64,
    /// Watt hours this month.
    pub month_wh: f64,
}

impl EnergyAccount {
    /// Adds `watts` drawn for `secs` seconds on local `day` of local `month`. A new day or month
    /// starts its figure at zero. Gaps longer than a minute (a suspended laptop) are not counted.
    pub fn add(&mut self, watts: f64, secs: f64, day: i64, month: i64) {
        if day != self.day {
            self.day = day;
            self.today_wh = 0.0;
        }
        if month != self.month {
            self.month = month;
            self.month_wh = 0.0;
        }
        if !(0.0..=60.0).contains(&secs) || !watts.is_finite() || watts <= 0.0 {
            return;
        }
        let wh = watts * secs / 3600.0;
        self.today_wh += wh;
        self.month_wh += wh;
    }
}

/// The cost of `wh` watt hours at `price_per_kwh`.
pub fn cost(wh: f64, price_per_kwh: f64) -> f64 {
    wh / 1000.0 * price_per_kwh
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn adds_and_resets_per_day_and_month() {
        let mut account = EnergyAccount::default();
        account.add(3600.0, 1.0, 10, 5);
        assert_eq!(account.today_wh, 1.0);
        account.add(3600.0, 1.0, 10, 5);
        assert_eq!(account.today_wh, 2.0);
        account.add(3600.0, 1.0, 11, 5);
        assert_eq!(account.today_wh, 1.0);
        assert_eq!(account.month_wh, 3.0);
        account.add(3600.0, 1.0, 12, 6);
        assert_eq!(account.month_wh, 1.0);
    }

    #[test]
    fn long_gaps_are_not_counted() {
        let mut account = EnergyAccount::default();
        account.add(100.0, 3600.0, 0, 0);
        assert_eq!(account.today_wh, 0.0);
    }

    #[test]
    fn cost_is_per_kwh() {
        assert!((cost(1840.0, 0.16) - 0.2944).abs() < 1e-9);
    }
}
