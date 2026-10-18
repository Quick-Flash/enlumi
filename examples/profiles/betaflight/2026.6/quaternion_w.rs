// SPDX-License-Identifier: AGPL-3.0-or-later
// Inputs: logged quaternion x/y/z divided by 32767. Firmware selects w >= 0.
pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    for (i, value) in outputs[0].iter_mut().enumerate() {
        let norm_squared = inputs.iter().map(|input| input[i] * input[i]).sum::<f64>();
        // Permit int16 quantization error near w = 0; reject invalid vectors.
        *value = if norm_squared.is_finite() && norm_squared <= 1.0002 {
            (1.0 - norm_squared).max(0.0).sqrt()
        } else {
            f64::NAN
        };
    }
}
