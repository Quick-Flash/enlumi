pub const OUTPUTS: usize = 1;
#[derive(Default)]
pub struct State {}
pub fn filter(_time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    for (i, out) in outputs[0].iter_mut().enumerate() {
        *out = inputs[0][i] - inputs[1][i];
    }
}
