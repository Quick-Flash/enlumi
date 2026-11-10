#[path = "../examples/filters/integral_hold.rs"]
mod integral_hold;

fn run(time: &[f64], values: &[f64], state: &mut integral_hold::State) -> Vec<f64> {
    let mut output = vec![f64::NAN; time.len()];
    integral_hold::filter(time, &[values], &mut [&mut output], state);
    output
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }
}

#[test]
fn trapezoids_survive_chunk_boundaries() {
    let mut state = integral_hold::State::default();
    close(&run(&[0.0, 0.01], &[0.0, 2.0], &mut state), &[0.0, 0.01]);
    close(&run(&[0.02, 0.03], &[4.0, 6.0], &mut state), &[0.04, 0.09]);
}

#[test]
fn invalid_values_and_timestamp_gaps_hold_total_without_bridging() {
    let mut state = integral_hold::State::default();
    close(
        &run(
            &[0.0, 0.01, 0.02, 0.03, 0.04],
            &[2.0, 2.0, f64::NAN, 2.0, 2.0],
            &mut state,
        ),
        &[0.0, 0.02, 0.02, 0.02, 0.04],
    );
    close(&run(&[1.0, 1.01], &[2.0, 2.0], &mut state), &[0.04, 0.06]);
    close(
        &run(&[1.02, 1.03, 1.04], &[f64::INFINITY, 2.0, 2.0], &mut state),
        &[0.06, 0.06, 0.08],
    );
}

#[test]
fn duplicate_or_backward_times_do_not_change_total() {
    let mut state = integral_hold::State::default();
    close(
        &run(&[0.0, 0.01, 0.01, 0.0, 0.01], &[2.0; 5], &mut state),
        &[0.0, 0.02, 0.02, 0.02, 0.04],
    );
}
