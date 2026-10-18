// SPDX-License-Identifier: AGPL-3.0-or-later
// Inputs: Betaflight eRPM in hundreds, recorded motor_poles.
pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    if outputs[0].is_empty() {
        return;
    }
    let poles = inputs[1][0];
    assert!(
        poles.is_finite() && poles >= 2.0 && poles.fract() == 0.0 && poles % 2.0 == 0.0,
        "motor_poles must be a positive even integer"
    );
    for (i, value) in outputs[0].iter_mut().enumerate() {
        *value = inputs[0][i] * 200.0 / inputs[1][i];
    }
}
