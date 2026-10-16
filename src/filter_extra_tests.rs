use crate::{derived::FilterSpec, filter, filter_extra, filter_zero_phase};
use plot::{PlotPoint, time_series::SeriesData};
fn data(values: &[(f64, f64)]) -> SeriesData {
    SeriesData::new(
        values
            .iter()
            .map(|&(x, y)| PlotPoint { x, y })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}
#[test]
fn average_and_calculus_respect_missing_duplicate_and_gapped_samples() {
    let input = data(&[
        (0.0, 1.0),
        (0.1, 3.0),
        (0.2, f64::NAN),
        (0.3, 7.0),
        (0.3, 9.0),
        (0.4, 11.0),
        (10.0, 13.0),
        (10.1, 15.0),
    ]);
    let average = filter_extra::moving_average(&input, 2).unwrap();
    for (point, expected) in
        average
            .samples()
            .iter()
            .zip([1.0, 2.0, f64::NAN, 7.0, 9.0, 10.0, 13.0, 14.0])
    {
        if expected.is_nan() {
            assert!(point.y.is_nan());
        } else {
            close(point.y, expected);
        }
    }
    let integral = filter_extra::integral(&input, 5.0).unwrap();
    for (i, expected) in [
        (0, 5.0),
        (1, 5.2),
        (3, 5.0),
        (4, 5.0),
        (5, 6.0),
        (6, 5.0),
        (7, 6.4),
    ] {
        close(integral.samples()[i].y, expected);
    }
    let derivative = filter_extra::derivative(&input).unwrap();
    assert!(derivative.samples()[2].y.is_nan() && derivative.samples()[3].y.is_nan());
    for i in [0, 1, 4, 5, 6, 7] {
        close(derivative.samples()[i].y, 20.0);
    }
    assert_eq!(input.samples()[0].y, 1.0);
}
#[test]
fn derivative_and_trapezoids_use_irregular_timestamps() {
    let quadratic = data(&[
        (0.0, 0.0),
        (0.1, 0.01),
        (0.4, 0.16),
        (0.9, 0.81),
        (1.2, 1.44),
    ]);
    let derivative = filter_extra::derivative(&quadratic).unwrap();
    for point in &derivative.samples()[1..4] {
        close(point.y, 2.0 * point.x);
    }
    let linear = data(&[(0.0, 1.0), (0.1, 1.2), (0.4, 1.8), (0.9, 2.8)]);
    let integral = filter_extra::integral(&linear, 3.0).unwrap();
    for point in integral.samples() {
        close(point.y, 3.0 + point.x + point.x * point.x);
    }
}
#[test]
fn new_filters_validate_parameters_and_input_counts() {
    for spec in [
        FilterSpec::ZeroPhaseLowPass(0.0),
        FilterSpec::ZeroPhaseHighPass(f64::NAN),
        FilterSpec::MovingAverage(0),
        FilterSpec::MovingAverage(1_000_001),
        FilterSpec::Integral(f64::INFINITY),
    ] {
        assert!(spec.validate().is_err());
    }
    for spec in [
        FilterSpec::ZeroPhaseLowPass(5.0),
        FilterSpec::ZeroPhaseHighPass(5.0),
        FilterSpec::MovingAverage(3),
        FilterSpec::Derivative,
        FilterSpec::Integral(0.0),
    ] {
        assert!(spec.validate().is_ok());
        assert!(spec.validate_inputs(1).is_ok());
        assert!(spec.validate_inputs(0).is_err());
        assert!(spec.validate_inputs(2).is_err());
        assert!(spec.apply(&data(&[])).unwrap().samples().is_empty());
    }
}
#[test]
fn zero_phase_preserves_constants_timestamps_and_short_segments() {
    let input = SeriesData::new(
        (0..100)
            .map(|i| PlotPoint {
                x: i as f64 * 0.01,
                y: 7.0,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for high in [false, true] {
        let result = filter_zero_phase::apply(&input, 2.0, high).unwrap();
        for (raw, point) in input.samples().iter().zip(result.samples()) {
            assert_eq!(raw.x, point.x);
            close(point.y, if high { 0.0 } else { 7.0 });
        }
        let short = filter_zero_phase::apply(&data(&[(0.0, 7.0)]), 2.0, high).unwrap();
        close(short.samples()[0].y, if high { 0.0 } else { 7.0 });
    }
}
#[test]
fn zero_phase_filters_each_segment_without_bridging_gaps_or_duplicates() {
    let input = data(&[
        (0.0, 1.0),
        (0.1, 2.0),
        (0.2, 4.0),
        (0.3, f64::NAN),
        (1.0, 100.0),
        (1.1, 110.0),
        (1.1, 7.0),
        (1.2, 8.0),
        (20.0, 9.0),
    ]);
    for high in [false, true] {
        let result = filter_zero_phase::apply(&input, 2.0, high).unwrap();
        assert!(result.samples()[3].y.is_nan());
        for range in [0..3, 4..6, 6..8, 8..9] {
            let isolated = SeriesData::new(input.samples()[range.clone()].to_vec()).unwrap();
            let expected = filter_zero_phase::apply(&isolated, 2.0, high).unwrap();
            for (actual, expected) in result.samples()[range].iter().zip(expected.samples()) {
                close(actual.y, expected.y);
            }
        }
    }
}
#[test]
fn forward_backward_passes_remove_phase_lag_for_low_and_high_pass() {
    let input = SeriesData::new(
        (0..6001)
            .map(|i| {
                let x = i as f64 * 0.01;
                PlotPoint {
                    x,
                    y: (std::f64::consts::TAU * x).sin(),
                }
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let causal = filter::low_pass(&input, 5.0).unwrap();
    let quadrature = |series: &SeriesData| {
        series.samples()[1000..5000]
            .iter()
            .map(|point| point.y * (std::f64::consts::TAU * point.x).cos())
            .sum::<f64>()
            / 2000.0
    };
    assert!(quadrature(&causal).abs() > 0.01);
    for high in [false, true] {
        let result = filter_zero_phase::apply(&input, 5.0, high).unwrap();
        assert!(
            quadrature(&result).abs() < 1e-10,
            "Phase lag remained for high={high}: {}",
            quadrature(&result)
        );
        let in_phase = result.samples()[1000..5000]
            .iter()
            .map(|point| point.y * (std::f64::consts::TAU * point.x).sin())
            .sum::<f64>();
        assert!(in_phase > 0.0, "Filter must not invert signal");
    }
}
