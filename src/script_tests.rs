use crate::{derived::FilterSpec, script::Script, script_compile, script_state::output_names};
use plot::{PlotPoint, time_series::SeriesData};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn cancelled() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
fn data(len: usize) -> SeriesData {
    SeriesData::new(
        (0..len)
            .map(|i| PlotPoint {
                x: i as f64 * 0.01,
                y: (i as f64 * 0.01).sin(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn script(body: &str, outputs: usize) -> Script {
    Script {
        source: format!(
            r#"
#[derive(Default)]
pub struct State {{ pub count: usize, pub previous: f64 }}
pub const OUTPUTS: usize = {outputs};
pub fn filter(time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], state: &mut State) {{
    {body}
}}
"#
        ),
        outputs,
    }
}

#[test]
fn script_schema_rejects_empty_duplicate_and_excessive_outputs() {
    assert_eq!(
        output_names(" First \nSecond ").unwrap(),
        ["First", "Second"]
    );
    for text in ["", "one\n\ntwo", "same\nSAME"] {
        assert!(output_names(text).is_err());
    }
    assert!(
        output_names(
            &(0..17)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
        .is_err()
    );
    assert!(script("", 0).validate(1).is_err());
    assert!(script("", 1).validate(0).is_err());
    assert!(script("", 17).validate(1).is_err());
    assert!(script("", 1).validate(65).is_err());
    let invalid = FilterSpec::Rust(Arc::new(script("", 1)), 1);
    assert!(invalid.validate().is_err());
}

#[test]
fn script_compilation_checks_cancel_before_spawning_compiler() {
    let cancel = AtomicBool::new(true);
    assert!(
        script_compile::compile(script_compile::EXAMPLE, &cancel)
            .unwrap_err()
            .contains("cancelled")
    );
}

#[test]
#[ignore = "requires rustc with wasm32-unknown-unknown installed"]
fn script_e2e_state_multi_output_alignment_and_runtime_limits() {
    let input = data(9001);
    let program = script(
        "for i in 0..time.len() { state.count += 1; outputs[0][i] = inputs[0][i] * 2.0; outputs[1][i] = state.count as f64; }",
        2,
    );
    let started = Instant::now();
    let result = program.apply(&[&input], cancelled()).unwrap();
    eprintln!(
        "Rust compile + WASM run (9001 samples, 2 outputs): {:?}",
        started.elapsed()
    );
    assert_eq!(result.len(), 2);
    for (i, point) in input.samples().iter().enumerate() {
        assert_eq!(
            result[0].samples()[i],
            PlotPoint {
                x: point.x,
                y: point.y * 2.0
            }
        );
        assert_eq!(result[1].samples()[i].y, (i + 1) as f64);
    }
    let repeated = program.apply(&[&input], cancelled()).unwrap();
    assert_eq!(repeated[1].samples()[0].y, 1.0, "State resets per run");
    assert!(
        program
            .apply(&[&data(0)], cancelled())
            .unwrap()
            .iter()
            .all(|data| data.samples().is_empty())
    );

    let a = SeriesData::new(vec![
        PlotPoint { x: 0.0, y: 3.0 },
        PlotPoint { x: 0.5, y: 3.0 },
        PlotPoint { x: 1.0, y: 3.0 },
        PlotPoint { x: 2.0, y: 3.0 },
    ])
    .unwrap();
    let b = SeriesData::new(vec![
        PlotPoint { x: 0.0, y: 0.0 },
        PlotPoint { x: 1.0, y: 4.0 },
    ])
    .unwrap();
    let norm = script(
        "for i in 0..time.len() { outputs[0][i] = inputs[0][i].hypot(inputs[1][i]); }",
        1,
    );
    let result = norm.apply(&[&a, &b], cancelled()).unwrap();
    let native = FilterSpec::Magnitude.apply_inputs(&[&a, &b]).unwrap();
    for (a, b) in result[0].samples().iter().zip(native.samples()) {
        assert!(a.y == b.y || a.y.is_nan() && b.y.is_nan());
    }

    // Starter iterates every field and every sample, preserving missing data.
    let starter = Script {
        source: script_compile::EXAMPLE.into(),
        outputs: 1,
    };
    let sum = starter.apply(&[&a, &b], cancelled()).unwrap();
    for (actual, expected) in sum[0].samples().iter().zip([3.0, 5.0, 7.0, f64::NAN]) {
        assert!(actual.y == expected || actual.y.is_nan() && expected.is_nan());
    }
    let copy = starter.apply(&[&input], cancelled()).unwrap();
    assert_eq!(copy[0].samples(), input.samples());

    let unfinished = script("outputs[0][0] = f64::INFINITY;", 1)
        .apply(&[&data(10)], cancelled())
        .unwrap();
    assert!(unfinished[0].samples().iter().all(|point| point.y.is_nan()));
    let bad = script("this is invalid Rust;", 1)
        .apply(&[&input], cancelled())
        .unwrap_err();
    assert!(bad.contains("filter.rs:") && bad.contains("error"), "{bad}");
    let trap = script("outputs[0][time.len()] = 0.0;", 1)
        .apply(&[&input], cancelled())
        .unwrap_err();
    assert!(trap.contains("Rust filter failed"), "{trap}");
    let memory = script("let size = std::hint::black_box(256 * 1024 * 1024); let v = vec![0u8; size]; std::hint::black_box(v);", 1).apply(&[&input], cancelled()).unwrap_err();
    assert!(memory.contains("Rust filter failed"), "{memory}");

    let spin = script(
        "loop { state.count = std::hint::black_box(state.count.wrapping_add(1)); }",
        1,
    );
    // Warm compiler first, then test interruption of code already executing.
    script_compile::compile(&spin.source, &AtomicBool::new(false)).unwrap();
    let cancel = cancelled();
    let worker_cancel = cancel.clone();
    let worker = std::thread::spawn(move || spin.apply(&[&input], worker_cancel));
    std::thread::sleep(Duration::from_millis(1));
    cancel.store(true, Ordering::Relaxed);
    let started = Instant::now();
    let error = worker.join().unwrap().unwrap_err();
    assert!(
        error.contains("cancelled") || error.contains("fuel"),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
#[ignore = "requires rustc with wasm32-unknown-unknown installed"]
fn script_e2e_million_sample_benchmark_matches_native_transforms() {
    let input = data(1_000_000);
    let magnitude = script(
        "for i in 0..time.len() { outputs[0][i] = inputs.iter().fold(0.0_f64, |norm, input| norm.hypot(input[i])); }",
        1,
    );
    script_compile::compile(&magnitude.source, &AtomicBool::new(false)).unwrap();
    let started = Instant::now();
    let result = magnitude
        .apply(&[&input, &input, &input], cancelled())
        .unwrap();
    eprintln!(
        "Cached WASM magnitude, 1M samples / 3 inputs: {:?}",
        started.elapsed()
    );
    let native = FilterSpec::Magnitude
        .apply_inputs(&[&input, &input, &input])
        .unwrap();
    for (a, b) in result[0].samples().iter().zip(native.samples()) {
        assert!((a.y - b.y).abs() < 1e-12);
    }

    let lowpass = script(
        "for i in 0..time.len() { state.previous += 0.1 * (inputs[0][i] - state.previous); outputs[0][i] = state.previous; }",
        1,
    );
    script_compile::compile(&lowpass.source, &AtomicBool::new(false)).unwrap();
    let started = Instant::now();
    let result = lowpass.apply(&[&input], cancelled()).unwrap();
    eprintln!(
        "Cached WASM stateful low-pass, 1M samples: {:?}",
        started.elapsed()
    );
    let mut previous = 0.0;
    for (a, b) in result[0].samples().iter().zip(input.samples()) {
        previous += 0.1 * (b.y - previous);
        assert_eq!(a.y, previous);
    }

    let quaternion = script(
        "for i in 0..time.len() { let (w,x,y,z) = (inputs[0][i],inputs[1][i],inputs[2][i],inputs[3][i]); outputs[0][i] = (2.0*(w*x+y*z)).atan2(w*w-x*x-y*y+z*z); outputs[1][i] = (-(2.0*(x*z-w*y))).clamp(-1.0,1.0).asin(); outputs[2][i] = (2.0*(x*y+w*z)).atan2(w*w+x*x-y*y-z*z); }",
        3,
    );
    script_compile::compile(&quaternion.source, &AtomicBool::new(false)).unwrap();
    let started = Instant::now();
    let result = quaternion.apply(&[&input; 4], cancelled()).unwrap();
    eprintln!(
        "Cached WASM quaternion, 1M samples / 3 outputs: {:?}",
        started.elapsed()
    );
    for (index, axis) in crate::derived::RpyAxis::ALL.into_iter().enumerate() {
        let native = FilterSpec::Quaternion(axis, crate::derived::AngleUnit::Radians)
            .apply_inputs(&[&input; 4])
            .unwrap();
        // At all-zero quaternion native deliberately returns NaN.
        for (a, b) in result[index].samples().iter().zip(native.samples()).skip(1) {
            assert!((a.y - b.y).abs() < 1e-12);
        }
    }
}

#[test]
fn script_contract_rejects_mismatched_counts_before_compilation() {
    let mut program = script("", 2);
    assert!(program.validate(1).is_ok());
    program.outputs = 1;
    let error = program.validate(1).unwrap_err();
    assert!(
        error.contains("declares 2") && error.contains("1 named rows"),
        "{error}"
    );
    for code in [
        "// pub const OUTPUTS: usize = 1;",
        "const OUTPUTS: usize = 1;",
        "pub const OUTPUTS: usize = 17;",
        "pub const OUTPUTS: usize = 0;",
        "pub const OUTPUTS: usize = 1 + 1;",
        "#[cfg(any())] pub const OUTPUTS: usize = 1;",
        "pub const OUTPUTS: usize = 1; pub const OUTPUTS: usize = 2;",
        "const TEXT: &str = \"pub const OUTPUTS: usize = 1;\";",
    ] {
        assert!(
            crate::script_contract::output_count(code).is_err(),
            "{code}"
        );
    }
    assert_eq!(
        crate::script_contract::output_count("/* OUTPUTS = 9 */ pub const OUTPUTS: usize = 2;")
            .unwrap(),
        2
    );
}
