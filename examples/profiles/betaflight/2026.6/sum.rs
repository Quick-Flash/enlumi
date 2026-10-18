// SPDX-License-Identifier: AGPL-3.0-or-later
// Every declared PID component is required. NaN gaps propagate.
pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    for (i, value) in outputs[0].iter_mut().enumerate() {
        *value = inputs.iter().map(|input| input[i]).sum();
    }
}
