//! How each screen builds its charts from the history: the series, its scale and the colors of
//! the palette, in the chart style and range of the settings.

use latexo_core::SeriesKey;

use crate::theme::{LatexoColors, Palette};

use super::chart::Chart;
use super::root::AppView;

/// A round ceiling for an automatic scale: 1, 2 or 5 times a power of ten, at least `floor`.
pub fn nice_ceiling(value: f32, floor: f32) -> f32 {
    let value = value.max(floor);
    if !value.is_finite() || value <= 0.0 {
        return floor.max(1.0);
    }
    let magnitude = 10f32.powf(value.log10().floor());
    for step in [1.0, 2.0, 5.0, 10.0] {
        let candidate = step * magnitude;
        if candidate >= value {
            return candidate;
        }
    }
    10.0 * magnitude
}

/// [`nice_ceiling`] for bytes: round in the binary unit the value is shown in, so the axis reads
/// `1 MB/s` or `500 KB/s` rather than `954 KB/s`.
pub fn nice_bytes_ceiling(value: f32, floor: f32) -> f32 {
    let value = value.max(floor).max(1.0);
    let mut unit = 1.0f32;
    while value / unit >= 1000.0 {
        unit *= 1024.0;
    }
    nice_ceiling(value / unit, 1.0) * unit
}

impl AppView {
    /// The values of `key` in the chart range.
    pub fn window(&self, key: &SeriesKey) -> Vec<f32> {
        let range = self.range();
        if range.is_long() {
            self.monitor.minutes.window(key, range)
        } else {
            self.monitor.history.window(key, range)
        }
    }

    /// The highest value of `key` in the chart range, from the history the chart reads.
    pub fn max_in_range(&self, key: &SeriesKey) -> Option<f32> {
        let range = self.range();
        if range.is_long() {
            self.monitor.minutes.max_in(key, range)
        } else {
            self.monitor.history.max_in(key, range)
        }
    }

    /// A chart of one series from 0 to `max`.
    pub fn chart(&self, key: &SeriesKey, max: f32, height: f32, palette: &Palette) -> Chart {
        self.chart_of(self.window(key), max, height, palette)
    }

    /// A chart of `values` from 0 to `max`.
    pub fn chart_of(&self, values: Vec<f32>, max: f32, height: f32, palette: &Palette) -> Chart {
        Chart::new(values, self.slots(), max, self.settings.chart_style)
            .height(height)
            .colors(
                palette.accent,
                palette.series_secondary(),
                palette.dot_off(),
            )
            .grid(palette.border)
    }

    /// A chart of two stacked series from 0 to `max`.
    pub fn stacked_chart(
        &self,
        first: &SeriesKey,
        second: &SeriesKey,
        max: f32,
        height: f32,
        palette: &Palette,
    ) -> Chart {
        self.chart(first, max, height, palette)
            .stacked(self.window(second))
    }

    /// The automatic scale of two stacked rate series: their highest sum in the range.
    pub fn stacked_max(&self, first: &SeriesKey, second: &SeriesKey, floor: f32) -> f32 {
        let a = self.window(first);
        let b = self.window(second);
        let peak = a
            .iter()
            .zip(b.iter())
            .map(|(a, b)| a + b)
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32::max);
        nice_bytes_ceiling(peak * 1.1, floor)
    }

    /// The automatic scale of one series.
    pub fn auto_max(&self, key: &SeriesKey, floor: f32) -> f32 {
        let peak = self
            .window(key)
            .into_iter()
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32::max);
        nice_ceiling(peak * 1.1, floor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_are_round() {
        assert_eq!(nice_ceiling(0.0, 1.0), 1.0);
        assert_eq!(nice_ceiling(3.2, 1.0), 5.0);
        assert_eq!(nice_ceiling(120.0, 1.0), 200.0);
        assert_eq!(nice_ceiling(1000.0, 1.0), 1000.0);
        assert_eq!(nice_ceiling(7.0e6, 1.0), 1.0e7);
    }

    #[test]
    fn byte_ceilings_are_round_in_binary_units() {
        assert_eq!(nice_bytes_ceiling(900.0 * 1024.0, 1.0), 1000.0 * 1024.0);
        assert_eq!(
            nice_bytes_ceiling(3.0 * 1024.0 * 1024.0, 1.0),
            5.0 * 1024.0 * 1024.0
        );
    }
}
