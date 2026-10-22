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
fn streaming_profile_publishes_raw_then_dependency_waves_and_honors_cancel() {
    use std::sync::{Mutex, atomic::Ordering};
    let log = fixture();
    let profile: Profile = serde_json::from_value(recipe()).unwrap();
    let ids = identity::ResourceIds {
        fields: BTreeMap::from([("first".into(), 100), ("second".into(), 101)]),
        geometry: BTreeMap::from([("region".into(), 102)]),
    };
    let snapshots = Mutex::new(Vec::new());
    let prepared = prepare::prepare_streaming(
        profile.clone(),
        &log,
        Path::new("."),
        ids.clone(),
        Arc::new(AtomicBool::new(false)),
        &|progress| {
            snapshots.lock().unwrap().push(progress);
        },
    )
    .unwrap();
    let snapshots = snapshots.into_inner().unwrap();
    let raw = "esc_status/0/esc[5].esc_rpm";
    let first = &prepared.symbols["first"];
    let second = &prepared.symbols["second"];
    let at = |source: &str| {
        snapshots
            .iter()
            .position(|snapshot| snapshot.data.contains_key(source))
            .unwrap()
    };
    assert!(at(raw) < at(first) && at(first) < at(second));
    for field in &prepared.fields {
        assert_eq!(
            snapshots.last().unwrap().data[&field.key()].samples(),
            field.data.as_ref().unwrap().samples()
        );
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let result =
        prepare::prepare_streaming(profile, &log, Path::new("."), ids, cancel.clone(), &|_| {
            cancel.store(true, Ordering::Relaxed);
        });
    assert!(result.err().unwrap().contains("cancelled"));
}

#[test]
fn captured_filter_aliases_reuse_identical_outputs_and_keep_dependents_available() {
    let mut value = recipe();
    value["filters"] = json!([
        {"id":"original","metadata":{"name":"Scaled"},"inputs":["esc_status/0/esc[5].esc_rpm"],"operation":{"kind":"affine","gain":2}},
        {"id":"copy","metadata":{"name":"Scaled"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":2.0,"offset":0.0}},
        {"id":"child","metadata":{"name":"Child"},"inputs":["$copy"],"operation":{"kind":"affine","gain":3}}
    ]);
    value["views"][0]["fields"] = json!(["$copy", "$child"]);
    let prepared = prepare_json(value, &fixture()).unwrap();
    assert_eq!(prepared.fields.len(), 2);
    assert_eq!(prepared.ids.fields["original"], prepared.ids.fields["copy"]);
    assert!(prepared.diagnostics.is_empty());
    assert_eq!(prepared.views[0].fields.len(), 2);
}

fn prepare_on_threads(
    value: serde_json::Value,
    log: &LogStore,
    base: &Path,
    threads: usize,
) -> prepare::Prepared {
    let profile: Profile = serde_json::from_value(value).unwrap();
    let ids = identity::ResourceIds {
        fields: profile
            .filters
            .iter()
            .enumerate()
            .map(|(index, field)| (field.id.clone(), index as u64 + 100))
            .collect(),
        geometry: profile
            .geometry
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.clone(), index as u64 + 1000))
            .collect(),
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
        .install(|| {
            prepare::prepare_inner(profile, log, base, ids, Arc::new(AtomicBool::new(false)))
                .unwrap()
        })
}

#[test]
fn parallel_profile_branches_preserve_dependencies_aliases_and_diagnostics() {
    let log = fixture();
    let mut value = recipe();
    value["filters"] = json!([
        {"id":"joined","metadata":{"name":"Joined"},"inputs":["$left","$right"],"operation":{"kind":"magnitude"}},
        {"id":"copy_child","metadata":{"name":"Copy child"},"inputs":["$copy"],"operation":{"kind":"affine","gain":7}},
        {"id":"left","metadata":{"name":"Left"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":2}},
        {"id":"right","metadata":{"name":"Right"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":3}},
        {"id":"copy","metadata":{"name":"Left"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":2}},
        {"id":"conflict","metadata":{"name":"Right"},"inputs":["$rpm"],"operation":{"kind":"affine","gain":9}},
        {"id":"broken","metadata":{"name":"Broken"},"inputs":["missing/0/field"],"operation":{"kind":"affine","gain":1}},
        {"id":"dependent","metadata":{"name":"Unavailable child"},"inputs":["$broken"],"operation":{"kind":"affine","gain":2}},
        {"id":"cyclic","metadata":{"name":"Cycle"},"inputs":["$cyclic"],"operation":{"kind":"affine","gain":2}}
    ]);
    value["views"][0]["fields"] = json!(["$joined", "$copy_child", "$dependent"]);
    let serial = prepare_on_threads(value.clone(), &log, Path::new("."), 1);
    let parallel = prepare_on_threads(value, &log, Path::new("."), 4);
    assert_eq!(serial.fields.len(), 4);
    assert_eq!(parallel.fields.len(), serial.fields.len());
    assert_eq!(parallel.ids.fields, serial.ids.fields);
    assert_eq!(parallel.warnings, serial.warnings);
    assert_eq!(parallel.views[0].fields.len(), 2);
    for (actual, expected) in parallel.fields.iter().zip(&serial.fields) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(
            actual.data.as_ref().unwrap().samples(),
            expected.data.as_ref().unwrap().samples()
        );
    }
    let joined = parallel
        .fields
        .iter()
        .find(|field| field.name == "Joined")
        .unwrap();
    let raw = log.prepare("esc_status/0/esc[5].esc_rpm").unwrap();
    for (actual, source) in joined
        .data
        .as_ref()
        .unwrap()
        .samples()
        .iter()
        .zip(raw.samples())
    {
        assert!((actual.y - (source.y * 2.).hypot(source.y * 3.)).abs() < 1e-6);
    }
}

#[test]
#[ignore = "set ENLUMI_BENCH_LOG and ENLUMI_BENCH_PROFILE; requires Rust WASM toolchain for Rust filters"]
fn profile_parallel_loading_benchmark() {
    use std::time::Instant;
    let path = std::path::PathBuf::from(
        std::env::var_os("ENLUMI_BENCH_PROFILE").expect("ENLUMI_BENCH_PROFILE"),
    );
    let log_path =
        std::path::PathBuf::from(std::env::var_os("ENLUMI_BENCH_LOG").expect("ENLUMI_BENCH_LOG"));
    let profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let log = LogStore::read(&log_path).unwrap();
    let base = path.parent().unwrap();
    // Warm compiler cache before comparing worker counts; each run decodes raw inputs anew.
    let started = Instant::now();
    let cold = prepare_on_threads(profile.clone(), &log, base, 4);
    eprintln!(
        "Cold four-worker preparation: {:?}; {} outputs, {} diagnostics",
        started.elapsed(),
        cold.fields.len(),
        cold.diagnostics.len()
    );
    drop(cold);
    let mut reference = None;
    for threads in [1, 4, 1, 4] {
        let started = Instant::now();
        let prepared = prepare_on_threads(profile.clone(), &log, base, threads);
        eprintln!(
            "Warm {threads}-worker preparation: {:?}; {} outputs, {} diagnostics",
            started.elapsed(),
            prepared.fields.len(),
            prepared.diagnostics.len()
        );
        let snapshot = prepared
            .fields
            .iter()
            .map(|field| {
                let samples = field.data.as_ref().unwrap().samples();
                let points: Vec<_> = [0, samples.len() / 2, samples.len().saturating_sub(1)]
                    .into_iter()
                    .filter_map(|index| samples.get(index))
                    .map(|point| (point.x.to_bits(), point.y.to_bits()))
                    .collect();
                (field.id, samples.len(), points)
            })
            .collect::<Vec<_>>();
        let actual = (snapshot, prepared.warnings);
        if let Some(expected) = &reference {
            assert_eq!(&actual, expected);
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn identical_geometry_with_distinct_plot_styles_keeps_separate_identities() {
    let mut value = recipe();
    value["version"] = json!(2);
    value["views"] = json!([]);
    value["geometry"] = json!([
        {"id":"a","name":"Limit","shape":{"kind":"line","coordinates":[0,1,1,1]}},
        {"id":"b","name":"Limit","shape":{"kind":"line","coordinates":[0,1,1,1]}},
        {"id":"c","name":"Limit","shape":{"kind":"line","coordinates":[0,1,1,1]}}
    ]);
    value["plot_presets"] = json!([
        {"name":"Default","lines":[],"geometry":["a","c"],"analysis":null},
        {"name":"Styled","lines":[],"geometry":["b"],"geometry_style":{"b":{"width":3,"shade":"None","extend":true,"orientation":"Horizontal","labels":false,"hidden":false}},"analysis":null}
    ]);
    let prepared = prepare_json(value, &fixture()).unwrap();
    assert_eq!(prepared.geometry.len(), 2);
    assert_eq!(prepared.ids.geometry["a"], prepared.ids.geometry["c"]);
    assert_ne!(prepared.ids.geometry["a"], prepared.ids.geometry["b"]);
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
fn profiles_skip_failed_resources_but_reject_schema_errors_and_firmware_mismatches() {
    let log = fixture();
    let mut value = recipe();
    value["filters"][0]["inputs"] = json!(["missing/0/field"]);
    assert!(
        prepare_json(value, &log)
            .unwrap()
            .warnings
            .iter()
            .any(|warning| warning.contains("Missing field"))
    );
    let mut value = recipe();
    value["filters"][1]["inputs"] = json!(["$second"]);
    assert!(
        prepare_json(value, &log)
            .unwrap()
            .warnings
            .iter()
            .any(|warning| warning.contains("cyclic"))
    );
    let mut value = recipe();
    value["filters"][0]["operation"]["gain"] = json!({"ratio":[1,0]});
    assert!(
        prepare_json(value, &log)
            .unwrap()
            .warnings
            .iter()
            .any(|warning| warning.contains("zero"))
    );
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
            .contains("profile requires")
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

#[test]
fn unavailable_dependencies_share_root_cause_with_affected_view() {
    let log = fixture();
    let mut value = recipe();
    value["filters"][0]["inputs"] = json!(["missing/0/field"]);
    let prepared = prepare_json(value, &log).unwrap();
    let cause = prepared
        .diagnostics
        .iter()
        .find(|cause| cause.message == "Missing field missing/0/field")
        .unwrap();
    assert!(cause.dependents.iter().any(|name| name == "Motor view"));
    assert_eq!(
        prepared
            .diagnostics
            .iter()
            .filter(|cause| cause.message.contains("Missing field missing/0/field"))
            .count(),
        1
    );
}
