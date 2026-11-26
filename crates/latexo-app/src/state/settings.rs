//! Application settings, persisted to `<OS config dir>/latexo/settings.toml` and applied live:
//! there is no Save button, every change made in the Settings screen writes the file at once.
//!
//! Every field is read and validated on its own: a field of the wrong type or out of range falls
//! back to its own default instead of discarding the whole file, and an unknown key is ignored.

use std::fs;
use std::path::{Path, PathBuf};

use latexo_core::Range;
use latexo_core::alerts::{Rule, default_rules};
use latexo_core::units::TemperatureUnit;
use serde::Serialize;

pub use telaio_app::language::LanguageChoice;
pub use telaio_app::logging::LogLevel;

/// The settings file, relative to the config directory.
const SETTINGS_FILE: &str = "settings.toml";

/// `<config dir>/latexo`, `None` when the OS does not report the config directory.
pub fn config_dir() -> Option<PathBuf> {
    telaio_app::paths::config_dir(&latexo_cli::IDENTITY)
}

/// Which theme mode the app follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Follow the OS light/dark setting.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

impl From<ThemeChoice> for telaio_ui::ThemeChoice {
    fn from(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::System => telaio_ui::ThemeChoice::System,
            ThemeChoice::Light => telaio_ui::ThemeChoice::Light,
            ThemeChoice::Dark => telaio_ui::ThemeChoice::Dark,
        }
    }
}

/// How often the machine is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum Interval {
    /// Every half second.
    #[serde(rename = "0.5s")]
    HalfSecond,
    /// Every second, the default.
    #[default]
    #[serde(rename = "1s")]
    OneSecond,
    /// Every two seconds.
    #[serde(rename = "2s")]
    TwoSeconds,
    /// Every five seconds.
    #[serde(rename = "5s")]
    FiveSeconds,
}

impl Interval {
    /// Every interval, shortest first.
    pub const ALL: [Interval; 4] = [
        Interval::HalfSecond,
        Interval::OneSecond,
        Interval::TwoSeconds,
        Interval::FiveSeconds,
    ];

    /// The length in seconds.
    pub fn secs(self) -> f64 {
        match self {
            Interval::HalfSecond => 0.5,
            Interval::OneSecond => 1.0,
            Interval::TwoSeconds => 2.0,
            Interval::FiveSeconds => 5.0,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Interval::HalfSecond => "0.5s",
            Interval::OneSecond => "1s",
            Interval::TwoSeconds => "2s",
            Interval::FiveSeconds => "5s",
        }
    }
}

/// How every chart is drawn, chosen in the header of each screen and in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartStyle {
    /// A line with a soft area under it, the default.
    #[default]
    Line,
    /// One thin bar per sample.
    Bars,
    /// A flat value between samples.
    Steps,
    /// The dotted grid of the first design.
    Dots,
}

impl ChartStyle {
    /// Every style, in the order of the selector.
    pub const ALL: [ChartStyle; 4] = [
        ChartStyle::Line,
        ChartStyle::Bars,
        ChartStyle::Steps,
        ChartStyle::Dots,
    ];
}

/// How the Cores screen draws the processors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoresView {
    /// Usage and temperature bars, one row per core, the default.
    #[default]
    Bars,
    /// A heat map of usage over time.
    Heatmap,
    /// A small chart per core.
    Charts,
    /// Tiles laid out as a processor map.
    Map,
}

impl CoresView {
    /// Every view, in the order of the selector.
    pub const ALL: [CoresView; 4] = [
        CoresView::Bars,
        CoresView::Heatmap,
        CoresView::Charts,
        CoresView::Map,
    ];
}

/// The temperature unit, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureChoice {
    /// Degrees Celsius.
    #[default]
    Celsius,
    /// Degrees Fahrenheit.
    Fahrenheit,
}

impl From<TemperatureChoice> for TemperatureUnit {
    fn from(choice: TemperatureChoice) -> Self {
        match choice {
            TemperatureChoice::Celsius => TemperatureUnit::Celsius,
            TemperatureChoice::Fahrenheit => TemperatureUnit::Fahrenheit,
        }
    }
}

