//! Axis ranges and the arithmetic for panning, zooming and edge dragging.
//!
//! Functions accept anything convertible into [`Range`] or [`Ranges`], so
//! `(min, max)` and `((x_min, x_max), (y_min, y_max))` tuples work directly.

use crate::PlotGeometry;

/// Closed interval of data values on one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    /// Range from `min` to `max`; see [`valid`] to check it.
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// `max - min`.
    pub fn span(self) -> f64 {
        self.max - self.min
    }

    /// Whether `value` lies in `min..=max`.
    pub fn contains(self, value: f64) -> bool {
        (self.min..=self.max).contains(&value)
    }

    /// Widened on both sides by `fraction` of the span, or by `flat` when the
    /// range is a single value.
    pub fn expand(self, fraction: f64, flat: f64) -> Self {
        let pad = if self.max > self.min {
            self.span() * fraction
        } else {
            flat
        };
        Self::new(self.min - pad, self.max + pad)
    }
}

impl From<(f64, f64)> for Range {
    fn from((min, max): (f64, f64)) -> Self {
        Self { min, max }
    }
}

impl From<Range> for (f64, f64) {
    fn from(range: Range) -> Self {
        (range.min, range.max)
    }
}

/// Visible data ranges of both axes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ranges {
    pub x: Range,
    pub y: Range,
}

impl Ranges {
    /// Ranges from anything convertible, such as `(min, max)` tuples.
    pub fn new(x: impl Into<Range>, y: impl Into<Range>) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

impl<X: Into<Range>, Y: Into<Range>> From<(X, Y)> for Ranges {
    fn from((x, y): (X, Y)) -> Self {
        Self::new(x, y)
    }
}

impl From<Ranges> for ((f64, f64), (f64, f64)) {
    fn from(ranges: Ranges) -> Self {
        (ranges.x.into(), ranges.y.into())
    }
}

/// `range` when both ends are finite and strictly increasing.
pub fn valid(range: impl Into<Range>) -> Option<Range> {
    let range = range.into();
    (range.min.is_finite()
        && range.max.is_finite()
        && range.span().is_finite()
        && range.min < range.max)
        .then_some(range)
}

/// Shift a range by `delta` data units.
pub fn pan(range: impl Into<Range>, delta: f64) -> Option<Range> {
    let range = range.into();
    valid((range.min + delta, range.max + delta))
}

/// Scale a range by `factor`, keeping the point at `anchor` (0 = min, 1 = max) fixed.
pub fn zoom(range: impl Into<Range>, anchor: f64, factor: f64) -> Option<Range> {
    let range = range.into();
    let span = range.span() * factor;
    let center = range.min + anchor * range.span();
    valid((center - anchor * span, center + (1.0 - anchor) * span))
}

/// Which end of a range an edge drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Min,
    Max,
}

/// Move one end of a range by `delta`, rejecting inverted results.
pub fn move_edge(range: impl Into<Range>, delta: f64, edge: Edge) -> Option<Range> {
    let range = range.into();
    let next = match edge {
        Edge::Min => Range::new(range.min + delta, range.max),
        Edge::Max => Range::new(range.min, range.max + delta),
    };
    (next.min.is_finite() && next.max.is_finite() && next.min < next.max).then_some(next)
}

/// Zoom factor for a scroll of `pixels`, continuous so fractional trackpad
/// deltas compose to the same result as whole wheel notches.
pub fn wheel_zoom_factor(pixels: f64) -> f64 {
    if !pixels.is_finite() {
        return 1.0;
    }
    (pixels.clamp(-240.0, 240.0) * 0.005).exp()
}

/// Smallest range holding every finite value, or `None` if there are none.
pub fn extent(values: impl IntoIterator<Item = f64>) -> Option<Range> {
    values.into_iter().filter(|value| value.is_finite()).fold(
        None,
        |extent: Option<Range>, value| {
            Some(extent.map_or(Range::new(value, value), |range| {
                Range::new(range.min.min(value), range.max.max(value))
            }))
        },
    )
}

/// Range covering `values` with `pad_fraction` of the span added on both
/// sides. A single repeated value is padded by that fraction of its
/// magnitude, zero by 1; no finite values gives `0..1`.
pub fn padded_bounds(values: impl IntoIterator<Item = f64>, pad_fraction: f64) -> Range {
    extent(values).map_or(Range::new(0.0, 1.0), |range| {
        let flat = if range.min != 0.0 {
            range.min.abs() * pad_fraction
        } else {
            1.0
        };
        range.expand(pad_fraction, flat)
    })
}

