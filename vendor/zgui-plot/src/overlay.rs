//! Reference lines, cursors, shaded regions and masks drawn over plot data.

use kurbo::BezPath;

use crate::{PlotGeometry, PlotPoint, Range, Ranges, append_line, append_rect};

/// The line `y = slope * x + intercept` clipped to the visible data area.
pub fn fit_line(
    slope: f64,
    intercept: f64,
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
) -> BezPath {
    let Ranges {
        x: x_range,
        y: y_range,
    } = ranges.into();
    let (mut lo, mut hi) = (x_range.min, x_range.max);
    if slope == 0.0 {
        if !y_range.contains(intercept) {
            return BezPath::new();
        }
    } else {
        let a = (y_range.min - intercept) / slope;
        let b = (y_range.max - intercept) / slope;
        lo = lo.max(a.min(b));
        hi = hi.min(a.max(b));
    }
    if lo > hi {
        return BezPath::new();
    }
    let mut path = BezPath::new();
    path.move_to((
        geometry.x(lo, x_range),
        geometry.y(slope * lo + intercept, y_range),
    ));
    path.line_to((
        geometry.x(hi, x_range),
        geometry.y(slope * hi + intercept, y_range),
    ));
    path
}

/// Full-span horizontal and vertical lines through `center` (screen
/// coordinates), plus a square marker of half-size `marker` when given.
pub fn crosshair(center: (f64, f64), geometry: PlotGeometry, marker: Option<f64>) -> BezPath {
    let mut path = BezPath::new();
    append_line(
        &mut path,
        (geometry.left, center.1),
        (geometry.right, center.1),
    );
    append_line(
        &mut path,
        (center.0, geometry.top),
        (center.0, geometry.bottom),
    );
    if let Some(half) = marker {
        append_rect(
            &mut path,
            center.0 - half,
            center.1 - half,
            center.0 + half,
            center.1 + half,
        );
    }
    path
}

/// What the pointer is hovering: a data point it snapped to, or a free
/// cursor position in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HoverTarget {
    Point(PlotPoint),
    Cursor((f64, f64)),
}

/// Crosshair for a hover target. A snapped point also gets a square marker of
/// half-size `marker`; a point outside the visible ranges draws nothing.
pub fn hover_crosshair(
    target: HoverTarget,
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
    marker: f64,
) -> BezPath {
    let ranges = ranges.into();
    match target {
        HoverTarget::Point(point) => {
            if !ranges.x.contains(point.x) || !ranges.y.contains(point.y) {
                return BezPath::new();
            }
            let center = (geometry.x(point.x, ranges.x), geometry.y(point.y, ranges.y));
            crosshair(center, geometry, Some(marker))
        }
        HoverTarget::Cursor(center) => crosshair(center, geometry, None),
    }
}

/// Full-height vertical line at screen `x`, spanning the data area.
pub fn cursor_line(x: f64, geometry: PlotGeometry) -> BezPath {
    let mut path = BezPath::new();
    append_line(&mut path, (x, geometry.top), (x, geometry.bottom));
    path
}

/// Vertical dashes at screen `x` from `top` to `bottom`: `dash` long, repeating every `period`.
pub fn append_dashed_vline(
    path: &mut BezPath,
    x: f64,
    (top, bottom): (f64, f64),
    dash: f64,
    period: f64,
) {
    if period <= 0.0 {
        return;
    }
    let mut y = top;
    while y < bottom {
        append_line(path, (x, y), (x, (y + dash).min(bottom)));
        y += period;
    }
}

/// Horizontal dashes at screen `y` from `left` to `right`: `dash` long, repeating every `period`.
pub fn append_dashed_hline(
    path: &mut BezPath,
    y: f64,
    (left, right): (f64, f64),
    dash: f64,
    period: f64,
) {
    if period <= 0.0 {
        return;
    }
    let mut x = left;
    while x < right {
        append_line(path, (x, y), ((x + dash).min(right), y));
        x += period;
    }
}

/// Dashed full-height lines at each value inside `range` (6px dashes every
/// 10px), such as limits on the X axis.
pub fn vertical_value_lines(
    values: impl IntoIterator<Item = f64>,
    range: impl Into<Range>,
    geometry: PlotGeometry,
) -> BezPath {
    let range = range.into();
    let mut path = BezPath::new();
    if range.min >= range.max {
        return path;
    }
    for value in values.into_iter().filter(|value| range.contains(*value)) {
        let x = geometry.x(value, range);
        append_dashed_vline(&mut path, x, (geometry.top, geometry.bottom), 6.0, 10.0);
    }
    path
}

