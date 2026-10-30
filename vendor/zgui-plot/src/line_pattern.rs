//! Bounded pixel-spaced patterns for retained lines (native Series::Line ignores dashes).
use crate::{PlotPoint, Ranges, line_draw::LinePattern, time_series::SeriesData};
use kurbo::Point;

/// Reduced/step-expanded data in; NaN-separated dash segments out.
/// Overscan preserves nearby pans. Extremely dense traces lengthen pattern spacing
/// to limit dash count rather than dropping tail geometry or allocating unbounded buffers.
pub fn prepare(
    data: &SeriesData,
    ranges: Ranges,
    pixels: (f64, f64),
    pattern: LinePattern,
    width: f64,
) -> SeriesData {
    if pattern == LinePattern::Solid {
        return data.clone();
    }
    SeriesData::new(prepare_path(data.samples(), ranges, pixels, pattern, width))
        .expect("time-series pattern adapter retains ordered timestamps")
}

/// Build pixel-spaced patterns along an already sampled path, preserving input order.
///
/// X may increase, decrease, or repeat. Nonfinite points break the path. Output
/// runs are separated by NaN Y values; their X remains finite so sorted input
/// can still be adapted to `SeriesData`. No sorting or time-series reduction is
/// performed here: callers choose sampling appropriate to their coordinate domain.
/// Clipping, dash phase, width-relative spacing, and the dash budget are shared
/// by time-series and arbitrary XY renderers. One viewport of overscan each side
/// allows consumers to retain this geometry while panning nearby.
pub fn prepare_path(
    points: &[PlotPoint],
    ranges: Ranges,
    pixels: (f64, f64),
    pattern: LinePattern,
    width: f64,
) -> Vec<PlotPoint> {
    match pattern {
        LinePattern::Solid => return points.to_vec(),
        LinePattern::None => return Vec::new(),
        _ => {}
    }
    let (w, h) = pixels;
    if ![w, h, ranges.x.span(), ranges.y.span(), width]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.)
    {
        return Vec::new();
    }
    let map = |p: PlotPoint| {
        Point::new(
            (p.x - ranges.x.min) / ranges.x.span() * w,
            (p.y - ranges.y.min) / ranges.y.span() * h,
        )
    };
    let unmap = |p: Point| PlotPoint {
        x: ranges.x.min + p.x / w * ranges.x.span(),
        y: ranges.y.min + p.y / h * ranges.y.span(),
    };
    let mut segments = Vec::new();
    let mut previous = None;
    let mut total = 0.0;
    for pair in points.windows(2) {
        let (a, b) = (map(pair[0]), map(pair[1]));
        let Some((a, b)) = clip(a, b, (-w, 2.0 * w, -h, 2.0 * h)) else {
            previous = None;
            continue;
        };
        let length = a.distance(b);
        if length == 0.0 {
            continue;
        }
        let connected = previous == Some(a);
        segments.push((a, b, length, connected));
        previous = Some(b);
        total += length;
    }
    let lengths: &[f64] = match pattern {
        LinePattern::Dashed => &[6.0, 3.0],
        LinePattern::Dotted => &[0.05, 3.0],
        LinePattern::DashDot => &[6.0, 3.0, 0.05, 3.0],
        _ => unreachable!(),
    };
    let unit = width.max(0.5);
    let period = lengths.iter().sum::<f64>() * unit;
    let scale = (total / (period * 16_384.0)).max(1.0) * unit;
    let mut index = 0;
    let mut left = lengths[0] * scale;
    let mut open = false;
    let mut out = Vec::new();
    for (a, b, length, connected) in segments {
        if !connected {
            if open {
                out.push(PlotPoint {
                    x: unmap(a).x,
                    y: f64::NAN,
                });
            }
            open = false;
            index = 0;
            left = lengths[0] * scale;
        }
        let mut at = 0.0;
        while at < length {
            let step = left.min(length - at);
            let end = (at + step).min(length);
            // Guard tiny residuals at long segments from preventing progress.
            if end <= at {
                break;
            }
            if index % 2 == 0 {
                if !open {
                    out.push(unmap(a.lerp(b, at / length)));
                }
                out.push(unmap(a.lerp(b, end / length)));
                open = true;
            }
            at = end;
            left -= step;
            if left <= 1e-9 * scale {
                if open {
                    out.push(PlotPoint {
                        x: unmap(a.lerp(b, at / length)).x,
                        y: f64::NAN,
                    });
                    open = false;
                }
                index = (index + 1) % lengths.len();
                left = lengths[index] * scale;
            }
        }
    }
    out
}

