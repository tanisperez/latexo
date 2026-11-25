//! [`Chart`]: Latexo's own chart, a domain widget. One series, or two stacked (user and system,
//! reads and writes), drawn in the four styles of the design: a line with its area, thin bars,
//! steps and the dotted grid. The newest sample is at the right edge; a history shorter than the
//! range leaves the left part empty instead of stretching.
//!
//! It never draws more points than it has pixels for: the values are averaged into buckets
//! ([`latexo_core::history::downsample`]) at paint time, when the width is known. The math is in
//! plain functions at the top, unit tested without a window.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::state::settings::ChartStyle;
use crate::theme::metrics::{DOT_PITCH, DOT_SIZE};

/// The most points per pixel of width the line styles draw.
const POINTS_PER_PX: f32 = 0.5;
/// The pitch of a bar of the bar style, in pixels, at most.
const BAR_PITCH: f32 = 4.0;
/// The gap between two bars.
const BAR_GAP: f32 = 1.0;
/// Width of the line strokes.
const STROKE: f32 = 1.5;
/// Dash and gap of the grid and of the threshold line.
const GRID_DASH: [f32; 2] = [2.0, 4.0];
const THRESHOLD_DASH: [f32; 2] = [4.0, 3.0];

/// The points of `values` once spread over `slots` time slots (the newest at the last slot) and
/// reduced to at most `points` buckets: each bucket's average and its slot position from 0 to 1.
/// `NaN` values stay gaps.
pub fn buckets(values: &[f32], slots: usize, points: usize) -> Vec<(f32, f32)> {
    let slots = slots.max(values.len()).max(2);
    let points = points.clamp(2, slots);
    // How many slots each bucket covers, rounded up so the buckets fit in `points`.
    let per = slots.div_ceil(points);
    let total_buckets = slots.div_ceil(per);
    let offset = slots - values.len();
    let mut out = Vec::new();
    for bucket in 0..total_buckets {
        // Buckets are aligned on the right edge: the last one ends at the newest slot.
        let end = slots.saturating_sub((total_buckets - 1 - bucket) * per);
        let start = end.saturating_sub(per);
        let in_values = start.max(offset)..end.max(offset);
        if in_values.is_empty() {
            continue;
        }
        let finite: Vec<f32> = values[in_values.start - offset..in_values.end - offset]
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        let value = if finite.is_empty() {
            f32::NAN
        } else {
            finite.iter().sum::<f32>() / finite.len() as f32
        };
        let x = (end - 1) as f32 / (slots - 1) as f32;
        out.push((x, value));
    }
    out
}

/// `value` from 0 to `max` as a height fraction, clamped.
fn fraction(value: f32, max: f32) -> f32 {
    if max <= 0.0 || !value.is_finite() {
        0.0
    } else {
        (value / max).clamp(0.0, 1.0)
    }
}

/// Bucketed points: position from 0 to 1 and value.
type Points = Vec<(f32, f32)>;

/// A chart, built per render.
#[derive(Clone)]
pub struct Chart {
    first: Vec<f32>,
    second: Option<Vec<f32>>,
    slots: usize,
    max: f32,
    height: f32,
    style: ChartStyle,
    first_color: Hsla,
    second_color: Hsla,
    grid_color: Option<Hsla>,
    threshold: Option<(f32, Hsla)>,
    dot_off: Hsla,
}

impl Chart {
    /// A chart of `values` (oldest first) over `slots` time slots, from 0 to `max`.
    pub fn new(values: Vec<f32>, slots: usize, max: f32, style: ChartStyle) -> Self {
        Self {
            first: values,
            second: None,
            slots,
            max,
            height: 48.0,
            style,
            first_color: gpui_kit::blue(),
            second_color: gpui_kit::blue().opacity(0.45),
            grid_color: None,
            threshold: None,
            dot_off: gpui_kit::transparent_black(),
        }
    }

    /// A second series stacked on the first.
    pub fn stacked(mut self, values: Vec<f32>) -> Self {
        self.second = Some(values);
        self
    }

    /// The height in pixels.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The colors of the two series and of an empty dot.
    pub fn colors(mut self, first: Hsla, second: Hsla, dot_off: Hsla) -> Self {
        self.first_color = first;
        self.second_color = second;
        self.dot_off = dot_off;
        self
    }

    /// Dashed lines at the bottom, middle and top.
    pub fn grid(mut self, color: Hsla) -> Self {
        self.grid_color = Some(color);
        self
    }

    /// A dashed line at `value`; values at or over it are drawn in `color` by the bars and dots.
    pub fn threshold(mut self, value: f32, color: Hsla) -> Self {
        self.threshold = Some((value, color));
        self
    }
}

impl IntoElement for Chart {
    type Element = Canvas<()>;

    fn into_element(self) -> Self::Element {
        let height = self.height;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.paint(bounds, window),
        )
        .w_full()
        .h(px(height))
    }
}