/// Data ranges covered by a screen-space selection between two corners given
/// in either order, clamped to the plot's data area.
pub fn selection(
    ranges: impl Into<Ranges>,
    ((x0, x1), (y0, y1)): ((f64, f64), (f64, f64)),
    geometry: PlotGeometry,
) -> Option<Ranges> {
    let ranges = ranges.into();
    if geometry.data_width() <= 0.0 || geometry.data_height() <= 0.0 {
        return None;
    }
    let x = |p: f64| geometry.x_value(p.clamp(geometry.left, geometry.right), ranges.x);
    let y = |p: f64| geometry.y_value(p.clamp(geometry.top, geometry.bottom), ranges.y);
    Some(Ranges {
        x: valid((x(x0.min(x1)), x(x0.max(x1))))?,
        y: valid((y(y0.max(y1)), y(y0.min(y1))))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(min: f64, max: f64) -> Range {
        Range::new(min, max)
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        assert_eq!(zoom((0.0, 10.0), 0.25, 2.0), Some(r(-2.5, 17.5)));
        assert_eq!(zoom((0.0, 10.0), 0.25, 0.5), Some(r(1.25, 6.25)));
        assert_eq!(zoom((0.0, 10.0), 0.5, f64::INFINITY), None);
    }

    #[test]
    fn edges_move_independently_and_never_invert() {
        assert_eq!(move_edge((0.0, 10.0), 2.0, Edge::Min), Some(r(2.0, 10.0)));
        assert_eq!(move_edge((0.0, 10.0), 2.0, Edge::Max), Some(r(0.0, 12.0)));
        assert_eq!(move_edge((0.0, 10.0), 11.0, Edge::Min), None);
    }

    #[test]
    fn pan_rejects_non_finite_ranges() {
        assert_eq!(pan((0.0, 1.0), 2.0), Some(r(2.0, 3.0)));
        assert_eq!(pan((0.0, 1.0), f64::INFINITY), None);
    }

    #[test]
    fn padded_bounds_handle_empty_constant_and_spread_values() {
        assert_eq!(padded_bounds([], 0.05), r(0.0, 1.0));
        assert_eq!(padded_bounds([f64::NAN], 0.05), r(0.0, 1.0));
        assert_eq!(padded_bounds([0.0, 0.0], 0.05), r(-1.0, 1.0));
        assert_eq!(padded_bounds([10.0], 0.5), r(5.0, 15.0));
        assert_eq!(padded_bounds([0.0, 10.0], 0.1), r(-1.0, 11.0));
    }

    #[test]
    fn extent_skips_non_finite_and_expand_pads_flat_ranges() {
        assert_eq!(extent([3.0, f64::NAN, -1.0]), Some(r(-1.0, 3.0)));
        assert_eq!(extent([f64::INFINITY]), None);
        assert_eq!(r(0.0, 10.0).expand(0.05, 0.5), r(-0.5, 10.5));
        assert_eq!(r(10.0, 10.0).expand(0.05, 0.5), r(9.5, 10.5));
    }

    #[test]
    fn selection_maps_either_drag_direction_and_clamps() {
        let geometry = PlotGeometry::with_bounds(0.0, 0.0, 100.0, 100.0, 100.0, 100.0).unwrap();
        let ranges = ((0.0, 10.0), (0.0, 1.0));
        let expected = Some(Ranges::new((2.0, 5.0), (0.25, 0.75)));
        assert_eq!(
            selection(ranges, ((20.0, 50.0), (25.0, 75.0)), geometry),
            expected
        );
        assert_eq!(
            selection(ranges, ((50.0, 20.0), (75.0, 25.0)), geometry),
            expected
        );
        assert_eq!(
            selection(ranges, ((-50.0, 200.0), (-1.0, 101.0)), geometry),
            Some(ranges.into())
        );
    }

    #[test]
    fn wheel_factor_composes_and_inverts() {
        assert!((wheel_zoom_factor(2.4).powi(10) - wheel_zoom_factor(24.0)).abs() < 1e-8);
        assert!((wheel_zoom_factor(24.0) * wheel_zoom_factor(-24.0) - 1.0).abs() < 1e-12);
        assert_eq!(wheel_zoom_factor(f64::NAN), 1.0);
    }

    #[test]
    fn tuples_convert_both_ways() {
        let ranges: Ranges = ((0.0, 1.0), (2.0, 3.0)).into();
        assert_eq!(ranges.y, r(2.0, 3.0));
        let back: ((f64, f64), (f64, f64)) = ranges.into();
        assert_eq!(back, ((0.0, 1.0), (2.0, 3.0)));
    }
}
