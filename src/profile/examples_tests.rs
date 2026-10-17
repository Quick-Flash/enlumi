use super::*;
use crate::log::LogStore;
use std::sync::{Arc, atomic::AtomicBool};

fn blackbox() -> LogStore {
    fn vb(mut value: u32, bytes: &mut Vec<u8>) {
        while value >= 128 {
            bytes.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        bytes.push(value as u8);
    }
    let mut names = vec!["loopIteration".to_owned(), "time".to_owned()];
    for field in ["gyroADC", "setpoint", "axisP", "axisI", "axisD"] {
        for axis in 0..3 {
            names.push(format!("{field}[{axis}]"));
        }
    }
    names.push("eRPM[0]".into());
    let signed: Vec<_> = (0..names.len())
        .map(|index| if (2..17).contains(&index) { "1" } else { "0" })
        .collect();
    let encoding: Vec<_> = signed
        .iter()
        .map(|signed| if *signed == "1" { "0" } else { "1" })
        .collect();
    let mut bytes = format!("H Product:Blackbox flight data recorder by Nicholas Sherlock\nH Data version:2\nH Firmware revision:Betaflight 4.5.0\nH blackbox_high_resolution:0\nH motor_poles:14\nH Field I name:{}\nH Field I signed:{}\nH Field I predictor:{}\nH Field I encoding:{}\n", names.join(","), signed.join(","), vec!["0";names.len()].join(","), encoding.join(",")).into_bytes();
    for frame in 0..4 {
        bytes.push(b'I');
        vb(frame + 1, &mut bytes);
        vb((frame + 1) * 1_000_000, &mut bytes);
        for index in 2..17 {
            vb(
                if index < 5 {
                    20
                } else if index < 8 {
                    40
                } else {
                    2
                },
                &mut bytes,
            );
        }
        vb(2100, &mut bytes);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flight.BBL");
    std::fs::write(&path, bytes).unwrap();
    LogStore::read(&path).unwrap()
}

fn example(name: &str, log: &LogStore) -> Result<prepare::Prepared, String> {
    let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/profiles"));
    prepare::prepare(
        load(&base.join(name))?,
        log,
        base,
        1,
        Arc::new(AtomicBool::new(false)),
    )
}

#[test]
fn example_profiles_check_actual_rpm_values_and_firmware_requirements() {
    let log = blackbox();
    let prepared = example("betaflight-4.5-motor-rpm.json", &log).unwrap();
    assert_eq!(
        prepared.fields[0].data.as_ref().unwrap().samples()[0].y,
        30_000.0
    );
    assert_eq!(
        prepared.fields[1].data.as_ref().unwrap().samples()[0].y,
        500.0
    );
    assert_eq!(prepared.views.len(), 2);
    let mut unsupported = log;
    unsupported
        .settings
        .get_mut(&1)
        .unwrap()
        .iter_mut()
        .find(|setting| setting.name == "Firmware revision")
        .unwrap()
        .value = "INAV 8".into();
    assert!(example("betaflight-4.5-motor-rpm.json", &unsupported).is_err());
    let fixture = LogStore::read(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    )))
    .unwrap();
    assert!(example("ulog-esc.json", &fixture).is_ok());
    let base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/profiles"));
    assert!(load(&base.join("ulog-mode-bands.json")).is_ok());
}

#[test]
#[ignore = "requires wasm32-unknown-unknown target"]
fn pid_example_computes_rate_error_and_rejects_high_resolution_mismatch() {
    let mut log = blackbox();
    let prepared = example("betaflight-4.5-pid.json", &log).unwrap();
    assert_eq!(prepared.fields.len(), 3);
    assert_eq!(prepared.views.len(), 6);
    for field in &prepared.fields {
        assert_eq!(field.data.as_ref().unwrap().samples()[0].y, 10.0);
    }
    log.settings
        .get_mut(&1)
        .unwrap()
        .iter_mut()
        .find(|setting| setting.name == "blackbox_high_resolution")
        .unwrap()
        .value = "1".into();
    assert!(example("betaflight-4.5-pid.json", &log).is_err());
}
