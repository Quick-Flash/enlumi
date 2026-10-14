use plot::{PlotPoint, Range, time_series::SeriesData};
use zgui::canvas::Lod;

const MAX_EXACT: usize = 65_536;

/// Viewport samples, including one neighbour each side for boundary segments.
/// Dense columns retain first/min/max/last; gapped columns keep extrema runs separately.
pub fn viewport(data: &SeriesData, x: Range, columns: usize, lod: Lod) -> SeriesData {
    let samples = data.samples();
    let start = samples.partition_point(|p| p.x < x.min).saturating_sub(1);
    let end = (samples.partition_point(|p| p.x <= x.max) + 1).min(samples.len());
    let samples = &samples[start.min(end)..end];
    let bins = columns.clamp(1, 8192) * 2;
    if samples.len() <= bins * 4 || (lod == Lod::Exact && samples.len() <= MAX_EXACT) {
        return SeriesData::new(samples.to_vec()).expect("sorted viewport samples");
    }
    let mut out = Vec::with_capacity(bins * 4 + 8);
    let mut at = 0;
    while at < samples.len() {
        let column = |p: PlotPoint| {
            (((p.x - x.min) / x.span() * bins as f64).floor() as i64).clamp(-1, bins as i64)
        };
        let bin = column(samples[at]);
        let first = at;
        let (mut low, mut high, mut gap) = (at, at, None);
        at += 1;
        while at < samples.len() && column(samples[at]) == bin {
            if samples[at].y < samples[low].y || !samples[low].y.is_finite() {
                low = at;
            }
            if samples[at].y > samples[high].y || !samples[high].y.is_finite() {
                high = at;
            }
            if !samples[at].y.is_finite() {
                gap = Some(at);
            }
            at += 1;
        }
        if !samples[first].y.is_finite() {
            gap = Some(first);
        }
        if gap.is_some() {
            let mut runs = Vec::with_capacity(2);
            for pick in [low, high] {
                if !samples[pick].y.is_finite() {
                    continue;
                }
                let mut start = pick;
                let mut end = pick;
                while start > first && samples[start - 1].y.is_finite() {
                    start -= 1;
                }
                while end + 1 < at && samples[end + 1].y.is_finite() {
                    end += 1;
                }
                if !runs.contains(&(start, end)) {
                    runs.push((start, end));
                }
            }
            runs.sort_unstable();
            out.push(PlotPoint {
                x: samples[first].x,
                y: f64::NAN,
            });
            for (start, end) in runs {
                let mut picks = [start, low.clamp(start, end), high.clamp(start, end), end];
                picks.sort_unstable();
                let mut previous = None;
                for pick in picks {
                    if previous != Some(pick) {
                        out.push(samples[pick]);
                        previous = Some(pick);
                    }
                }
                out.push(PlotPoint {
                    x: samples[end].x,
                    y: f64::NAN,
                });
            }
        } else {
            let mut picks = [first, low, high, at - 1];
            picks.sort_unstable();
            let mut previous = None;
            for pick in picks {
                if previous != Some(pick) {
                    out.push(samples[pick]);
                    previous = Some(pick);
                }
            }
        }
    }
    SeriesData::new(out).expect("column reduction retains sorted timestamps")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_draw::{Connection, prepare};

    #[test]
    fn million_sample_steps_are_bounded_and_retain_spikes() {
        let data = SeriesData::new(
            (0..1_000_000)
                .map(|i| PlotPoint {
                    x: i as f64,
                    y: if i == 543_210 { 1000.0 } else { (i % 2) as f64 },
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let view = viewport(&data, Range::new(0.0, 999_999.0), 1000, Lod::Columns);
        assert!(view.samples().len() <= 8008);
        assert!(view.samples().iter().any(|p| p.y == 1000.0));
        for mode in [Connection::Pre, Connection::Post] {
            let steps = prepare(&view, mode);
            assert!(steps.samples().len() <= 16016);
            assert!(
                steps
                    .samples()
                    .windows(2)
                    .all(|p| p[0].x == p[1].x || p[0].y == p[1].y)
            );
        }
        assert_eq!(data.samples().len(), 1_000_000);
    }

    #[test]
    fn zoom_keeps_boundary_neighbours_and_sparse_steps_exact() {
        let data = SeriesData::new(
            (0..100)
                .map(|i| PlotPoint {
                    x: i as f64,
                    y: (i % 3) as f64,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let view = viewport(&data, Range::new(40.0, 42.0), 1000, Lod::Columns);
        assert_eq!(view.samples(), &data.samples()[39..44]);
        assert_eq!(
            prepare(&view, Connection::Post).samples()[1],
            PlotPoint { x: 40.0, y: 0.0 }
        );
        let outside = viewport(&data, Range::new(200.0, 300.0), 1000, Lod::Columns);
        assert_eq!(outside.samples().len(), 1);
    }

    #[test]
    fn dense_gaps_stay_broken_and_exact_has_safety_budget() {
        let data = SeriesData::new(
            (0..100_000)
                .map(|i| PlotPoint {
                    x: i as f64,
                    y: if i % 11 == 0 { f64::NAN } else { 1.0 },
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let view = viewport(&data, Range::new(0.0, 99_999.0), 100, Lod::Exact);
        assert!(view.samples().len() <= 2208);
        assert!(view.samples().iter().any(|p| p.y.is_nan()));
        assert!(view.samples().iter().any(|p| p.y.is_finite()));
        for pair in prepare(&view, Connection::Pre).samples().windows(2) {
            if pair[0].y.is_finite() && pair[1].y.is_finite() {
                assert!(
                    data.samples()[pair[0].x as usize..=pair[1].x as usize]
                        .iter()
                        .all(|p| p.y.is_finite()),
                    "Reduction bridged source gap"
                );
            }
        }
    }
}
