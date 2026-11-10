// Enlumi Rust filter: one input, one output. Time is in seconds.
pub const OUTPUTS: usize = 1;

// Largest interval to integrate. Set above normal sample interval.
const MAX_DT: f64 = 0.1;

#[derive(Default)]
pub struct State {
    total: f64,
    previous: Option<(f64, f64)>,
}

pub fn filter(time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], state: &mut State) {
    assert_eq!(inputs.len(), 1, "Choose one input field");
    assert_eq!(outputs.len(), OUTPUTS, "Name one output");

    for (i, &t) in time.iter().enumerate() {
        let value = inputs[0][i];
        if t.is_finite() && value.is_finite() {
            if let Some((previous_time, previous_value)) = state.previous {
                let dt = t - previous_time;
                if dt > 0.0 && dt <= MAX_DT {
                    state.total += (previous_value * 0.5 + value * 0.5) * dt;
                }
            }
            state.previous = Some((t, value));
        } else {
            // Break integration history, retain accumulated total.
            state.previous = None;
        }
        outputs[0][i] = state.total;
    }
}