/// The chart range, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum RangeChoice {
    /// One minute.
    #[serde(rename = "1m")]
    OneMinute,
    /// Five minutes.
    #[default]
    #[serde(rename = "5m")]
    FiveMinutes,
    /// Fifteen minutes.
    #[serde(rename = "15m")]
    FifteenMinutes,
    /// One hour.
    #[serde(rename = "1h")]
    OneHour,
    /// Six hours.
    #[serde(rename = "6h")]
    SixHours,
    /// A day.
    #[serde(rename = "24h")]
    Day,
    /// A week.
    #[serde(rename = "7d")]
    Week,
}

impl From<RangeChoice> for Range {
    fn from(choice: RangeChoice) -> Self {
        match choice {
            RangeChoice::OneMinute => Range::OneMinute,
            RangeChoice::FiveMinutes => Range::FiveMinutes,
            RangeChoice::FifteenMinutes => Range::FifteenMinutes,
            RangeChoice::OneHour => Range::OneHour,
            RangeChoice::SixHours => Range::SixHours,
            RangeChoice::Day => Range::Day,
            RangeChoice::Week => Range::Week,
        }
    }
}

impl From<Range> for RangeChoice {
    fn from(range: Range) -> Self {
        match range {
            Range::OneMinute => RangeChoice::OneMinute,
            Range::FiveMinutes => RangeChoice::FiveMinutes,
            Range::FifteenMinutes => RangeChoice::FifteenMinutes,
            Range::OneHour => RangeChoice::OneHour,
            Range::SixHours => RangeChoice::SixHours,
            Range::Day => RangeChoice::Day,
            Range::Week => RangeChoice::Week,
        }
    }
}

/// How long the minute history is kept on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum HistoryKeep {
    /// Only the last hour, in memory; nothing is written.
    #[serde(rename = "1h")]
    OneHour,
    /// A day, the default.
    #[default]
    #[serde(rename = "24h")]
    Day,
    /// A week.
    #[serde(rename = "7d")]
    Week,
}

impl HistoryKeep {
    /// Every choice, shortest first.
    pub const ALL: [HistoryKeep; 3] = [HistoryKeep::OneHour, HistoryKeep::Day, HistoryKeep::Week];

    /// The minutes kept on disk, 0 for none.
    pub fn minutes(self) -> usize {
        match self {
            HistoryKeep::OneHour => 0,
            HistoryKeep::Day => 24 * 60,
            HistoryKeep::Week => 7 * 24 * 60,
        }
    }

    /// Whether the charts may show `range`.
    pub fn allows(self, range: Range) -> bool {
        range.secs() <= (self.minutes() as f64 * 60.0).max(3600.0)
    }

    fn code(self) -> &'static str {
        match self {
            HistoryKeep::OneHour => "1h",
            HistoryKeep::Day => "24h",
            HistoryKeep::Week => "7d",
        }
    }
}

/// Bounds of the price of a kWh.
pub const PRICE_RANGE: std::ops::RangeInclusive<f32> = 0.0..=5.0;
/// Step of the price stepper.
pub const PRICE_STEP: f32 = 0.01;

/// Every setting.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    /// The UI language.
    pub language: LanguageChoice,
    /// The theme mode.
    pub theme: ThemeChoice,
    /// How often the machine is read.
    pub interval: Interval,
    /// How long the minute history is kept.
    pub history_keep: HistoryKeep,
    /// How every chart is drawn.
    pub chart_style: ChartStyle,
    /// The range every chart shows.
    pub range: RangeChoice,
    /// How the Cores screen draws the processors.
    pub cores_view: CoresView,
    /// °C or °F.
    pub temperature_unit: TemperatureChoice,
    /// The price of a kWh, for the cost of the energy used.
    pub energy_price: f32,
    /// The currency symbol written after the cost.
    pub currency: String,
    /// Start Latexo when the user logs in.
    pub start_at_login: bool,
    /// Look for a new version at startup (only in builds with the updater).
    pub check_updates: bool,
    /// The log level.
    pub log_level: LogLevel,
    /// The alert rules.
    #[serde(rename = "alert")]
    pub alerts: Vec<Rule>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LanguageChoice::default(),
            theme: ThemeChoice::default(),
            interval: Interval::default(),
            history_keep: HistoryKeep::default(),
            chart_style: ChartStyle::default(),
            range: RangeChoice::default(),
            cores_view: CoresView::default(),
            temperature_unit: TemperatureChoice::default(),
            energy_price: 0.16,
            currency: "€".to_string(),
            start_at_login: false,
            check_updates: true,
            log_level: LogLevel::default(),
            alerts: default_rules(),
        }
    }
}

