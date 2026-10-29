use super::*;
use crate::{
    derived::FilterSpec,
    log::LogSetting,
    spectral::{Mode, Options, Window as SpectralWindow},
};
use px4_ulog::full_parser::{Completeness, MultiId, ParsedData, SomeVec};
use std::collections::HashMap;
use zgui_testkit_view::Window;

fn log(scale: f64, other: bool, reverse_settings: bool) -> Arc<LogStore> {
    let fields = || {
        BTreeMap::from([
            (
                "timestamp".into(),
                SomeVec::UInt64((0..256).map(|index| index * 1000).collect()),
            ),
            (
                "value".into(),
                SomeVec::Double(
                    (0..256)
                        .map(|index| scale * (1.0 + (index as f64 / 8.0).sin()))
                        .collect(),
                ),
            ),
        ])
        .into_iter()
        .collect()
    };
    let mut messages = HashMap::from([(
        "sensor".into(),
        HashMap::from([(MultiId::new(0), fields())]),
    )]);
    if other {
        messages.insert("other".into(), HashMap::from([(MultiId::new(0), fields())]));
    }
    let mut log = LogStore::from_parsed(
        "synthetic".into(),
        0,
        ParsedData {
            completeness: Completeness::Complete,
            messages,
        },
    )
    .unwrap();
    let mut settings = vec![
        LogSetting {
            name: "Alpha".into(),
            value: (scale * 10.0).to_string(),
            kind: "Initial parameter".into(),
        },
        LogSetting {
            name: "Beta".into(),
            value: "999".into(),
            kind: "Initial parameter".into(),
        },
    ];
    if reverse_settings {
        settings.reverse();
    }
    log.settings.insert(1, settings);
    log.add_setting_signals();
    Arc::new(log)
}

fn add_line(pane: crate::state::Pane, id: u64, source: &str, log: &LogStore) {
    pane.lines.update(|lines| {
        lines.push(Line {
            id,
            source: source.into(),
            name: source.into(),
            data: log.prepare(source).unwrap(),
            revision: 0,
        })
    });
}

fn replace(workbench: Workbench, log: Arc<LogStore>) {
    let session = Session::capture(workbench, None)
        .unwrap()
        .refresh(log.clone());
    workbench.log.set(Some(log));
    workbench.restore_log_session(session);
}

#[test]
fn replacement_preserves_workspaces_views_plots_and_appearance_with_new_data() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let old = log(1.0, true, false);
    workbench.log.set(Some(old.clone()));
    let first_workspace = workbench.workspace_id();
    let first_view = workbench.bind_view();
    let first = workbench.pane();
    add_line(first, 100, "sensor/0/value", &old);
    add_line(first, 101, "other/0/value", &old);
    first.selected.set(Some(100));
    first.legend_order.set(vec![101, 100]);
    first.plot.hidden.set(BTreeSet::from(["100".into()]));
    let mut style = plot::plot_data::SeriesStyle::for_id("100");
    style.color = [12, 34, 56, 255];
    first
        .plot
        .styles
        .set(BTreeMap::from([("100".into(), style)]));
    first.background.set(Some([20, 30, 40]));
    workbench.profiles.titles.update(|titles| {
        titles.insert(first.id, "My plot".into());
    });
    let tab = workbench.add_plot(first.id, false, true);
    let layout = workbench.layout().get_untracked();
    let second_view = workbench.new_view();
    let second_pane = workbench.pane();
    add_line(second_pane, 102, "sensor/0/value", &old);
    workbench.new_blank_workspace();
    let second_workspace = workbench.workspace_id();
    let other = workbench.pane();
    add_line(other, 103, "sensor/0/value", &old);
    let old_buffer = first.lines.get_untracked()[0].data.clone();

    replace(workbench, log(3.0, false, true));
    assert_eq!(workbench.workspace_id(), second_workspace);
    assert_eq!(workbench.active().get_untracked(), other.id);
    assert_eq!(workbench.workspaces.items.with_untracked(Vec::len), 2);
    assert_eq!(first.lines.with_untracked(Vec::len), 1);
    assert_eq!(first.lines.get_untracked()[0].data.samples()[0].y, 3.0);
    assert_ne!(
        first.lines.get_untracked()[0].data.samples().as_ptr(),
        old_buffer.samples().as_ptr()
    );
    assert_eq!(other.lines.get_untracked()[0].data.samples()[0].y, 3.0);
    assert_eq!(
        second_pane.lines.get_untracked()[0].data.samples()[0].y,
        3.0
    );
    assert_eq!(first.selected.get_untracked(), Some(100));
    assert_eq!(first.legend_order.get_untracked(), vec![100]);
    assert!(
        first
            .plot
            .hidden
            .with_untracked(|hidden| hidden.contains("100"))
    );
    assert_eq!(first.plot.styles.get_untracked()["100"].color, style.color);
    assert_eq!(first.background.get_untracked(), Some([20, 30, 40]));
    assert_eq!(
        workbench.profiles.titles.get_untracked()[&first.id],
        "My plot"
    );
    assert!(!workbench.profiles.warnings.get_untracked().is_empty());
    workbench.select_workspace(first_workspace);
    assert_eq!(workbench.views.active.get_untracked(), second_view);
    workbench.switch_view(first_view.view().id);
    assert_eq!(workbench.active().get_untracked(), tab.id);
    assert_eq!(workbench.layout().get_untracked(), layout);
    assert!(workbench.find(tab.id).is_some());
}

