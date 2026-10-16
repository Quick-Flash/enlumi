//! Offline PT1 forward/backward passes, with odd-reflected finite segment edges.
use plot::{PlotPoint, time_series::SeriesData};

pub fn apply(source: &SeriesData, hz: f64, high_pass: bool) -> Result<SeriesData, String> {
    if !hz.is_finite() || hz <= 0.0 {
        return Err("Cutoff must be finite and greater than zero Hz".into());
    }
    let points = source.samples();
    let gap = crate::filter::gap_threshold(points);
    let median_dt = gap / 10.0;
    let mut output: Vec<_> = points
        .iter()
        .map(|point| PlotPoint {
            x: point.x,
            y: f64::NAN,
        })
        .collect();
    for range in crate::filter_extra::segments(points, gap) {
        let segment = &points[range.clone()];
        let first = segment[0];
        let last = segment[segment.len() - 1];
        // Three time constants of reflected history, bounded for large logs.
        let pad = ((3.0 / (std::f64::consts::TAU * hz * median_dt)).ceil() as usize)
            .clamp(6, 65_536)
            .min(segment.len() - 1);
        let mut padded = Vec::with_capacity(segment.len() + 2 * pad);
        for i in (1..=pad).rev() {
            padded.push(PlotPoint {
                x: 2.0 * first.x - segment[i].x,
                y: 2.0 * first.y - segment[i].y,
            });
        }
        padded.extend_from_slice(segment);
        for i in 1..=pad {
            let point = segment[segment.len() - 1 - i];
            padded.push(PlotPoint {
                x: 2.0 * last.x - point.x,
                y: 2.0 * last.y - point.y,
            });
        }
        pass(&mut padded, false, high_pass, hz);
        pass(&mut padded, true, high_pass, hz);
        for (target, filtered) in output[range]
            .iter_mut()
            .zip(&padded[pad..pad + segment.len()])
        {
            target.y = filtered.y;
        }
    }
    SeriesData::new(output)
}
fn pass(points: &mut [PlotPoint], reverse: bool, high_pass: bool, hz: f64) {
    let len = points.len();
    let index = |step| if reverse { len - 1 - step } else { step };
    let mut previous = points[index(0)];
    points[index(0)].y = if high_pass { 0.0 } else { previous.y };
    for step in 1..len {
        let i = index(step);
        let raw = points[i];
        let dt = (raw.x - previous.x).abs();
        let alpha = -(-std::f64::consts::TAU * hz * dt).exp_m1();
        let low = previous.y + alpha * (raw.y - previous.y);
        points[i].y = if high_pass { raw.y - low } else { low };
        previous = PlotPoint { x: raw.x, y: low };
    }
}