/// Loads the settings, or the defaults when the folder is unknown, the file is missing or it is
/// not valid TOML.
pub fn load_settings() -> Settings {
    config_dir().map_or_else(Settings::default, |base| read_settings(&base))
}

/// Writes the settings. Failures are logged: the change stays applied for this session.
pub fn save_settings(settings: &Settings) {
    if let Some(base) = config_dir() {
        write_settings(&base, settings);
    }
}

/// The path of the settings file, `None` when the config folder is unknown.
pub fn settings_path() -> Option<PathBuf> {
    config_dir().map(|base| base.join(SETTINGS_FILE))
}

fn write_settings(base: &Path, settings: &Settings) {
    if let Err(error) = fs::create_dir_all(base) {
        log::warn!("could not create {}: {error}", base.display());
        return;
    }
    let text = match toml::to_string_pretty(settings) {
        Ok(text) => text,
        Err(error) => {
            log::warn!("could not serialize the settings: {error}");
            return;
        }
    };
    let path = base.join(SETTINGS_FILE);
    if let Err(error) = fs::write(&path, text) {
        log::warn!("could not save {}: {error}", path.display());
    }
}

fn read_settings(base: &Path) -> Settings {
    match fs::read_to_string(base.join(SETTINGS_FILE)) {
        Ok(content) => parse_settings(&content),
        Err(_) => Settings::default(),
    }
}

/// One enum field: the variant whose stored name is `text`, case-insensitive.
fn parse_choice<T: Copy>(text: &str, all: &[T], name: impl Fn(T) -> &'static str) -> Option<T> {
    let text = text.trim().to_ascii_lowercase();
    all.iter().copied().find(|choice| name(*choice) == text)
}

