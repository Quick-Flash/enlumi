use plot::{PlotPoint, time_series::SeriesData};

/// Causal PT1, using actual dt. Nonfinite samples, duplicate times and gaps reset state.
/// Gap threshold: 10 times median positive sample interval.
pub fn low_pass(source: &SeriesData, cutoff_hz: f64) -> Result<SeriesData, String> {
    if !cutoff_hz.is_finite() || cutoff_hz <= 0.0 {
        return Err("Cutoff must be finite and greater than zero Hz".into());
    }
    let points = source.samples();
    let mut intervals: Vec<_> = points
        .windows(2)
        .map(|p| p[1].x - p[0].x)
        .filter(|dt| dt.is_finite() && *dt > 0.0)
        .collect();
    intervals.sort_unstable_by(f64::total_cmp);
    let gap = intervals
        .get(intervals.len() / 2)
        .map_or(f64::INFINITY, |dt| dt * 10.0);
    let mut previous: Option<PlotPoint> = None;
    let filtered = points
        .iter()
        .map(|&point| {
            let mut result = point;
            if point.y.is_finite() {
                if let Some(last) = previous {
                    let dt = point.x - last.x;
                    if dt > 0.0 && dt <= gap {
                        let alpha = -(-std::f64::consts::TAU * cutoff_hz * dt).exp_m1();
                        result.y = last.y + alpha * (point.y - last.y);
                    }
                }
                previous = Some(result);
            } else {
                previous = None;
            }
            result
        })
        .collect::<Vec<_>>();
    SeriesData::new(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(samples: &[(f64, f64)]) -> SeriesData {
        SeriesData::new(
            samples
                .iter()
                .map(|&(x, y)| PlotPoint { x, y })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn constant_and_irregular_step_use_actual_time() {
        let input = data(&[(0.0, 0.0), (0.1, 1.0), (0.3, 1.0)]);
        let output = low_pass(&input, 1.0).unwrap();
        assert!(
            (output.samples()[1].y - (1.0 - (-std::f64::consts::TAU * 0.1).exp())).abs() < 1e-12
        );
        assert!(
            (output.samples()[2].y - (1.0 - (-std::f64::consts::TAU * 0.3).exp())).abs() < 1e-12
        );
        assert_eq!(input.samples()[1].y, 1.0);
        assert!(
            low_pass(&data(&[(0.0, 7.0), (0.1, 7.0)]), 1.0)
                .unwrap()
                .samples()
                .iter()
                .all(|p| p.y == 7.0)
        );
    }
    #[test]
    fn gaps_nan_and_duplicate_times_reset_state() {
        let output = low_pass(
            &data(&[
                (0.0, 0.0),
                (0.1, 1.0),
                (0.2, f64::NAN),
                (0.3, 5.0),
                (0.3, 6.0),
                (10.0, 9.0),
            ]),
            1.0,
        )
        .unwrap();
        assert!(output.samples()[2].y.is_nan());
        assert_eq!(output.samples()[3].y, 5.0);
        assert_eq!(output.samples()[4].y, 6.0);
        assert_eq!(output.samples()[5].y, 9.0);
    }
    #[test]
    fn rejects_bad_cutoffs_and_handles_empty() {
        for cutoff in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(low_pass(&data(&[]), cutoff).is_err());
        }
        assert!(low_pass(&data(&[]), 1.0).unwrap().samples().is_empty());
    }
}
