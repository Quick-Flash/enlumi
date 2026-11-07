//! Bounded color batches after XY sampling and dash preparation, retaining dash phase.
use super::telemetry::Coloring;
use crate::trajectory::PreparedTrace;
use plot::{PlotPoint, Ranges, line_draw::LineDraw, plot_data::SeriesStyle};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
pub type Buckets = Vec<([u8; 4], Arc<[[f32; 2]]>)>;
#[derive(Default, Clone)]
pub struct Colored {
    pub lines: Buckets,
    pub markers: Buckets,
}
fn finite(p: PlotPoint) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn mix(a: f64, b: f64, t: f64) -> f64 {
    if a.is_finite() && b.is_finite() {
        a + (b - a) * t
    } else {
        f64::NAN
    }
}
fn fraction(p: PlotPoint, a: PlotPoint, b: PlotPoint, scale: [f64; 2]) -> Option<f64> {
    let dx = (b.x - a.x) * scale[0];
    let dy = (b.y - a.y) * scale[1];
    let length = dx * dx + dy * dy;
    if length == 0. {
        return None;
    }
    let x = (p.x - a.x) * scale[0];
    let y = (p.y - a.y) * scale[1];
    let t = (x * dx + y * dy) / length;
    ((-1e-6..=1. + 1e-6).contains(&t) && (x * dy - y * dx).abs() / length.sqrt() < 0.01)
        .then(|| t.clamp(0., 1.))
}
pub fn prepare(
    trace: &PreparedTrace,
    ranges: Ranges,
    pixels: (f64, f64),
    draw: LineDraw,
    style: SeriesStyle,
    colors: &Coloring,
) -> Colored {
    let (width, radius, fallback) = (style.width, style.radius, style.color);
    let Some(values) = colors.samples.get(&trace.id) else {
        return Colored::default();
    };
    let points = &trace.track.points;
    let clipped = crate::trajectory_render::clip_runs(points, ranges);
    let sampled = crate::trajectory_render::sample_xy(&clipped, ranges, pixels);
    let mut source = 0;
    let scalars = sampled
        .iter()
        .map(|&point| {
            if !finite(point) {
                return (f64::NAN, f64::NAN);
            }
            while source < points.len() && points[source] != point {
                source += 1;
            }
            let value = (
                values.get(source).copied().unwrap_or(f64::NAN),
                trace.track.times.get(source).copied().unwrap_or(f64::NAN),
            );
            source = source.saturating_add(1);
            value
        })
        .collect::<Vec<_>>();
    let patterned = plot::line_pattern::prepare_path(&sampled, ranges, pixels, draw.pattern, width);
    let scale = [pixels.0 / ranges.x.span(), pixels.1 / ranges.y.span()];
    let local = |p: PlotPoint| {
        [
            (p.x - trace.track.origin.x) as f32,
            (p.y - trace.track.origin.y) as f32,
        ]
    };
    let mut lines = BTreeMap::<[u8; 4], Vec<[f32; 2]>>::new();
    let mut segment = 0;
    for pair in patterned
        .windows(2)
        .filter(|pair| pair.iter().all(|&p| finite(p)))
    {
        while segment + 2 < sampled.len()
            && sampled[segment + 1] == pair[0]
            && finite(sampled[segment + 2])
        {
            segment += 1;
        }
        let mut ends = None;
        while segment + 1 < sampled.len() {
            if let (Some(a), Some(b)) = (
                fraction(pair[0], sampled[segment], sampled[segment + 1], scale),
                fraction(pair[1], sampled[segment], sampled[segment + 1], scale),
            ) {
                ends = Some((a, b));
                break;
            }
            segment += 1;
        }
        let (a, b, ta, tb) = ends.map_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN), |(a, b)| {
            (
                mix(scalars[segment].0, scalars[segment + 1].0, a),
                mix(scalars[segment].0, scalars[segment + 1].0, b),
                mix(scalars[segment].1, scalars[segment + 1].1, a),
                mix(scalars[segment].1, scalars[segment + 1].1, b),
            )
        });
        let count = colors
            .bucket_for(trace.id, a)
            .zip(colors.bucket_for(trace.id, b))
            .map_or(1, |(a, b)| a.abs_diff(b).max(1)) as usize;
        let mut cuts = crate::state_tint::cuts(&colors.tints, ta, tb);
        cuts.extend((1..count).map(|i| i as f64 / count as f64));
        cuts.sort_unstable_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for cut in cuts.windows(2) {
            let (lo, hi) = (cut[0], cut[1]);
            let mid = (lo + hi) / 2.;
            let color = colors.shade(trace.id, mix(a, b, mid), mix(ta, tb, mid), fallback);
            let lerp = |t: f64| PlotPoint {
                x: pair[0].x + (pair[1].x - pair[0].x) * t,
                y: pair[0].y + (pair[1].y - pair[0].y) * t,
            };
            lines.entry(color).or_default().extend([
                local(lerp(lo)),
                local(lerp(hi)),
                [f32::NAN, f32::NAN],
            ]);
        }
    }
    let mut markers = BTreeMap::<[u8; 4], Vec<[f32; 2]>>::new();
    if draw.markers {
        let mut cells = HashSet::new();
        let spacing = (radius * 2.).max(4.);
        for (index, &p) in points.iter().enumerate() {
            if !finite(p)
                || p.x < ranges.x.min - ranges.x.span() * 0.75
                || p.x > ranges.x.max + ranges.x.span() * 0.75
                || p.y < ranges.y.min - ranges.y.span() * 0.75
                || p.y > ranges.y.max + ranges.y.span() * 0.75
            {
                continue;
            }
            if cells.insert((
                ((p.x - ranges.x.min) * scale[0] / spacing).floor() as i64,
                ((p.y - ranges.y.min) * scale[1] / spacing).floor() as i64,
            )) {
                markers
                    .entry(colors.color(trace.id, index, fallback))
                    .or_default()
                    .push(local(p));
            }
        }
    }
    Colored {
        lines: lines
            .into_iter()
            .map(|(key, value)| (key, value.into()))
            .collect(),
        markers: markers
            .into_iter()
            .map(|(key, value)| (key, value.into()))
            .collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_tint_splits_at_exact_times_and_keeps_base_color() {
        let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
        let tint = crate::state_tint::tests::fixture();
        let colors = super::super::telemetry::prepare_styled(
            &tracks,
            &lines,
            &spec,
            &Default::default(),
            &Default::default(),
            vec![tint],
        );
        let result = prepare(
            &tracks.traces[0],
            Ranges::new((-10., 100.), (-10., 10.)),
            (1100., 200.),
            LineDraw::default(),
            SeriesStyle {
                width: 2.,
                radius: 3.,
                color: [0, 0, 200, 255],
                ..SeriesStyle::for_id("2")
            },
            &colors,
        );
        assert!(
            result
                .lines
                .iter()
                .any(|(color, _)| *color == [64, 0, 150, 255])
        );
        assert!(
            result
                .lines
                .iter()
                .any(|(color, _)| *color == [0, 64, 150, 255])
        );
        assert!(
            result
                .lines
                .iter()
                .any(|(color, _)| *color == [0, 0, 200, 255])
        );
        for (color, points) in &result.lines {
            for pair in points
                .windows(2)
                .filter(|pair| pair.iter().flatten().all(|v| v.is_finite()))
            {
                if *color == [64, 0, 150, 255] {
                    assert!(pair.iter().all(|p| p[0] <= 10.001));
                }
                if *color == [0, 64, 150, 255] {
                    assert!(pair.iter().all(|p| p[0] >= 9.999 && p[0] <= 30.001));
                }
            }
        }
    }
    #[test]
    fn color_batches_preserve_dash_ink_and_position_gaps() {
        let (lines, mut spec, tracks) = super::super::telemetry::tests::fixture();
        spec.map.as_mut().unwrap().coloring = super::super::telemetry::ColorBy::Speed;
        let colors = super::super::telemetry::prepare(&tracks, &lines, &spec, &Default::default());
        let trace = &tracks.traces[0];
        let ranges = Ranges::new((-10., 100.), (-10., 10.));
        let pixels = (1100., 200.);
        for pattern in [
            plot::line_draw::LinePattern::Solid,
            plot::line_draw::LinePattern::Dashed,
            plot::line_draw::LinePattern::Dotted,
            plot::line_draw::LinePattern::DashDot,
        ] {
            let result = prepare(
                trace,
                ranges,
                pixels,
                LineDraw {
                    pattern,
                    markers: true,
                    ..Default::default()
                },
                SeriesStyle {
                    width: 2.,
                    radius: 3.,
                    color: [255; 4],
                    ..SeriesStyle::for_id("2")
                },
                &colors,
            );
            assert!(result.lines.len() > 1 && result.lines.len() <= 65);
            assert_eq!(
                result
                    .markers
                    .iter()
                    .map(|(_, points)| points.len())
                    .sum::<usize>(),
                5
            );
            let segments = result
                .lines
                .iter()
                .flat_map(|(_, points)| points.windows(2))
                .filter(|pair| pair.iter().flatten().all(|v| v.is_finite()))
                .collect::<Vec<_>>();
            assert!(
                segments
                    .iter()
                    .all(|pair| pair[0][0].max(pair[1][0]) <= 30.001
                        || pair[0][0].min(pair[1][0]) >= 59.999)
            );
            let ink = segments
                .iter()
                .map(|pair| f64::from((pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])))
                .sum::<f64>();
            let sampled = crate::trajectory_render::sample_xy(
                &crate::trajectory_render::clip_runs(&trace.track.points, ranges),
                ranges,
                pixels,
            );
            let plain = plot::line_pattern::prepare_path(&sampled, ranges, pixels, pattern, 2.);
            let expected = plain
                .windows(2)
                .filter(|pair| pair.iter().all(|&p| finite(p)))
                .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
                .sum::<f64>();
            assert!(
                (ink - expected).abs() < 0.001,
                "{pattern:?}: colored ink {ink}, original {expected}"
            );
        }
    }
}
