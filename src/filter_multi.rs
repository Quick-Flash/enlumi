use crate::derived::{FilterSpec, RpyAxis};
use plot::{PlotPoint, time_series::SeriesData};

/// First input supplies timestamps. Others interpolate inside finite, short segments only.
pub fn apply(spec: FilterSpec, inputs: &[&SeriesData]) -> Result<SeriesData, String> {
    let reference = inputs[0].samples();
    let mut aligned: Vec<_> = inputs
        .iter()
        .map(|data| Aligned::new(data, reference))
        .collect();
    let mut values = vec![0.0; inputs.len()];
    let samples = reference
        .iter()
        .enumerate()
        .map(|(index, point)| {
            for (value, input) in values.iter_mut().zip(&mut aligned) {
                *value = input.at(index, point.x);
            }
            let y = if values.iter().all(|value| value.is_finite()) {
                match spec {
                    FilterSpec::Magnitude => values
                        .iter()
                        .fold(0.0_f64, |norm, &value| norm.hypot(value)),
                    FilterSpec::Quaternion(axis, unit) => {
                        unit.convert_radians(quaternion(&values, axis))
                    }
                    _ => unreachable!("multi-input transform"),
                }
            } else {
                f64::NAN
            };
            PlotPoint {
                x: point.x,
                y: if y.is_finite() { y } else { f64::NAN },
            }
        })
        .collect::<Vec<_>>();
    SeriesData::new(samples)
}

fn quaternion(q: &[f64], axis: RpyAxis) -> f64 {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    if w == 0.0 && x == 0.0 && y == 0.0 && z == 0.0 {
        return f64::NAN;
    }
    match axis {
        RpyAxis::Roll => (2.0 * (w * x + y * z)).atan2(w * w - x * x - y * y + z * z),
        RpyAxis::Pitch => (-(2.0 * (x * z - w * y))).clamp(-1.0, 1.0).asin(),
        RpyAxis::Yaw => (2.0 * (x * y + w * z)).atan2(w * w + x * x - y * y - z * z),
    }
}

struct Aligned<'a> {
    samples: &'a [PlotPoint],
    matching: bool,
    cursor: usize,
    gap: f64,
}
impl<'a> Aligned<'a> {
    fn new(data: &'a SeriesData, reference: &[PlotPoint]) -> Self {
        let samples = data.samples();
        let matching = samples.len() == reference.len()
            && samples.iter().zip(reference).all(|(a, b)| a.x == b.x);
        let mut intervals: Vec<_> = samples
            .windows(2)
            .filter(|_| !matching)
            .map(|p| p[1].x - p[0].x)
            .filter(|dt| *dt > 0.0)
            .collect();
        intervals.sort_unstable_by(f64::total_cmp);
        let gap = intervals
            .get(intervals.len() / 2)
            .map_or(0.0, |dt| dt * 10.0);
        Self {
            samples,
            matching,
            cursor: 0,
            gap,
        }
    }
    fn at(&mut self, index: usize, time: f64) -> f64 {
        if self.matching {
            return self.samples[index].y;
        }
        let Some(first) = self.samples.first() else {
            return f64::NAN;
        };
        if time < first.x {
            return f64::NAN;
        }
        while self.cursor + 1 < self.samples.len() && self.samples[self.cursor + 1].x <= time {
            self.cursor += 1;
        }
        let left = self.samples[self.cursor];
        if left.x == time {
            return left.y;
        }
        let Some(right) = self.samples.get(self.cursor + 1) else {
            return f64::NAN;
        };
        let dt = right.x - left.x;
        if dt <= 0.0 || dt > self.gap || !left.y.is_finite() || !right.y.is_finite() {
            return f64::NAN;
        }
        left.y * (1.0 - (time - left.x) / dt) + right.y * ((time - left.x) / dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derived::AngleUnit;
    fn data(points: &[(f64, f64)]) -> SeriesData {
        SeriesData::new(
            points
                .iter()
                .map(|&(x, y)| PlotPoint { x, y })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn magnitude_aligns_by_time_preserves_gaps_and_does_not_extrapolate() {
        let a = data(&[
            (-1., 3.),
            (0., 3.),
            (0.5, 3.),
            (1., 3.),
            (2., 3.),
            (10., 3.),
            (20., 3.),
        ]);
        let b = data(&[
            (0., 0.),
            (1., 4.),
            (2., f64::NAN),
            (3., 4.),
            (4., 4.),
            (19., 4.),
        ]);
        let output = FilterSpec::Magnitude.apply_inputs(&[&a, &b]).unwrap();
        let y: Vec<_> = output.samples().iter().map(|p| p.y).collect();
        assert!(y[0].is_nan());
        assert_eq!(y[1], 3.);
        assert!((y[2] - 13.0_f64.sqrt()).abs() < 1e-12);
        assert_eq!(y[3], 5.);
        assert!(y[4..].iter().all(|v| v.is_nan()));
    }
    #[test]
    fn quaternion_matches_supplied_wxyz_formulas_including_mixed_rotation() {
        let q = [0.8, 0.2, -0.4, 0.4]; // unit quaternion, all components active
        let inputs: Vec<_> = q.iter().map(|&v| data(&[(0., v)])).collect();
        let refs: Vec<_> = inputs.iter().collect();
        let expected = [0.0_f64.atan2(0.6), (-0.8_f64).asin(), 0.48_f64.atan2(0.36)];
        for (axis, expected) in RpyAxis::ALL.into_iter().zip(expected) {
            let output = FilterSpec::Quaternion(axis, AngleUnit::Radians)
                .apply_inputs(&refs)
                .unwrap();
            assert!((output.samples()[0].y - expected).abs() < 1e-12, "{axis:?}");
            let degrees = FilterSpec::Quaternion(axis, AngleUnit::Degrees)
                .apply_inputs(&refs)
                .unwrap();
            assert!(
                (degrees.samples()[0].y - expected.to_degrees()).abs() < 1e-12,
                "{axis:?}"
            );
        }
        let zeros = data(&[(0., 0.)]);
        assert!(
            FilterSpec::Quaternion(RpyAxis::Yaw, AngleUnit::Radians)
                .apply_inputs(&[&zeros; 4])
                .unwrap()
                .samples()[0]
                .y
                .is_nan()
        );
        assert!(
            FilterSpec::Quaternion(RpyAxis::Roll, AngleUnit::Radians)
                .apply_inputs(&[&zeros])
                .is_err()
        );
    }
    #[test]
    fn matching_timestamps_keep_duplicate_samples_and_large_magnitudes_stay_finite() {
        let a = data(&[(0., 3.), (0., 6.), (1., 1e30)]);
        let b = data(&[(0., 4.), (0., 8.), (1., 1e30)]);
        let output = FilterSpec::Magnitude.apply_inputs(&[&a, &b]).unwrap();
        assert_eq!(output.samples()[0].y, 5.);
        assert_eq!(output.samples()[1].y, 10.);
        assert!(output.samples()[2].y.is_finite());
        let huge = data(&[(0., 1e300)]);
        assert!(
            FilterSpec::Magnitude
                .apply_inputs(&[&huge, &huge])
                .unwrap()
                .samples()[0]
                .y
                .is_finite()
        );
    }
}
