use crate::{PlotPoint, Ranges};

/// Index of the visible point closest to `target`, measured in screen pixels,
/// if it lies within `radius_px`. `pixels` is the plot's data area size.
pub fn nearest(
    points: impl IntoIterator<Item = PlotPoint>,
    target: PlotPoint,
    ranges: impl Into<Ranges>,
    pixels: (f64, f64),
    radius_px: f64,
) -> Option<usize> {
    let Ranges { x, y } = ranges.into();
    let distance = |point: PlotPoint| {
        ((point.x - target.x) / x.span() * pixels.0).powi(2)
            + ((point.y - target.y) / y.span() * pixels.1).powi(2)
    };
    points
        .into_iter()
        .enumerate()
        .filter(|(_, point)| x.contains(point.x) && y.contains(point.y))
        .map(|(index, point)| (index, distance(point)))
        .filter(|(_, distance)| *distance <= radius_px * radius_px)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

/// Find nearest point in X-sorted data, narrowing scan to points within pixel radius.
pub fn nearest_sorted(
    points: &[crate::PlotPoint],
    target: crate::PlotPoint,
    ranges: impl Into<crate::Ranges>,
    pixels: (f64, f64),
    radius_px: f64,
) -> Option<usize> {
    let ranges = ranges.into();
    let reach = radius_px / pixels.0 * ranges.x.span();
    let start = points.partition_point(|point| point.x < target.x - reach);
    let end = start + points[start..].partition_point(|point| point.x <= target.x + reach);
    nearest(
        points[start..end].iter().copied(),
        target,
        ranges,
        pixels,
        radius_px,
    )
    .map(|index| start + index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> PlotPoint {
        PlotPoint { x, y }
    }

    #[test]
    fn radius_is_measured_in_pixels_on_both_axes() {
        let points = [point(0.5, 0.5)];
        let hit = |x, y| {
            nearest(
                points,
                point(x, y),
                ((0.0, 1.0), (0.0, 1.0)),
                (600.0, 400.0),
                10.0,
            )
        };
        assert_eq!(hit(0.5 + 8.0 / 600.0, 0.5), Some(0));
        assert_eq!(hit(0.5, 0.5 + 8.0 / 400.0), Some(0));
        assert_eq!(hit(0.5 + 12.0 / 600.0, 0.5), None);
    }

    #[test]
    fn points_outside_view_are_ignored() {
        let points = [point(0.5, 20.0), point(0.5, 0.5)];
        let hit = |y| {
            nearest(
                points,
                point(0.5, y),
                ((0.0, 1.0), (0.0, 1.0)),
                (400.0, 300.0),
                10.0,
            )
        };
        assert_eq!(hit(20.0), None);
        assert_eq!(hit(0.51), Some(1));
    }
}
