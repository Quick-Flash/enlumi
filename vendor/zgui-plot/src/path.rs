use kurbo::BezPath;

/// Append a straight segment as its own subpath.
pub fn append_line(path: &mut BezPath, start: (f64, f64), end: (f64, f64)) {
    path.move_to(start);
    path.line_to(end);
}

/// Append a closed axis-aligned rectangle between two corners.
pub fn append_rect(path: &mut BezPath, x0: f64, y0: f64, x1: f64, y1: f64) {
    path.move_to((x0, y0));
    path.line_to((x1, y0));
    path.line_to((x1, y1));
    path.line_to((x0, y1));
    path.close_path();
}

/// Append a closed circle built from four cubic arcs; nothing for a zero radius.
pub fn append_circle(path: &mut BezPath, center: (f64, f64), radius: f64) {
    let radius = radius.max(0.0);
    if radius == 0.0 {
        return;
    }
    let kappa = 0.5522847498 * radius;
    let (x, y) = center;
    path.move_to((x + radius, y));
    path.curve_to(
        (x + radius, y + kappa),
        (x + kappa, y + radius),
        (x, y + radius),
    );
    path.curve_to(
        (x - kappa, y + radius),
        (x - radius, y + kappa),
        (x - radius, y),
    );
    path.curve_to(
        (x - radius, y - kappa),
        (x - kappa, y - radius),
        (x, y - radius),
    );
    path.curve_to(
        (x + kappa, y - radius),
        (x + radius, y - kappa),
        (x + radius, y),
    );
    path.close_path();
}

/// Append a closed rectangle whose top corners are rounded by `radius`, clamped
/// to fit; a zero radius gives a plain rectangle.
pub fn append_top_rounded_rect(
    path: &mut BezPath,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    radius: f64,
) {
    let radius = radius.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    if radius == 0.0 {
        append_rect(path, x0, y0, x1, y1);
        return;
    }
    let kappa = 0.5522847498;
    path.move_to((x0, y1));
    path.line_to((x1, y1));
    path.line_to((x1, y0 + radius));
    path.curve_to(
        (x1, y0 + radius - kappa * radius),
        (x1 - radius + kappa * radius, y0),
        (x1 - radius, y0),
    );
    path.line_to((x0 + radius, y0));
    path.curve_to(
        (x0 + radius - kappa * radius, y0),
        (x0, y0 + radius - kappa * radius),
        (x0, y0 + radius),
    );
    path.line_to((x0, y1));
    path.close_path();
}

/// Triangle marker centred at origin, pointing up and reaching `radius`.
pub fn triangle_marker(radius: f64) -> BezPath {
    let half = radius * 3f64.sqrt() / 2.0;
    let mut path = BezPath::new();
    path.move_to((0.0, -radius));
    path.line_to((half, radius / 2.0));
    path.line_to((-half, radius / 2.0));
    path.close_path();
    path
}

/// Diamond marker centred at origin, reaching `radius` from it.
pub fn diamond_marker(radius: f64) -> BezPath {
    let mut path = BezPath::new();
    path.move_to((0.0, -radius));
    path.line_to((radius, 0.0));
    path.line_to((0.0, radius));
    path.line_to((-radius, 0.0));
    path.close_path();
    path
}

/// Plus marker centred at origin, with arms reaching `radius`.
pub fn cross_marker(radius: f64, thickness: f64) -> BezPath {
    let (r, t) = (radius, thickness / 2.0);
    let mut path = BezPath::new();
    path.move_to((-t, -r));
    for point in [
        (t, -r),
        (t, -t),
        (r, -t),
        (r, t),
        (t, t),
        (t, r),
        (-t, r),
        (-t, t),
        (-r, t),
        (-r, -t),
        (-t, -t),
    ] {
        path.line_to(point);
    }
    path.close_path();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closes(path: &BezPath) -> bool {
        matches!(path.elements().last(), Some(kurbo::PathEl::ClosePath))
    }

    #[test]
    fn closed_shapes_end_with_close_path() {
        let mut rounded = BezPath::new();
        append_top_rounded_rect(&mut rounded, 0.0, 0.0, 10.0, 10.0, 2.0);
        let mut circle = BezPath::new();
        append_circle(&mut circle, (5.0, 5.0), 3.0);
        assert!(closes(&rounded) && closes(&circle));
    }

    #[test]
    fn zero_radius_circle_adds_nothing() {
        let mut path = BezPath::new();
        append_circle(&mut path, (5.0, 5.0), 0.0);
        assert!(path.is_empty());
    }
}
