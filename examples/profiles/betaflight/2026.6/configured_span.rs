// SPDX-License-Identifier: AGPL-3.0-or-later
// Inputs: decoded motor output, motorOutput[0], motorOutput[1].
// Percentage of configured span; no thrust estimate or protocol-specific defaults.
pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    if outputs[0].is_empty() {
        return;
    }
    let low = inputs[1][0];
    let high = inputs[2][0];
    assert!(
        low.is_finite() && high.is_finite() && high > low,
        "motorOutput needs finite increasing bounds"
    );
    for (i, value) in outputs[0].iter_mut().enumerate() {
        *value = ((inputs[0][i] - inputs[1][i]) * 100.0 / (inputs[2][i] - inputs[1][i]))
            .clamp(0.0, 100.0);
    }
}