/// Dashed full-width lines at each value inside `range` (6px dashes every
/// 10px), such as limits on the Y axis.
pub fn horizontal_value_lines(
    values: impl IntoIterator<Item = f64>,
    range: impl Into<Range>,
    geometry: PlotGeometry,
) -> BezPath {
    let range = range.into();
    let mut path = BezPath::new();
    for value in values.into_iter().filter(|value| range.contains(*value)) {
        let y = geometry.y(value, range);
        append_dashed_hline(&mut path, y, (geometry.left, geometry.right), 6.0, 10.0);
    }
    path
}

/// Canvas area outside the data rectangle, as four non-overlapping rectangles.
/// Filling it with the background hides data that overflows the plot bounds
/// without clipping every shape individually.
pub fn margin_frame(geometry: PlotGeometry) -> BezPath {
    let PlotGeometry {
        width,
        height,
        left,
        top,
        right,
        bottom,
    } = geometry;
    let mut path = BezPath::new();
    append_rect(&mut path, 0.0, 0.0, width, top);
    append_rect(&mut path, 0.0, bottom, width, height);
    append_rect(&mut path, 0.0, top, left, bottom);
    append_rect(&mut path, right, top, width, bottom);
    path
}

/// Sort intervals and merge any that overlap or touch.
pub fn merge_intervals(intervals: impl IntoIterator<Item = Range>) -> Vec<Range> {
    let mut intervals = intervals.into_iter().collect::<Vec<_>>();
    intervals.sort_by(|a, b| a.min.total_cmp(&b.min));
    let mut merged: Vec<Range> = Vec::new();
    for interval in intervals {
        if let Some(last) = merged.last_mut()
            && interval.min <= last.max
        {
            last.max = last.max.max(interval.max);
        } else {
            merged.push(interval);
        }
    }
    merged
}

/// Non-overlapping rectangles covering full-height `x` bands and full-width
/// `y` bands. Expects merged, sorted bands. X bands are emitted whole; Y bands
/// only in the gaps between X bands, so no area is shaded twice.
pub fn band_rects(x: &[Range], y: &[Range], ranges: impl Into<Ranges>) -> Vec<Ranges> {
    let ranges = ranges.into();
    let mut rects = x
        .iter()
        .map(|band| Ranges {
            x: *band,
            y: ranges.y,
        })
        .collect::<Vec<_>>();
    let mut start = ranges.x.min;
    let end_marker = Range::new(ranges.x.max, ranges.x.max);
    for band in x.iter().copied().chain(std::iter::once(end_marker)) {
        let end = band.min.min(ranges.x.max);
        if start < end {
            rects.extend(y.iter().map(|y_band| Ranges::new((start, end), *y_band)));
        }
        start = band.max;
    }
    rects
}

/// Append data rectangles as screen-space rectangles.
pub fn append_data_rects(
    path: &mut BezPath,
    rects: impl IntoIterator<Item = Ranges>,
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
) {
    let ranges = ranges.into();
    for rect in rects {
        append_rect(
            path,
            geometry.x(rect.x.min, ranges.x),
            geometry.y(rect.y.max, ranges.y),
            geometry.x(rect.x.max, ranges.x),
            geometry.y(rect.y.min, ranges.y),
        );
    }
}

