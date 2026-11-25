//! Latexo's visual foundations on top of telaio: telaio's default tokens (the design uses them
//! unchanged), the two derived colors of the charts ([`LatexoColors`]) and the layout metrics.

pub mod metrics;

pub use telaio_ui::{Palette, PaletteExt};

use gpui_kit::{App, Hsla};

/// Loads the "Latexo Light" and "Latexo Dark" themes. Call once, after `window::init`, before
/// opening the window.
pub fn install(cx: &mut App) {
    telaio_ui::install(cx, &theme_family());
}

fn theme_family() -> telaio_ui::ThemeFamily {
    telaio_ui::ThemeFamily::new("Latexo")
}

/// The colors the charts add to telaio's palette, derived from it as the design does.
pub trait LatexoColors {
    /// The second series of a chart (system time, writes, upload): the accent, translucent.
    fn series_secondary(&self) -> Hsla;
    /// An empty cell of the dotted charts and the heat map.
    fn dot_off(&self) -> Hsla;
    /// The color of a temperature: the accent, warning from 80 °C, danger from 95 °C.
    fn temperature(&self, celsius: f32) -> Hsla;
}

impl LatexoColors for Palette {
    fn series_secondary(&self) -> Hsla {
        self.accent.opacity(if self.dark { 0.45 } else { 0.42 })
    }

    fn dot_off(&self) -> Hsla {
        if self.dark {
            self.border.blend(self.surface.opacity(0.3))
        } else {
            self.border.blend(self.bg.opacity(0.4))
        }
    }

    fn temperature(&self, celsius: f32) -> Hsla {
        if celsius >= 95.0 {
            self.danger
        } else if celsius >= 80.0 {
            self.warning
        } else {
            self.accent
        }
    }
}