impl Chart {
    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width < 4.0 || height < 4.0 {
            return;
        }
        if let Some(color) = self.grid_color
            && self.style != ChartStyle::Dots
        {
            for fraction in [0.0, 0.5, 1.0] {
                let y = (height - 1.0) * (1.0 - fraction) + 0.5;
                dashed_line(bounds, y, GRID_DASH, color, window);
            }
        }
        match self.style {
            ChartStyle::Line | ChartStyle::Steps => self.paint_lines(bounds, window),
            ChartStyle::Bars => self.paint_bars(bounds, window),
            ChartStyle::Dots => self.paint_dots(bounds, window),
        }
        if let Some((value, color)) = self.threshold
            && self.style != ChartStyle::Dots
        {
            let y = (height - 1.0) * (1.0 - fraction(value, self.max));
            dashed_line(bounds, y.floor() + 0.5, THRESHOLD_DASH, color, window);
        }
    }

    /// The first series and the top of the stack, bucketed alike.
    fn series(&self, points: usize) -> (Points, Option<Points>) {
        let first = buckets(&self.first, self.slots, points);
        let top = self.second.as_ref().map(|second| {
            let sum: Vec<f32> = self
                .first
                .iter()
                .zip(second.iter())
                .map(|(a, b)| a + b)
                .collect();
            buckets(&sum, self.slots, points)
        });
        (first, top)
    }

    fn paint_lines(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let points = ((width * POINTS_PER_PX) as usize).max(2);
        let (first, top) = self.series(points);
        let steps = self.style == ChartStyle::Steps;
        let to_point = |(x, value): (f32, f32)| {
            point(
                bounds.origin.x + px(x * width),
                bounds.origin.y + px(1.0 + (height - 2.0) * (1.0 - fraction(value, self.max))),
            )
        };
        for run in runs(&first) {
            let line = shape(run.iter().copied().map(to_point).collect(), steps);
            if let Some(path) = area(&line, bounds.origin.y + bounds.size.height) {
                window.paint_path(path, self.first_color.opacity(0.2));
            }
            if let Some(path) = stroke(&line, STROKE) {
                window.paint_path(path, self.first_color);
            }
        }
        if let Some(top) = top {
            for (run_top, run_first) in runs(&top).into_iter().zip(runs(&first)) {
                let upper = shape(run_top.iter().copied().map(to_point).collect(), steps);
                let lower = shape(run_first.iter().copied().map(to_point).collect(), steps);
                if let Some(path) = band(&upper, &lower) {
                    window.paint_path(path, self.second_color);
                }
                if let Some(path) = stroke(&upper, STROKE) {
                    window.paint_path(path, self.first_color.opacity(0.5));
                }
            }
        }
    }

    fn paint_bars(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let count = ((width / BAR_PITCH) as usize).min(self.slots.max(2)).max(2);
        let pitch = width / count as f32;
        let bar = (pitch - BAR_GAP).max(1.0);
        let (first, top) = self.series(count);
        let bottom = bounds.origin.y + bounds.size.height;
        for (index, (x, value)) in first.iter().enumerate() {
            if !value.is_finite() {
                continue;
            }
            let left = bounds.origin.x + px((x * (width - bar)).round());
            let first_height = (height * fraction(*value, self.max)).max(1.0);
            let color = match self.threshold {
                Some((limit, hot)) if *value >= limit => hot,
                _ => self.first_color.opacity(0.9),
            };
            window.paint_quad(fill(
                Bounds::new(
                    point(left, bottom - px(first_height)),
                    size(px(bar), px(first_height)),
                ),
                color,
            ));
            if let Some((_, total)) = top.as_ref().and_then(|top| top.get(index)) {
                let total_height = height * fraction(*total, self.max);
                if total_height > first_height {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(left, bottom - px(total_height)),
                            size(px(bar), px(total_height - first_height)),
                        ),
                        self.second_color,
                    ));
                }
            }
        }
    }

    fn paint_dots(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let columns = ((width + DOT_PITCH - DOT_SIZE) / DOT_PITCH) as usize;
        let rows = (height / DOT_PITCH) as usize;
        if columns == 0 || rows == 0 {
            return;
        }
        // One column per bucket, the newest at the right edge.
        let slots = self.slots.max(columns);
        let points = columns.min(slots);
        let (first, top) = self.series(points);
        let lit = |value: f32| (fraction(value, self.max) * rows as f32).round() as usize;
        let bottom = bounds.origin.y + px(rows as f32 * DOT_PITCH);
        let right = bounds.origin.x + bounds.size.width;
        let values: Vec<(f32, Option<f32>)> = first
            .iter()
            .enumerate()
            .map(|(index, (_, value))| {
                (
                    *value,
                    top.as_ref().and_then(|top| top.get(index)).map(|(_, v)| *v),
                )
            })
            .collect();
        for column in 0..columns {
            // Column 0 is the rightmost.
            let x = right - px(column as f32 * DOT_PITCH + DOT_SIZE);
            let entry = values.len().checked_sub(column + 1).map(|i| values[i]);
            let (first_rows, top_rows, hot) = match entry {
                Some((value, total)) if value.is_finite() => {
                    let first_rows = lit(value).max(1).min(rows);
                    let top_rows = total
                        .filter(|t| t.is_finite())
                        .map_or(first_rows, |t| lit(t).clamp(first_rows, rows));
                    let hot = self
                        .threshold
                        .filter(|(limit, _)| value >= *limit)
                        .map(|(_, color)| color);
                    (first_rows, top_rows, hot)
                }
                _ => (0, 0, None),
            };
            for row in 0..rows {
                let color = if row < first_rows {
                    hot.unwrap_or(self.first_color)
                } else if row < top_rows {
                    self.second_color
                } else {
                    self.dot_off
                };
                let y = bottom - px((row + 1) as f32 * DOT_PITCH) + px(DOT_PITCH - DOT_SIZE);
                window.paint_quad(fill(
                    Bounds::new(point(x, y), size(px(DOT_SIZE), px(DOT_SIZE))),
                    color,
                ));
            }
        }
    }
}

