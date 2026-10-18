// SPDX-License-Identifier: AGPL-3.0-or-later
// Inputs: rate setpoint, measured rate. Both already in deg/s.
pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    for (i, value) in outputs[0].iter_mut().enumerate() {
        *value = inputs[0][i] - inputs[1][i];
    }
}
