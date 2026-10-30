use super::*;
use crate::line_draw::{Connection, LineDraw};
use std::collections::BTreeMap;

impl PlotData {
    /// Fit visible series to current X window, including clipped boundary segments.
    /// No finite visible geometry returns None, preserving caller's current Y view.
    pub fn fitted_y_range(
        &self,
        hidden: &BTreeSet<String>,
        x: Range,
        bins: usize,
        drawing: &BTreeMap<String, LineDraw>,
    ) -> Option<Range> {
        let mut extent: Option<Range> = None;
        let mut add = |y: f64| {
            if y.is_finite() {
                extent = Some(extent.map_or(Range::new(y, y), |old| {
                    Range::new(old.min.min(y), old.max.max(y))
                }));
            }
        };
        match self {
            Self::Time(set) => {
                for line in set
                    .series()
                    .iter()
                    .filter(|line| !hidden.contains(line.id()))
                {
                    let samples = line.data().samples();
                    let start = samples.partition_point(|point| point.x < x.min);
                    let end = samples.partition_point(|point| point.x <= x.max);
                    for point in &samples[start..end] {
                        add(point.y);
                    }
                    if drawing
                        .get(line.id())
                        .is_some_and(|draw| draw.pattern == crate::line_draw::LinePattern::None)
                    {
                        continue;
                    }
                    let mode = drawing
                        .get(line.id())
                        .copied()
                        .unwrap_or_default()
                        .connection;
                    for edge in [x.min, x.max] {
                        let right = samples.partition_point(|point| point.x <= edge);
                        if right == 0 || right == samples.len() {
                            continue;
                        }
                        let (a, b) = (samples[right - 1], samples[right]);
                        if !a.y.is_finite() || !b.y.is_finite() || b.x <= a.x {
                            continue;
                        }
                        add(match mode {
                            Connection::Linear => {
                                let t = (edge - a.x) / (b.x - a.x);
                                (1.0 - t) * a.y + t * b.y
                            }
                            Connection::Pre => b.y,
                            Connection::Post => a.y,
                        });
                    }
                }
            }
            Self::Scatter(set) => {
                for line in set
                    .series()
                    .iter()
                    .filter(|line| !hidden.contains(line.id()))
                {
                    let start = line
                        .x_sorted
                        .partition_point(|&i| line.samples[i].x < x.min);
                    let end = line
                        .x_sorted
                        .partition_point(|&i| line.samples[i].x <= x.max);
                    for &i in &line.x_sorted[start..end] {
                        add(line.samples[i].y);
                    }
                }
            }
            Self::Histogram(set) => {
                let groups = set.grouped_bins(hidden, bins, x);
                if groups.is_empty() {
                    return None;
                }
                let max = (0..bins.clamp(1, 4096))
                    .map(|i| groups.iter().map(|(_, bins)| bins[i].count).sum::<usize>())
                    .max()
                    .unwrap_or(0);
                return Some(Range::new(0.0, (max as f64).max(1.0)));
            }
        }
        extent.and_then(|range| {
            crate::range::valid(range.expand(0.05, (range.min.abs() * 1e-6).max(1e-9)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_series::{SeriesData, TimeSeries};

    fn time(points: &[(f64, f64)]) -> PlotData {
        PlotData::Time(
            TimeSeriesSet::new(vec![TimeSeries::new(
                "a",
                "a",
                SeriesData::new(
                    points
                        .iter()
                        .map(|&(x, y)| PlotPoint { x, y })
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
            )])
            .unwrap(),
        )
    }

    #[test]
    fn visible_y_clips_boundary_segments_and_respects_step_connections() {
        let data = time(&[(0.0, 0.0), (100.0, 200.0), (101.0, 10000.0)]);
        let hidden = BTreeSet::new();
        let x = Range::new(10.0, 20.0);
        for (mode, expected) in [
            (Connection::Linear, Range::new(19.0, 41.0)),
            (Connection::Pre, Range::new(199.9998, 200.0002)),
            (Connection::Post, Range::new(-1e-9, 1e-9)),
        ] {
            let drawing = [(
                "a".into(),
                LineDraw {
                    connection: mode,
                    markers: false,
                    ..Default::default()
                },
            )]
            .into();
            let actual = data.fitted_y_range(&hidden, x, 80, &drawing).unwrap();
            assert!((actual.min - expected.min).abs() < 1e-10);
            assert!((actual.max - expected.max).abs() < 1e-10);
        }
    }

    #[test]
    fn visible_y_excludes_outliers_hidden_series_gaps_and_empty_windows() {
        let data = time(&[
            (0.0, 10000.0),
            (10.0, 2.0),
            (15.0, 4.0),
            (20.0, 2.0),
            (30.0, -10000.0),
        ]);
        let hidden = BTreeSet::new();
        let drawing = BTreeMap::new();
        assert_eq!(
            data.fitted_y_range(&hidden, Range::new(10.0, 20.0), 80, &drawing),
            Some(Range::new(1.9, 4.1))
        );
        assert!(
            data.fitted_y_range(&["a".into()].into(), Range::new(10.0, 20.0), 80, &drawing)
                .is_none()
        );
        assert!(
            data.fitted_y_range(&hidden, Range::new(40.0, 50.0), 80, &drawing)
                .is_none()
        );
        let gap = time(&[(0.0, 1.0), (5.0, f64::NAN), (10.0, 3.0)]);
        assert!(
            gap.fitted_y_range(&hidden, Range::new(6.0, 9.0), 80, &drawing)
                .is_none()
        );
    }
}
