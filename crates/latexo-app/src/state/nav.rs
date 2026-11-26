//! The screens of the side bar, and the `LATEXO_OPEN` debug hook that starts on one of them.

/// One screen of the side bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    /// The cards of every resource, the default.
    #[default]
    Overview,
    /// The process table.
    Processes,
    /// The processor.
    Cpu,
    /// Each core.
    Cores,
    /// The graphics cards.
    Gpu,
    /// Memory.
    Memory,
    /// Disks.
    Disk,
    /// The network.
    Network,
    /// Temperature sensors.
    Temperatures,
    /// Fans.
    Fans,
    /// Power and energy.
    Energy,
    /// Alert rules and their history.
    Alerts,
    /// Settings, inside the window.
    Settings,
    /// About Latexo.
    About,
}

impl Screen {
    /// The screens of the top group of the side bar.
    pub const TOP: [Screen; 2] = [Screen::Overview, Screen::Processes];
    /// The hardware screens.
    pub const HARDWARE: [Screen; 9] = [
        Screen::Cpu,
        Screen::Cores,
        Screen::Gpu,
        Screen::Memory,
        Screen::Disk,
        Screen::Network,
        Screen::Temperatures,
        Screen::Fans,
        Screen::Energy,
    ];
    /// The system screens.
    pub const SYSTEM: [Screen; 2] = [Screen::Alerts, Screen::Settings];

    /// The name used by the debug hook and in the logs.
    pub fn code(self) -> &'static str {
        match self {
            Screen::Overview => "overview",
            Screen::Processes => "processes",
            Screen::Cpu => "cpu",
            Screen::Cores => "cores",
            Screen::Gpu => "gpu",
            Screen::Memory => "memory",
            Screen::Disk => "disk",
            Screen::Network => "network",
            Screen::Temperatures => "temperatures",
            Screen::Fans => "fans",
            Screen::Energy => "energy",
            Screen::Alerts => "alerts",
            Screen::Settings => "settings",
            Screen::About => "about",
        }
    }

    /// The screen named `code`, case-insensitive.
    pub fn parse(code: &str) -> Option<Self> {
        let code = code.trim().to_ascii_lowercase();
        Self::TOP
            .into_iter()
            .chain(Self::HARDWARE)
            .chain(Self::SYSTEM)
            .chain([Screen::About])
            .find(|screen| screen.code() == code)
    }

    /// Whether the screen lists processes (the busiest ones, the top users of memory, disk or
    /// GPU), so the sampler reads them on every sample.
    pub fn shows_processes(self) -> bool {
        matches!(
            self,
            Screen::Overview
                | Screen::Processes
                | Screen::Cpu
                | Screen::Memory
                | Screen::Disk
                | Screen::Gpu
        )
    }

    /// Whether the header shows the chart style and range selectors.
    pub fn has_charts(self) -> bool {
        !matches!(
            self,
            Screen::Processes | Screen::Alerts | Screen::Settings | Screen::About
        )
    }
}

/// The screen of the `LATEXO_OPEN` debug hook, `None` when it is unset or unknown.
pub fn startup_screen() -> Option<Screen> {
    let value = std::env::var("LATEXO_OPEN").ok()?;
    let screen = Screen::parse(&value);
    if screen.is_none() && !opens_flyout() {
        log::warn!("ignoring LATEXO_OPEN={value:?}: not a screen");
    }
    screen
}

/// Whether the `LATEXO_OPEN=flyout` debug hook asks for the tray flyout at startup.
pub fn opens_flyout() -> bool {
    std::env::var("LATEXO_OPEN").is_ok_and(|value| value.trim().eq_ignore_ascii_case("flyout"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_parses_back_from_its_code() {
        for screen in Screen::TOP
            .into_iter()
            .chain(Screen::HARDWARE)
            .chain(Screen::SYSTEM)
            .chain([Screen::About])
        {
            assert_eq!(Screen::parse(screen.code()), Some(screen));
        }
        assert_eq!(Screen::parse(" CPU "), Some(Screen::Cpu));
        assert_eq!(Screen::parse("nope"), None);
    }
}