fn parse_settings(content: &str) -> Settings {
    let mut settings = Settings::default();
    let Ok(table) = content.parse::<toml::Table>() else {
        log::warn!("settings.toml is not valid TOML, using the defaults");
        return settings;
    };
    let text = |key: &str| table.get(key).and_then(toml::Value::as_str);
    let flag = |key: &str| table.get(key).and_then(toml::Value::as_bool);
    if let Some(value) = text("language").and_then(LanguageChoice::parse) {
        settings.language = value;
    }
    if let Some(value) = text("theme").and_then(|t| {
        parse_choice(
            t,
            &[ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark],
            |c| match c {
                ThemeChoice::System => "system",
                ThemeChoice::Light => "light",
                ThemeChoice::Dark => "dark",
            },
        )
    }) {
        settings.theme = value;
    }
    if let Some(value) =
        text("interval").and_then(|t| parse_choice(t, &Interval::ALL, Interval::code))
    {
        settings.interval = value;
    }
    if let Some(value) =
        text("history_keep").and_then(|t| parse_choice(t, &HistoryKeep::ALL, HistoryKeep::code))
    {
        settings.history_keep = value;
    }
    if let Some(value) = text("chart_style").and_then(|t| {
        parse_choice(t, &ChartStyle::ALL, |c| match c {
            ChartStyle::Line => "line",
            ChartStyle::Bars => "bars",
            ChartStyle::Steps => "steps",
            ChartStyle::Dots => "dots",
        })
    }) {
        settings.chart_style = value;
    }
    if let Some(value) = text("range").and_then(|t| {
        let all = [
            RangeChoice::OneMinute,
            RangeChoice::FiveMinutes,
            RangeChoice::FifteenMinutes,
            RangeChoice::OneHour,
            RangeChoice::SixHours,
            RangeChoice::Day,
            RangeChoice::Week,
        ];
        parse_choice(t, &all, |c| match c {
            RangeChoice::OneMinute => "1m",
            RangeChoice::FiveMinutes => "5m",
            RangeChoice::FifteenMinutes => "15m",
            RangeChoice::OneHour => "1h",
            RangeChoice::SixHours => "6h",
            RangeChoice::Day => "24h",
            RangeChoice::Week => "7d",
        })
    }) {
        settings.range = value;
    }
    if let Some(value) = text("cores_view").and_then(|t| {
        parse_choice(t, &CoresView::ALL, |c| match c {
            CoresView::Bars => "bars",
            CoresView::Heatmap => "heatmap",
            CoresView::Charts => "charts",
            CoresView::Map => "map",
        })
    }) {
        settings.cores_view = value;
    }
    if let Some(value) = text("temperature_unit").and_then(|t| {
        parse_choice(
            t,
            &[TemperatureChoice::Celsius, TemperatureChoice::Fahrenheit],
            |c| match c {
                TemperatureChoice::Celsius => "celsius",
                TemperatureChoice::Fahrenheit => "fahrenheit",
            },
        )
    }) {
        settings.temperature_unit = value;
    }
    if let Some(value) = table.get("energy_price").and_then(as_f32)
        && PRICE_RANGE.contains(&value)
    {
        settings.energy_price = value;
    }
    if let Some(value) = text("currency").filter(|c| !c.trim().is_empty() && c.len() <= 8) {
        settings.currency = value.trim().to_string();
    }
    if let Some(value) = flag("start_at_login") {
        settings.start_at_login = value;
    }
    if let Some(value) = flag("check_updates") {
        settings.check_updates = value;
    }
    if let Some(value) = text("log_level").and_then(LogLevel::parse) {
        settings.log_level = value;
    }
    if let Some(rules) = table.get("alert").and_then(toml::Value::as_array) {
        settings.alerts = rules
            .iter()
            .filter_map(|rule| rule.clone().try_into::<Rule>().ok())
            .collect();
    }
    if !settings.history_keep.allows(settings.range.into()) {
        settings.range = RangeChoice::OneHour;
    }
    settings
}

fn as_f32(value: &toml::Value) -> Option<f32> {
    value
        .as_float()
        .map(|v| v as f32)
        .or_else(|| value.as_integer().map(|v| v as f32))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_missing_or_broken_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_settings(dir.path()), Settings::default());
        assert_eq!(parse_settings("not = [toml"), Settings::default());
    }

    #[test]
    fn saved_settings_read_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            language: LanguageChoice::Gl,
            theme: ThemeChoice::Dark,
            interval: Interval::TwoSeconds,
            history_keep: HistoryKeep::Week,
            chart_style: ChartStyle::Steps,
            range: RangeChoice::OneHour,
            cores_view: CoresView::Map,
            temperature_unit: TemperatureChoice::Fahrenheit,
            energy_price: 0.25,
            currency: "$".to_string(),
            start_at_login: true,
            check_updates: false,
            log_level: LogLevel::Debug,
            alerts: default_rules().into_iter().take(2).collect(),
        };
        write_settings(dir.path(), &settings);
        assert_eq!(read_settings(dir.path()), settings);
    }

    #[test]
    fn each_field_falls_back_on_its_own() {
        let settings = parse_settings(
            "theme = \"purple\"\ninterval = \"5s\"\nenergy_price = 99.0\ncurrency = \"\"\n",
        );
        assert_eq!(settings.theme, ThemeChoice::System);
        assert_eq!(settings.interval, Interval::FiveSeconds);
        assert_eq!(settings.energy_price, 0.16);
        assert_eq!(settings.currency, "€");
        assert_eq!(settings.alerts, default_rules());
    }

    #[test]
    fn an_empty_alert_list_stays_empty_and_bad_rules_are_skipped() {
        let settings = parse_settings("alert = []\n");
        assert!(settings.alerts.is_empty());
        let settings = parse_settings(
            "[[alert]]\nenabled = true\nmetric = \"cpu_usage\"\nthreshold = 90.0\nduration_secs = 10\n\n[[alert]]\nmetric = \"nope\"\n",
        );
        assert_eq!(settings.alerts.len(), 1);
    }
}