/// Axis line along the right edge of the data area with outward ticks of
/// `tick_length` at each fraction (0 = bottom, 1 = top) of its height.
pub fn right_axis(geometry: PlotGeometry, fractions: &[f64], tick_length: f64) -> BezPath {
    let mut path = BezPath::new();
    append_line(
        &mut path,
        (geometry.right, geometry.top),
        (geometry.right, geometry.bottom),
    );
    for fraction in fractions {
        let y = geometry.bottom - fraction * geometry.data_height();
        append_line(
            &mut path,
            (geometry.right, y),
            (geometry.right + tick_length, y),
        );
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> PlotGeometry {
        PlotGeometry::with_bounds(10.0, 5.0, 110.0, 105.0, 120.0, 130.0).unwrap()
    }

    #[test]
    fn fit_line_clips_to_visible_axes() {
        let ranges = ((0.0, 10.0), (0.0, 5.0));
        let bounds = kurbo::Shape::bounding_box(&fit_line(1.0, 0.0, ranges, geometry()));
        assert_eq!((bounds.x0, bounds.x1), (10.0, 60.0));
        assert!(fit_line(0.0, 6.0, ranges, geometry()).is_empty());
    }

    #[test]
    fn hover_crosshair_marks_visible_points_only() {
        let ranges = ((0.0, 10.0), (0.0, 10.0));
        let point = |x, y| HoverTarget::Point(PlotPoint { x, y });
        let marked = hover_crosshair(point(5.0, 5.0), ranges, geometry(), 4.0);
        assert!(
            marked
                .elements()
                .iter()
                .any(|el| matches!(el, kurbo::PathEl::ClosePath))
        );
        assert!(hover_crosshair(point(5.0, 20.0), ranges, geometry(), 4.0).is_empty());
        let cursor = hover_crosshair(HoverTarget::Cursor((50.0, 50.0)), ranges, geometry(), 4.0);
        assert!(
            !cursor
                .elements()
                .iter()
                .any(|el| matches!(el, kurbo::PathEl::ClosePath))
        );
    }

    #[test]
    fn cursor_line_spans_data_height() {
        let bounds = kurbo::Shape::bounding_box(&cursor_line(40.0, geometry()));
        assert_eq!((bounds.x0, bounds.y0, bounds.y1), (40.0, 5.0, 105.0));
    }

    #[test]
    fn value_lines_skip_values_outside_the_range() {
        let lines = vertical_value_lines([0.0, 5.0, 20.0], (0.0, 10.0), geometry());
        let starts = lines
            .elements()
            .iter()
            .filter_map(|el| match el {
                kurbo::PathEl::MoveTo(point) => Some(point.x.to_bits()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(starts, 2);
        assert!(vertical_value_lines([5.0], (1.0, 1.0), geometry()).is_empty());
        let rows = horizontal_value_lines([5.0], (0.0, 10.0), geometry());
        let bounds = kurbo::Shape::bounding_box(&rows);
        assert_eq!((bounds.x0, bounds.y0), (10.0, 55.0));
        assert!(bounds.x1 <= 110.0);
    }

    #[test]
    fn dashes_stop_at_the_end() {
        let mut path = BezPath::new();
        append_dashed_vline(&mut path, 1.0, (0.0, 23.0), 6.0, 10.0);
        let bounds = kurbo::Shape::bounding_box(&path);
        assert_eq!(bounds.y1, 23.0);
        assert_eq!(path.elements().len(), 6);
    }

    #[test]
    fn margin_frame_surrounds_data_area() {
        let frame = margin_frame(geometry());
        assert!(!kurbo::Shape::contains(&frame, (60.0, 50.0).into()));
        assert!(kurbo::Shape::contains(&frame, (5.0, 50.0).into()));
        assert!(kurbo::Shape::contains(&frame, (60.0, 120.0).into()));
    }

    #[test]
    fn intervals_merge_when_overlapping_or_touching() {
        let r = Range::new;
        assert_eq!(
            merge_intervals([r(5.0, 6.0), r(0.0, 2.0), r(2.0, 3.0)]),
            [r(0.0, 3.0), r(5.0, 6.0)]
        );
    }

    #[test]
    fn band_rects_do_not_overlap() {
        let r = Range::new;
        let rects = band_rects(&[r(2.0, 4.0)], &[r(0.0, 1.0)], ((0.0, 10.0), (0.0, 10.0)));
        assert_eq!(
            rects,
            [
                Ranges::new((2.0, 4.0), (0.0, 10.0)),
                Ranges::new((0.0, 2.0), (0.0, 1.0)),
                Ranges::new((4.0, 10.0), (0.0, 1.0)),
            ]
        );
    }

    #[test]
    fn right_axis_ticks_point_outward() {
        let path = right_axis(geometry(), &[0.0, 0.5, 1.0], 4.0);
        let bounds = kurbo::Shape::bounding_box(&path);
        assert_eq!((bounds.x0, bounds.x1), (110.0, 114.0));
        assert_eq!((bounds.y0, bounds.y1), (5.0, 105.0));
    }
}
