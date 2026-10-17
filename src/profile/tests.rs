use super::*;
use crate::log::LogStore;
use serde_json::json;
use std::sync::{Arc, atomic::AtomicBool};

fn fixture() -> LogStore {
    LogStore::read(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    )))
    .unwrap()
}

fn recipe() -> serde_json::Value {
    json!({
        "version":1, "name":"Fixture profile",
        "signals":[{"id":"rpm","source":"esc_status/0/esc[5].esc_rpm","metadata":{"name":"ESC 6 RPM","unit":"rpm","axis":"motor6"}}],
        "filters":[
            {"id":"second","metadata":{"name":"Second stage"},"inputs":["$first"],"operation":{"kind":"affine","gain":2}},
            {"id":"first","metadata":{"name":"First stage"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":{"ratio":[1,{"setting":{"name":"CAL_GYRO0_XSCALE","occurrence":0}}]}}}
        ],
        "geometry":[{"id":"region","name":"Named flight interval","shape":{"kind":"band","start":0,"end":1}}],
        "views":[{"name":"Motor view","fields":["$rpm","$second"],"geometry":["region"]}]
    })
}

fn prepare_json(value: serde_json::Value, log: &LogStore) -> Result<prepare::Prepared, String> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("profile.json");
    std::fs::write(&path, value.to_string()).unwrap();
    prepare::prepare(
        load(&path)?,
        log,
        directory.path(),
        100,
        Arc::new(AtomicBool::new(false)),
    )
}

#[test]
fn profiles_resolve_settings_forward_dependencies_metadata_and_views() {
    let log = fixture();
    let prepared = prepare_json(recipe(), &log).unwrap();
    assert_eq!(prepared.fields.len(), 2);
    assert_eq!(
        prepared.metadata["esc_status/0/esc[5].esc_rpm"].label(),
        "ESC 6 RPM [rpm]"
    );
    let data = prepared.fields[0].data.as_ref().unwrap();
    let raw = log.prepare("esc_status/0/esc[5].esc_rpm").unwrap();
    assert_eq!(data.samples().len(), raw.samples().len());
    assert_eq!(prepared.geometry[0].name, "Named flight interval");
    assert_eq!(prepared.views[0].fields.len(), 2);
    assert_eq!(prepared.views[0].geometry, vec![102]);
}

#[test]
fn numeric_settings_are_regular_filter_sources() {
    let log = fixture();
    let source = resolve::setting(
        &log,
        &Setting {
            name: "CAL_GYRO0_XSCALE".into(),
            flight: 1,
            occurrence: Some(0),
            element: None,
        },
    )
    .unwrap();
    assert!(log.catalogue.iter().any(|entry| entry.id == source));
    let data = log.prepare(&source).unwrap();
    assert!(data.samples().iter().all(|point| point.y.is_finite()));
    assert_eq!(
        data.samples().first().unwrap().y,
        data.samples().last().unwrap().y
    );
    let mut value = recipe();
    value["filters"][0]["inputs"] = json!([{"setting":{"name":"CAL_GYRO0_XSCALE","occurrence":0}}]);
    let prepared = prepare_json(value, &log).unwrap();
    assert_eq!(prepared.fields[0].sources, vec![source]);
}

#[test]
fn profile_failures_report_missing_fields_cycles_zero_division_and_typos() {
    let log = fixture();
    let mut value = recipe();
    value["filters"][0]["inputs"] = json!(["missing/0/field"]);
    assert!(
        prepare_json(value, &log)
            .err()
            .unwrap()
            .contains("Missing field")
    );
    let mut value = recipe();
    value["filters"][1]["inputs"] = json!(["$second"]);
    assert!(prepare_json(value, &log).err().unwrap().contains("cycle"));
    let mut value = recipe();
    value["filters"][0]["operation"]["gain"] = json!({"ratio":[1,0]});
    assert!(prepare_json(value, &log).err().unwrap().contains("zero"));
    let mut value = recipe();
    value["typo"] = json!(true);
    assert!(prepare_json(value, &log).is_err());
    let mut value = recipe();
    value["filters"][0]["inputs"] = json!([{"setting":{"name":"CAL_GYRO0_XSCALE"},"typo":true}]);
    assert!(prepare_json(value, &log).is_err());
    let mut value = recipe();
    value["requires_settings"] =
        json!([{"setting":{"name":"CAL_GYRO0_XSCALE","occurrence":0},"equals":"not the value"}]);
    assert!(
        prepare_json(value, &log)
            .err()
            .unwrap()
            .contains("requirement")
    );
}

#[test]
fn settings_never_guess_between_duplicate_records() {
    let mut log = fixture();
    let setting = log.settings[&1]
        .iter()
        .find(|setting| setting.name == "CAL_GYRO0_XSCALE")
        .unwrap()
        .clone();
    log.settings.get_mut(&1).unwrap().push(setting);
    assert!(
        resolve::setting(
            &log,
            &Setting {
                name: "CAL_GYRO0_XSCALE".into(),
                flight: 1,
                occurrence: None,
                element: None,
            }
        )
        .unwrap_err()
        .contains("occurrence")
    );
}

#[test]
fn state_band_profiles_preserve_names_and_unmapped_states() {
    let log = fixture();
    let source = log
        .catalogue
        .iter()
        .find(|entry| entry.id.starts_with("@setting/"))
        .unwrap()
        .id
        .clone();
    let value = log.prepare(&source).unwrap().samples()[0].y;
    assert_eq!(value.fract(), 0.0);
    let mut profile = recipe();
    profile["geometry"][0]["shape"] = json!({"kind":"states","source":source,"rules":[{"value":value as i64,"name":"Named active state","color":[10,20,30]}]});
    let prepared = prepare_json(profile, &log).unwrap();
    let states = prepared.geometry[0].states.as_ref().unwrap();
    assert!(
        states
            .rules
            .iter()
            .any(|rule| rule.name == "Named active state")
    );
    assert!(!states.intervals.is_empty());
}

#[test]
#[ignore = "requires wasm32-unknown-unknown target"]
fn profile_rust_filter_uses_recorded_setting_as_numeric_input() {
    let log = fixture();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("sum.rs"),
        crate::script_compile::EXAMPLE,
    )
    .unwrap();
    let mut value = recipe();
    value["filters"] = json!([{"id":"sum","metadata":{"name":"RPM plus calibration"},"inputs":["$rpm",{"setting":{"name":"CAL_GYRO0_XSCALE","occurrence":0}}],"operation":{"kind":"rust","path":"sum.rs"}}]);
    value["views"][0]["fields"] = json!(["$sum"]);
    let path = directory.path().join("profile.json");
    std::fs::write(&path, value.to_string()).unwrap();
    let prepared = prepare::prepare(
        load(&path).unwrap(),
        &log,
        directory.path(),
        1,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let raw = log.prepare("esc_status/0/esc[5].esc_rpm").unwrap();
    let settings = resolve::setting(
        &log,
        &Setting {
            name: "CAL_GYRO0_XSCALE".into(),
            flight: 1,
            occurrence: Some(0),
            element: None,
        },
    )
    .unwrap();
    let offset = log.prepare(&settings).unwrap().samples()[0].y;
    let output = prepared.fields[0].data.as_ref().unwrap();
    for (actual, raw) in output
        .samples()
        .iter()
        .zip(raw.samples())
        .filter(|(_, raw)| raw.x >= 0.0 && raw.x <= log.duration)
    {
        assert!((actual.y - raw.y - offset).abs() < 1e-6);
    }
}