/// The runs of finite values, as separate lists.
fn runs(points: &[(f32, f32)]) -> Vec<Vec<(f32, f32)>> {
    let mut runs = Vec::new();
    let mut current = Vec::new();
    for point in points {
        if point.1.is_finite() {
            current.push(*point);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// The points as drawn: straight, or with a flat step to the next one.
fn shape(points: Vec<Point<Pixels>>, steps: bool) -> Vec<Point<Pixels>> {
    if !steps {
        return points;
    }
    let mut out = Vec::with_capacity(points.len() * 2);
    for (index, current) in points.iter().enumerate() {
        out.push(*current);
        if let Some(next) = points.get(index + 1) {
            out.push(point(next.x, current.y));
        }
    }
    out
}

fn stroke(points: &[Point<Pixels>], width: f32) -> Option<Path<Pixels>> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(*first);
    for point in rest {
        builder.line_to(*point);
    }
    builder.build().ok()
}

fn area(points: &[Point<Pixels>], baseline: Pixels) -> Option<Path<Pixels>> {
    let (first, rest) = points.split_first()?;
    let last = points.last()?;
    if rest.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::fill();
    builder.move_to(point(first.x, baseline));
    builder.line_to(*first);
    for point in rest {
        builder.line_to(*point);
    }
    builder.line_to(point(last.x, baseline));
    builder.close();
    builder.build().ok()
}

/// The band between `upper` and `lower`, which run over the same x positions.
fn band(upper: &[Point<Pixels>], lower: &[Point<Pixels>]) -> Option<Path<Pixels>> {
    let (first, rest) = upper.split_first()?;
    if rest.is_empty() || lower.len() < 2 {
        return None;
    }
    let mut builder = PathBuilder::fill();
    builder.move_to(*first);
    for point in rest {
        builder.line_to(*point);
    }
    for point in lower.iter().rev() {
        builder.line_to(*point);
    }
    builder.close();
    builder.build().ok()
}

fn dashed_line(bounds: Bounds<Pixels>, y: f32, dash: [f32; 2], color: Hsla, window: &mut Window) {
    let y = bounds.origin.y + px(y);
    let mut builder = PathBuilder::stroke(px(1.0)).dash_array(&[px(dash[0]), px(dash[1])]);
    builder.move_to(point(bounds.origin.x, y));
    builder.line_to(point(bounds.origin.x + bounds.size.width, y));
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::{buckets, fraction, runs};
    use pretty_assertions::assert_eq;

    #[test]
    fn a_short_history_sits_at_the_right_edge() {
        let points = buckets(&[1.0, 2.0], 5, 10);
        assert_eq!(points, vec![(0.75, 1.0), (1.0, 2.0)]);
    }

    #[test]
    fn a_long_history_is_averaged_into_buckets() {
        let values: Vec<f32> = (0..8).map(|v| v as f32).collect();
        let points = buckets(&values, 8, 4);
        assert_eq!(points.len(), 4);
        assert_eq!(points[0].1, 0.5);
        assert_eq!(points[3], (1.0, 6.5));
    }

    #[test]
    fn gaps_split_the_runs() {
        let points = [(0.0, 1.0), (0.5, f32::NAN), (1.0, 2.0)];
        assert_eq!(runs(&points).len(), 2);
    }

    #[test]
    fn fractions_are_clamped() {
        assert_eq!(fraction(2.0, 1.0), 1.0);
        assert_eq!(fraction(f32::NAN, 1.0), 0.0);
        assert_eq!(fraction(1.0, 0.0), 0.0);
    }
}