#[test]
fn replacement_recomputes_filters_and_parameters_and_skips_only_broken_branches() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let old = log(1.0, true, false);
    workbench.log.set(Some(old.clone()));
    let field = |id, source: &str| DerivedField {
        id,
        name: format!("Filter {id}"),
        sources: vec![source.into()],
        group: id,
        filter: FilterSpec::Affine {
            gain: 2.0,
            offset: 0.0,
        },
        data: None,
    };
    workbench.derived.set(vec![
        field(100, "sensor/0/value"),
        field(101, "other/0/value"),
        field(102, "@derived/101"),
        field(103, "@setting/1/0"),
        field(104, "@derived/100"),
    ]);
    add_line(workbench.pane(), 200, "@setting/1/0", &old);
    replace(workbench, log(3.0, false, true));
    let fields = workbench.derived.get_untracked();
    assert_eq!(
        fields.iter().map(|field| field.id).collect::<BTreeSet<_>>(),
        BTreeSet::from([100, 103, 104])
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.id == 100)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .samples()[0]
            .y,
        6.0
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.id == 104)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .samples()[0]
            .y,
        12.0
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.id == 103)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .samples()[0]
            .y,
        60.0
    );
    assert_eq!(
        workbench.pane().lines.get_untracked()[0].source,
        "@setting/1/1"
    );
    assert_eq!(
        workbench.pane().lines.get_untracked()[0].data.samples()[0].y,
        30.0
    );
}

#[test]
fn replacement_recomputes_spectral_plots_without_replacing_their_ids() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let old = log(1.0, true, false);
    workbench.log.set(Some(old.clone()));
    let options = Options {
        mode: Mode::Fft,
        size: 0,
        window: SpectralWindow::Hann,
        overlap: 50,
        heatmap_bins: 40,
        start: None,
        end: None,
        remove_dc: true,
    };
    let spectrum = crate::spectral::compute(
        &old.prepare("sensor/0/value").unwrap(),
        None,
        &options,
        &AtomicBool::new(false),
    )
    .unwrap();
    let data = SeriesData::new(spectrum.points).unwrap();
    let pane = workbench.add_analysis_panel(AnalysisResult {
        title: "FFT".into(),
        units: "value".into(),
        summary: String::new(),
        series: TimeSeriesSet::new(vec![TimeSeries::new(
            "sensor/0/value",
            "Signal",
            data.clone(),
        )])
        .unwrap(),
        heatmap: None,
        mode: Mode::Fft,
        options,
        condition: String::new(),
    });
    let line_id = pane.lines.get_untracked()[0].id;
    replace(workbench, log(3.0, false, false));
    assert_eq!(workbench.active().get_untracked(), pane.id);
    assert_eq!(pane.lines.get_untracked()[0].id, line_id);
    let updated = pane.lines.get_untracked()[0].data.clone();
    let old_peak = data
        .samples()
        .iter()
        .map(|point| point.y)
        .fold(0.0, f64::max);
    let new_peak = updated
        .samples()
        .iter()
        .map(|point| point.y)
        .fold(0.0, f64::max);
    assert!((new_peak / old_peak - 3.0).abs() < 0.0001);
}

