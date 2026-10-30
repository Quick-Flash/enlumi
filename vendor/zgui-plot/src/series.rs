use kurbo::BezPath;

use crate::{PlotGeometry, Ranges, append_circle};

/// Numeric sample used by backend independent plot geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

/// Build clipped line segments for adjacent samples, preserving input order.
pub fn line_segments(
    points: &[PlotPoint],
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
) -> BezPath {
    let Ranges {
        x: x_range,
        y: y_range,
    } = ranges.into();
    // Keep all clipped segments in one path. One shape per adjacent pair makes
    // dense trend lines overwhelm canvas submission compared with batched scatter.
    let mut path = BezPath::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let (mut start, mut end) = (0.0_f64, 1.0_f64);
        let mut outside = false;
        for (p, q) in [
            (-dx, a.x - x_range.min),
            (dx, x_range.max - a.x),
            (-dy, a.y - y_range.min),
            (dy, y_range.max - a.y),
        ] {
            if p == 0.0 {
                outside |= q < 0.0;
            } else if p < 0.0 {
                start = start.max(q / p);
            } else {
                end = end.min(q / p);
            }
        }
        if outside || start > end {
            continue;
        }
        path.move_to((
            geometry.x(a.x + start * dx, x_range),
            geometry.y(a.y + start * dy, y_range),
        ));
        path.line_to((
            geometry.x(a.x + end * dx, x_range),
            geometry.y(a.y + end * dy, y_range),
        ));
    }
    path
}

/// Build batched marker geometry for visible samples.
pub fn scatter_path(
    points: &[PlotPoint],
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
    radius: f64,
    edge_margin_px: f64,
) -> BezPath {
    let Ranges {
        x: x_range,
        y: y_range,
    } = ranges.into();
    let x_margin = edge_margin_px * x_range.span() / geometry.data_width();
    let y_margin = edge_margin_px * y_range.span() / geometry.data_height();
    let visible_x = (x_range.min - x_margin, x_range.max + x_margin);
    let visible_y = (y_range.min - y_margin, y_range.max + y_margin);
    let mut path = BezPath::new();
    for point in points.iter().filter(|point| {
        point.x.is_finite()
            && point.y.is_finite()
            && visible_x.0 <= point.x
            && point.x <= visible_x.1
            && visible_y.0 <= point.y
            && point.y <= visible_y.1
    }) {
        append_circle(
            &mut path,
            (geometry.x(point.x, x_range), geometry.y(point.y, y_range)),
            radius,
        );
    }
    path
}