fn clip(
    a: Point,
    b: Point,
    (xmin, xmax, ymin, ymax): (f64, f64, f64, f64),
) -> Option<(Point, Point)> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let delta = b - a;
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for (origin, d, min, max) in [(a.x, delta.x, xmin, xmax), (a.y, delta.y, ymin, ymax)] {
        if d == 0.0 {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let (t0, t1) = ((min - origin) / d, (max - origin) / d);
            low = low.max(t0.min(t1));
            high = high.min(t0.max(t1));
            if low > high {
                return None;
            }
        }
    }
    Some((a.lerp(b, low), a.lerp(b, high)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_paths_keep_loops_vertical_segments_and_gaps() {
        let points = [
            PlotPoint { x: 0., y: 0. },
            PlotPoint { x: 10., y: 0. },
            PlotPoint { x: 10., y: 10. },
            PlotPoint { x: 0., y: 10. },
            PlotPoint {
                x: f64::NAN,
                y: f64::NAN,
            },
            PlotPoint { x: 8., y: 5. },
            PlotPoint { x: 1., y: 5. },
        ];
        assert!(
            SeriesData::new(points.to_vec()).is_err(),
            "This path is deliberately outside the time-series domain"
        );
        let path = prepare_path(
            &points,
            Ranges::new((0., 10.), (0., 10.)),
            (100., 100.),
            LinePattern::Dashed,
            1.,
        );
        let segments = path
            .windows(2)
            .filter(|pair| pair[0].y.is_finite() && pair[1].y.is_finite())
            .collect::<Vec<_>>();
        assert!(
            segments
                .iter()
                .any(|pair| pair[0].x == 10. && pair[1].x == 10. && pair[1].y > pair[0].y)
        );
        assert!(segments.iter().any(|pair| pair[1].x < pair[0].x));
        assert!(
            segments
                .iter()
                .all(|pair| pair[0].x == pair[1].x || pair[0].y == pair[1].y),
            "Separate XY runs must not be bridged"
        );
    }

    #[test]
    fn pattern_gaps_scale_with_thick_strokes_in_both_adapters() {
        let points = [PlotPoint { x: 0., y: 0. }, PlotPoint { x: 1000., y: 0. }];
        let data = SeriesData::new(points.to_vec()).unwrap();
        let ranges = Ranges::new((0., 1000.), (-1., 1.));
        for pattern in [
            LinePattern::Dashed,
            LinePattern::Dotted,
            LinePattern::DashDot,
        ] {
            for width in [1., 10., 32.] {
                let path = prepare_path(&points, ranges, (1000., 100.), pattern, width);
                let first_break = path.iter().position(|point| point.y.is_nan()).unwrap();
                let gap = path[first_break + 1].x - path[first_break - 1].x;
                assert!(
                    (gap - 3. * width).abs() < 1e-9,
                    "Gaps must remain wider than the stroke caps"
                );
                let time = prepare(&data, ranges, (1000., 100.), pattern, width);
                assert_eq!(time.samples().len(), path.len());
                for (a, b) in time.samples().iter().zip(path) {
                    assert_eq!(a.x, b.x);
                    assert!(a.y == b.y || (a.y.is_nan() && b.y.is_nan()));
                }
            }
        }
    }

    #[test]
    fn solid_none_clipping_and_invalid_geometry_work_without_sorted_x() {
        let points = [PlotPoint { x: 20., y: 2. }, PlotPoint { x: -20., y: 2. }];
        let ranges = Ranges::new((0., 10.), (0., 10.));
        assert_eq!(
            prepare_path(&points, ranges, (100., 100.), LinePattern::Solid, 1.),
            points
        );
        assert!(prepare_path(&points, ranges, (100., 100.), LinePattern::None, 1.).is_empty());
        let clipped = prepare_path(&points, ranges, (100., 100.), LinePattern::Dashed, 1.);
        assert!(
            clipped
                .iter()
                .all(|point| point.x >= -10. && point.x <= 20.)
        );
        assert!(
            prepare_path(&points, ranges, (f64::NAN, 100.), LinePattern::Dashed, 1.).is_empty()
        );
        assert!(prepare_path(&points, ranges, (100., 0.), LinePattern::Dashed, 1.).is_empty());
    }

    #[test]
    fn patterns_have_pixel_spacing_and_none_has_no_line() {
        let data = SeriesData::new(vec![
            PlotPoint { x: 0.0, y: 1.0 },
            PlotPoint { x: 100.0, y: 1.0 },
        ])
        .unwrap();
        let ranges = Ranges::new((0.0, 100.0), (0.0, 2.0));
        for pattern in [
            LinePattern::Dashed,
            LinePattern::Dotted,
            LinePattern::DashDot,
        ] {
            let prepared = prepare(&data, ranges, (100.0, 100.0), pattern, 1.0);
            assert!(prepared.samples().iter().any(|p| p.y.is_nan()));
            assert!(
                prepared
                    .samples()
                    .iter()
                    .filter(|p| p.y.is_finite())
                    .all(|p| p.y == 1.0)
            );
            let expected = if pattern == LinePattern::Dotted {
                0.05
            } else {
                6.0
            };
            assert!((prepared.samples()[1].x - expected).abs() < 1e-9);
            let zoomed = prepare(
                &data,
                Ranges::new((0.0, 50.0), (0.0, 2.0)),
                (100.0, 100.0),
                pattern,
                1.0,
            );
            assert!((zoomed.samples()[1].x - expected / 2.0).abs() < 1e-9);
        }
        assert!(
            prepare(&data, ranges, (100.0, 100.0), LinePattern::None, 1.0)
                .samples()
                .is_empty()
        );
    }
    #[test]
    fn dense_steps_remain_bounded_and_keep_gaps() {
        let data = SeriesData::new(
            (0..20_000)
                .map(|x| PlotPoint {
                    x: x as f64,
                    y: if x % 99 == 0 {
                        f64::NAN
                    } else {
                        (x % 2) as f64
                    },
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let steps = crate::line_draw::prepare(&data, crate::line_draw::Connection::Post);
        let patterned = prepare(
            &steps,
            Ranges::new((0.0, 20_000.0), (0.0, 1.0)),
            (1000.0, 1000.0),
            LinePattern::Dotted,
            1.0,
        );
        assert!(patterned.samples().len() < 250_000);
        assert!(patterned.samples().iter().any(|p| p.y.is_nan()));
        for pair in patterned
            .samples()
            .windows(2)
            .filter(|p| p[0].y.is_finite() && p[1].y.is_finite())
        {
            assert!(pair[0].x == pair[1].x || (pair[0].y - pair[1].y).abs() < 1e-9);
        }
    }
}