#[test]
fn replacement_reprepares_profile_settings_and_refreshes_state_bands() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let old = log(1.0, true, false);
    workbench.log.set(Some(old.clone()));
    let directory = tempfile::tempdir().unwrap();
    let definition: crate::profile::Profile = serde_json::from_value(serde_json::json!({
        "version":2,"name":"Dynamic",
        "filters":[{"id":"scaled","metadata":{"name":"Scaled"},"inputs":["sensor/0/value"],"operation":{"kind":"affine","gain":{"setting":{"name":"Alpha"}}}}]
    })).unwrap();
    let prepared = crate::profile::prepare::prepare(
        definition.clone(),
        &old,
        directory.path(),
        600,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let fields = prepared.fields.clone();
    let key = ProfileKey::new(directory.path().join("dynamic.json"), 1);
    workbench
        .commit_instance(
            AppliedProfile::from_prepared(key.clone(), definition, prepared),
            fields,
            Vec::new(),
        )
        .unwrap();
    let field = workbench.derived.get_untracked()[0].clone();
    workbench.pane().lines.set(vec![Line {
        id: 700,
        name: "Scaled".into(),
        source: field.key(),
        data: field.data.unwrap(),
        revision: 0,
    }]);
    let mut states = crate::state_bands::StateBands::build(
        "@setting/1/1".into(),
        old.prepare("@setting/1/1").unwrap(),
        old.duration,
    )
    .unwrap();
    states.rules[0].name = "Ready".into();
    let before = states.input.clone();
    let mut geometry = Geometry::new(800, crate::geometry::GeometryKind::Band, kurbo::Point::ZERO);
    geometry.states = Some(Arc::new(states));
    workbench.geometry.items.set(vec![geometry]);
    workbench.pane().geometry.set(BTreeSet::from([800]));
    replace(workbench, log(3.0, false, true));
    assert_eq!(
        workbench.pane().lines.get_untracked()[0].data.samples()[0].y,
        90.0
    );
    let profile = workbench
        .profiles
        .instances
        .get_untracked()
        .remove(&key)
        .unwrap();
    assert_eq!(
        profile.fields[0].data.as_ref().unwrap().samples()[0].y,
        90.0
    );
    let geometry = workbench.geometry_item(800).unwrap();
    let states = geometry.states.unwrap();
    assert_eq!(states.source, "@setting/1/0");
    assert_eq!(states.rules[0].name, "Ready");
    assert_ne!(states.input.samples().as_ptr(), before.samples().as_ptr());
}

#[test]
fn failed_open_leaves_the_current_layout_and_plot_data_intact() {
    use std::time::{Duration, Instant};
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let old = log(1.0, true, false);
    workbench.log.set(Some(old.clone()));
    add_line(workbench.pane(), 100, "sensor/0/value", &old);
    let tab = workbench.add_plot(workbench.pane().id, false, true);
    let layout = workbench.layout().get_untracked();
    let directory = tempfile::tempdir().unwrap();
    workbench.open(directory.path().join("missing.ulg"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench.loading.get_untracked() {
        window.frame();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(Arc::ptr_eq(&workbench.log.get_untracked().unwrap(), &old));
    assert_eq!(workbench.active().get_untracked(), tab.id);
    assert_eq!(workbench.layout().get_untracked(), layout);
    assert_eq!(
        workbench.all_panes()[0].lines.get_untracked()[0]
            .data
            .samples()[0]
            .y,
        1.0
    );
}
