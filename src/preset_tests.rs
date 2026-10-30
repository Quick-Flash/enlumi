use crate::{
    preset_capture::{CaptureKind, capture},
    profile::{self, identity::ProfileKey},
    state::{Line, Workbench},
};
use plot::{PlotPoint, Ranges, time_series::SeriesData};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use zgui::prelude::*;

const SOURCE: &str = "esc_status/0/esc[5].esc_rpm";

#[test]
fn trajectory_recipe_preserves_axes_and_display_mode_through_capture_and_restore() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log()));
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let other = log()
        .catalogue
        .iter()
        .find(|signal| signal.id.ends_with("esc_rpm") && signal.id != SOURCE)
        .unwrap()
        .id
        .clone();
    add_line(workbench, &other, log().prepare(&other).unwrap());
    let pane = workbench.pane();
    pane.trajectory.set(Some(crate::trajectory::Spec {
        x: SOURCE.into(),
        y: SOURCE.into(),
        points: true,
        equal: false,
        extra: vec![crate::trajectory::Pair {
            x: SOURCE.into(),
            y: other.clone(),
            gps_scale: None,
            fix: String::new(),
        }],
        ..Default::default()
    }));
    pane.timeline_link.set(true);
    let styled = pane
        .lines
        .with_untracked(|lines| lines.iter().find(|line| line.source == other).unwrap().id);
    let mut style = pane.plot.style(&styled.to_string());
    style.color = [12, 45, 200, 255];
    style.width = 4.5;
    style.radius = 6.;
    pane.plot.styles.update(|styles| {
        styles.insert(styled.to_string(), style);
    });
    pane.drawing.update(|drawing| {
        drawing.insert(
            styled,
            plot::line_draw::LineDraw {
                markers: true,
                pattern: plot::line_draw::LinePattern::Dashed,
                ..Default::default()
            },
        );
    });
    let definition: profile::Profile = serde_json::from_value(
        capture(workbench, &CaptureKind::Plot(pane.id), "XY recipe")
            .unwrap()
            .document,
    )
    .unwrap();
    let recipe = definition.plot_presets[0].clone();
    assert_eq!(
        recipe.trajectory.as_ref().unwrap().x,
        recipe.lines[0].source
    );
    assert_eq!(
        recipe.trajectory.as_ref().unwrap().y,
        recipe.lines[0].source
    );
    let directory = tempfile::tempdir().unwrap();
    let key = prepare(workbench, definition, &directory.path().join("xy.json"));
    workbench.apply_plot_preset(&key, &recipe, None).unwrap();
    let restored = workbench.pane();
    settle(&window, || restored.lines.with_untracked(Vec::len) == 2);
    assert!(restored.timeline_link.get_untracked());
    let spec = restored.trajectory.get_untracked().unwrap();
    assert_eq!(spec.x, SOURCE);
    assert_eq!(spec.y, SOURCE);
    assert!(spec.points);
    assert!(!spec.equal);
    assert_eq!(spec.extra[0].x, SOURCE);
    assert_eq!(spec.extra[0].y, other);
    let restored_id = restored
        .lines
        .with_untracked(|lines| lines.iter().find(|line| line.source == other).unwrap().id);
    assert_eq!(restored.plot.style(&restored_id.to_string()), style);
    assert_eq!(
        restored.drawing.get_untracked()[&restored_id].pattern,
        plot::line_draw::LinePattern::Dashed
    );
    assert!(restored.drawing.get_untracked()[&restored_id].markers);
    workbench.duplicate_plot(restored);
    assert_eq!(workbench.pane().trajectory.get_untracked(), Some(spec));
    assert!(workbench.pane().timeline_link.get_untracked());
}
#[test]
fn link_defaults_enable_pointer_and_timeline_but_preserve_explicit_choices() {
    let defaults = crate::preset::Links::default();
    assert!(defaults.pointer && defaults.timeline);
    assert!(!defaults.x && !defaults.y);
    let legacy: crate::preset::Links =
        serde_json::from_str(r#"{"x":false,"y":false,"pointer":false}"#).unwrap();
    assert!(legacy.timeline);
    assert!(!legacy.pointer);
    let explicit: crate::preset::Links =
        serde_json::from_str(r#"{"x":false,"y":false,"pointer":false,"timeline":false}"#).unwrap();
    assert!(!explicit.timeline && !explicit.pointer);
}

fn log() -> Arc<crate::log::LogStore> {
    Arc::new(
        crate::log::LogStore::read(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    )
}
fn add_line(workbench: Workbench, source: &str, data: SeriesData) {
    let pane = workbench.pane();
    pane.lines.update(|lines| {
        lines.push(Line {
            id: workbench.id(),
            source: source.into(),
            name: source.into(),
            data,
            revision: 0,
        })
    });
}
fn settle(window: &zgui_testkit_view::Window, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.frame();
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "Preset worker timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn prepare(workbench: Workbench, definition: profile::Profile, path: &Path) -> ProfileKey {
    let log = log();
    let prepared = profile::prepare::prepare(
        definition.clone(),
        &log,
        path.parent().unwrap(),
        10000,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let key = ProfileKey::new(path.to_owned(), 1);
    let fields = prepared.fields.clone();
    workbench.log.set(Some(log));
    workbench
        .commit_instance(
            crate::profile_instance::AppliedProfile::from_prepared(
                key.clone(),
                definition,
                prepared,
            ),
            fields,
            Vec::new(),
        )
        .unwrap();
    key
}
#[test]
fn saving_captured_filters_reuses_raw_inputs_and_numeric_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("literal.json");
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let data = log().prepare(SOURCE).unwrap();
    workbench.derived.set(vec![crate::derived::DerivedField {
        id: 70,
        name: "Scaled".into(),
        sources: vec![SOURCE.into()],
        group: 70,
        filter: crate::derived::FilterSpec::Affine {
            gain: 2.,
            offset: 0.,
        },
        data: Some(data.clone()),
    }]);
    add_line(workbench, "@derived/70", data);
    std::fs::write(&path,serde_json::json!({"version":2,"name":"Literal","filters":[{"id":"original","metadata":{"name":"Scaled","unit":"","axis":""},"inputs":[SOURCE],"operation":{"kind":"affine","gain":2}}]}).to_string()).unwrap();
    for name in ["First", "Second"] {
        crate::preset_store::save(
            capture(workbench, &CaptureKind::Plot(workbench.pane().id), name).unwrap(),
            &path,
            false,
            None,
        )
        .unwrap();
    }
    let saved = profile::load(&path).unwrap();
    assert_eq!(saved.filters.len(), 1);
    assert_eq!(saved.plot_presets[0].lines[0].source, "$original");
    assert_eq!(saved.plot_presets[1].lines[0].source, "$original");
}

#[test]
fn view_save_overwrites_without_dialog_and_save_as_preserves_original() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Dashboard.json");
    workbench
        .profiles
        .directory
        .set(Some(directory.path().to_owned()));
    workbench.rename_view("Dashboard".into());
    workbench.save_preset(CaptureKind::View);
    assert!(workbench.authoring.replace.get_untracked());
    assert!(!workbench.authoring.open.get_untracked());
    settle(&window, || !workbench.authoring.busy.get_untracked());
    assert!(
        !workbench.authoring.open.get_untracked(),
        "{}",
        workbench.authoring.error.get_untracked()
    );
    workbench.add_plot(workbench.pane().id, true, false);
    workbench.save_preset_as(CaptureKind::View);
    assert!(workbench.authoring.open.get_untracked());
    assert_eq!(workbench.authoring.name.get_untracked(), "Dashboard copy");
    assert!(!workbench.authoring.replace.get_untracked());
    workbench.save_authoring();
    settle(&window, || !workbench.authoring.busy.get_untracked());
    assert!(
        !workbench.authoring.open.get_untracked(),
        "{}",
        workbench.authoring.error.get_untracked()
    );
    let profile = profile::load(&path).unwrap();
    assert_eq!(
        profile
            .view_presets
            .iter()
            .find(|view| view.name == "Dashboard")
            .unwrap()
            .plots
            .len(),
        1
    );
    assert_eq!(
        profile
            .view_presets
            .iter()
            .find(|view| view.name == "Dashboard copy")
            .unwrap()
            .plots
            .len(),
        2
    );
    workbench.add_plot(workbench.pane().id, true, false);
    workbench.save_preset(CaptureKind::View);
    assert!(
        !workbench.authoring.open.get_untracked(),
        "Save must not open review dialog"
    );
    assert_eq!(workbench.authoring.name.get_untracked(), "Dashboard copy");
    assert!(workbench.authoring.replace.get_untracked());
    settle(&window, || !workbench.authoring.busy.get_untracked());
    assert!(
        !workbench.authoring.open.get_untracked(),
        "{}",
        workbench.authoring.error.get_untracked()
    );
    let profile = profile::load(&path).unwrap();
    assert_eq!(
        profile
            .view_presets
            .iter()
            .find(|view| view.name == "Dashboard")
            .unwrap()
            .plots
            .len(),
        1
    );
    assert_eq!(
        profile
            .view_presets
            .iter()
            .find(|view| view.name == "Dashboard copy")
            .unwrap()
            .plots
            .len(),
        3
    );
}

#[test]
fn save_overwrites_unlinked_views_and_workspaces_but_save_as_checks_collisions() {
    for kind in [CaptureKind::View, CaptureKind::Workspace] {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let directory = tempfile::tempdir().unwrap();
        workbench
            .profiles
            .directory
            .set(Some(directory.path().to_owned()));
        workbench.rename_view("Dashboard".into());
        workbench.workspace().name.set("Dashboard".into());
        workbench.save_preset(kind.clone());
        settle(&window, || !workbench.authoring.busy.get_untracked());
        assert!(workbench.authoring.error.with_untracked(String::is_empty));
        workbench.view().preset.set(None);
        workbench.workspace().source.set(None);
        workbench.add_plot(workbench.pane().id, true, false);
        workbench.save_preset(kind.clone());
        assert!(!workbench.authoring.open.get_untracked());
        assert!(workbench.authoring.replace.get_untracked());
        settle(&window, || !workbench.authoring.busy.get_untracked());
        assert!(workbench.authoring.error.with_untracked(String::is_empty));
        let path = directory.path().join("Dashboard.json");
        let saved = std::fs::read(&path).unwrap();
        let profile = profile::load(&path).unwrap();
        let plots = match kind {
            CaptureKind::View => &profile.view_presets[0].plots,
            CaptureKind::Workspace => &profile.workspace_presets[0].views[0].plots,
            _ => unreachable!(),
        };
        assert_eq!(plots.len(), 2);
        workbench.add_plot(workbench.pane().id, true, false);
        workbench.save_preset_as(kind);
        assert!(workbench.authoring.open.get_untracked());
        assert!(!workbench.authoring.replace.get_untracked());
        workbench.authoring.name.set("Dashboard".into());
        workbench.save_authoring();
        settle(&window, || !workbench.authoring.busy.get_untracked());
        assert!(workbench.authoring.open.get_untracked());
        assert!(
            workbench
                .authoring
                .error
                .get_untracked()
                .contains("already exists")
        );
        assert_eq!(std::fs::read(path).unwrap(), saved);
    }
}

#[test]
fn saved_view_plots_load_with_existing_input_labels() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let bundle = capture(workbench, &CaptureKind::View, "Dashboard").unwrap();
    let mut definition: profile::Profile = serde_json::from_value(bundle.document).unwrap();
    definition.signals[0].metadata.name = "Original label".into();
    prepare(
        workbench,
        definition.clone(),
        Path::new("C:/original-labels.json"),
    );
    definition.signals[0].metadata.name = "Saved label".into();
    let key = prepare(workbench, definition, Path::new("C:/saved-view.json"));
    assert_eq!(workbench.field_name_untracked(SOURCE), "Original label");
    let name = crate::preset_lookup::plot_key("Dashboard", 0);
    assert!(
        workbench
            .profiles
            .instances
            .with_untracked(|instances| instances[&key].views.contains_key(&name))
    );
    let original = workbench.pane().id;
    workbench.use_profile_item(
        crate::profile::identity::Drag {
            profile: key,
            item: crate::profile::identity::Item::View(name),
        },
        crate::profile_state::Destination::Plot(None),
    );
    settle(&window, || {
        workbench.pane().id != original
            && workbench
                .pane()
                .lines
                .with_untracked(|lines| lines.iter().any(|line| line.source == SOURCE))
    });
    assert!(workbench.profiles.error.with_untracked(String::is_empty));
}

#[test]
fn view_capture_deduplicates_dependencies_and_excludes_runtime_state() {
    use crate::{
        derived::{DerivedField, FilterSpec},
        geometry::{Geometry, GeometryKind},
    };
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let data = log().prepare(SOURCE).unwrap();
    workbench.derived.set(vec![DerivedField {
        id: 70,
        name: "Scaled".into(),
        sources: vec![SOURCE.into()],
        group: 70,
        filter: FilterSpec::Affine {
            gain: 2.,
            offset: 0.,
        },
        data: Some(data.clone()),
    }]);
    add_line(workbench, "@derived/70", data.clone());
    let first = workbench.pane();
    let line = first.lines.get_untracked()[0].id;
    first.drawing.update(|draw| {
        draw.insert(
            line,
            plot::line_draw::LineDraw {
                connection: plot::line_draw::Connection::Post,
                ..Default::default()
            },
        );
    });
    first.plot.hidden.update(|hidden| {
        hidden.insert(line.to_string());
    });
    let mut geometry = Geometry::new(80, GeometryKind::Band, kurbo::Point::ZERO);
    geometry.coordinates = [1., 0., 2., 1.];
    workbench.geometry.items.set(vec![geometry]);
    first.geometry.update(|ids| {
        ids.insert(80);
    });
    let second = workbench.add_plot(first.id, false, true);
    second.geometry.update(|ids| {
        ids.insert(80);
    });
    add_line(workbench, SOURCE, data);
    workbench.analysis().start.set("99".into());
    let before = capture(workbench, &CaptureKind::View, "Portable").unwrap();
    first
        .plot
        .set_view(Ranges::new((0.3, 0.6), (-100., 100.)))
        .unwrap();
    workbench.analysis().start.set("200".into());
    let after = capture(workbench, &CaptureKind::View, "Portable").unwrap();
    assert_eq!(before.document, after.document);
    assert_eq!(before.document["signals"].as_array().unwrap().len(), 1);
    assert_eq!(before.document["filters"].as_array().unwrap().len(), 1);
    assert_eq!(before.document["geometry"].as_array().unwrap().len(), 1);
    let plots = &before.document["view_presets"][0]["plots"];
    assert_eq!(plots[0]["geometry"], plots[1]["geometry"]);
    assert_eq!(plots[0]["lines"][0]["connection"], "post");
    assert_eq!(plots[0]["lines"][0]["hidden"], true);
    let text = before.document.to_string();
    for forbidden in ["ranges", "viewport", "auto_fit", "@derived/70", "points"] {
        assert!(
            !text.contains(forbidden),
            "Saved runtime state: {forbidden}"
        );
    }
    let definition: profile::Profile = serde_json::from_value(before.document).unwrap();
    let prepared = profile::prepare::prepare(
        definition,
        &log(),
        Path::new("."),
        100,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(prepared.views.len(), 2);
    assert_eq!(prepared.fields.len(), 1);
    assert_eq!(prepared.geometry.len(), 1);
}
#[test]
fn preset_store_preserves_legacy_entries_requires_replace_and_detects_stale_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("profile.json");
    let legacy = serde_json::json!({"version":1,"name":"Existing","views":[{"name":"Legacy","fields":[SOURCE]}]});
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let bundle = capture(workbench, &CaptureKind::Plot(workbench.pane().id), "Motor").unwrap();
    crate::preset_store::save(bundle.clone(), &path, false, None).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["views"], legacy["views"]);
    assert_eq!(saved["name"], "Existing");
    let mut changed = bundle;
    changed.document["plot_presets"][0]["lines"][0]["width"] = serde_json::json!(3.);
    let previous = std::fs::read(&path).unwrap();
    assert!(
        crate::preset_store::save(changed.clone(), &path, false, None)
            .unwrap_err()
            .contains("Replace")
    );
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    crate::preset_store::save(changed, &path, true, None).unwrap();
    assert!(
        crate::preset_store::write_document(saved, &path, Some(previous), BTreeMap::new(), None)
            .unwrap_err()
            .contains("changed while saving")
    );
    assert_eq!(
        profile::load(&path).unwrap().plot_presets[0].lines[0].width,
        3.
    );
}
#[test]
fn save_as_copies_rust_dependencies_and_rejects_missing_sources() {
    let directory = tempfile::tempdir().unwrap();
    let origin = directory.path().join("original.json");
    let source = "pub const OUTPUTS: usize = 1;\npub fn filter() {}";
    std::fs::write(directory.path().join("filter.rs"), source).unwrap();
    let document = serde_json::json!({"version":2,"name":"Rust","filters":[{"id":"f","metadata":{"name":"Rust"},"inputs":[SOURCE],"operation":{"kind":"rust","path":"filter.rs","outputs":1,"output":0}}]});
    let destination = directory.path().join("portable/copy.json");
    crate::preset_store::write_document(
        document.clone(),
        &destination,
        None,
        BTreeMap::new(),
        Some(&origin),
    )
    .unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
    let reference = saved["filters"][0]["operation"]["path"].as_str().unwrap();
    assert_eq!(
        std::fs::read_to_string(destination.parent().unwrap().join(reference)).unwrap(),
        source
    );
    std::fs::remove_file(directory.path().join("filter.rs")).unwrap();
    let missing = directory.path().join("missing.json");
    assert!(
        crate::preset_store::write_document(
            document,
            &missing,
            None,
            BTreeMap::new(),
            Some(&origin)
        )
        .unwrap_err()
        .contains("Missing Rust dependency")
    );
    assert!(!missing.exists());
}

#[test]
fn multiple_presets_reuse_shared_fields_and_remap_recipes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shared.json");
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    for name in ["First", "Second"] {
        crate::preset_store::save(
            capture(workbench, &CaptureKind::Plot(workbench.pane().id), name).unwrap(),
            &path,
            false,
            None,
        )
        .unwrap();
    }
    let definition = profile::load(&path).unwrap();
    assert_eq!(definition.signals.len(), 1);
    assert_eq!(definition.plot_presets.len(), 2);
    assert_eq!(
        definition.plot_presets[0].lines[0].source,
        definition.plot_presets[1].lines[0].source
    );
    let prepared = profile::prepare::prepare(
        definition,
        &log(),
        directory.path(),
        100,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(prepared.diagnostics.is_empty());
    assert_eq!(prepared.views.len(), 2);
}

#[test]
fn view_restore_remaps_layout_routes_background_work_and_append_keeps_bounds() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let original = workbench.bind_view();
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let first = workbench.pane();
    workbench.add_plot(first.id, true, false);
    let second = workbench.pane();
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    workbench.add_plot(second.id, false, true);
    let bundle = capture(workbench, &CaptureKind::View, "Dashboard").unwrap();
    let definition: profile::Profile = serde_json::from_value(bundle.document).unwrap();
    let key = prepare(workbench, definition, Path::new("C:/view-preset.json"));
    let bounds = Ranges::new((0.1, 0.2), (-3., 3.));
    first.plot.set_view(bounds).unwrap();
    workbench.use_saved_preset(
        key.clone(),
        "Dashboard".into(),
        crate::preset_ui::ApplyAction::NewView,
    );
    let restored = workbench.bind_view();
    assert_ne!(restored.view().id, original.view().id);
    assert_eq!(restored.panes().with_untracked(Vec::len), 3);
    let unrelated = workbench.new_view();
    settle(&window, || {
        workbench
            .authoring
            .restoring
            .with_value(|pending| pending.is_empty())
    });
    assert_eq!(workbench.views.active.get_untracked(), unrelated);
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 0);
    assert_eq!(
        restored.panes().with_untracked(|panes| panes
            .iter()
            .filter(|pane| !pane.lines.with_untracked(Vec::is_empty))
            .count()),
        2
    );
    let source = restored.view().preset.get_untracked().unwrap();
    assert!(!restored.preset_modified(&CaptureKind::View, &source));
    assert_eq!(first.plot.ranges.get_untracked(), bounds);
    original.use_saved_preset(
        key,
        "Dashboard".into(),
        crate::preset_ui::ApplyAction::AppendView,
    );
    settle(&window, || {
        original.panes().with_untracked(|panes| {
            panes
                .iter()
                .map(|pane| pane.lines.with_untracked(Vec::len))
                .sum::<usize>()
        }) == 4
    });
    assert_eq!(original.panes().with_untracked(Vec::len), 6);
    let mut dividers = Vec::new();
    original.layout().get_untracked().geometry(
        plot::workspace::layout::Rect {
            width: 1200.,
            height: 800.,
            ..Default::default()
        },
        &mut Vec::new(),
        &mut dividers,
    );
    assert_eq!(
        dividers
            .iter()
            .map(|divider| divider.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        dividers.len()
    );
    assert_eq!(first.plot.ranges.get_untracked(), bounds);
    assert_eq!(workbench.views.active.get_untracked(), unrelated);
}

#[test]
fn malformed_layout_and_saved_bounds_are_rejected() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let mut document = capture(workbench, &CaptureKind::View, "View")
        .unwrap()
        .document;
    document["view_presets"][0]["layout"] =
        serde_json::json!({"kind":"tabs","plots":[0,0],"active":0});
    let definition: profile::Profile = serde_json::from_value(document.clone()).unwrap();
    assert!(crate::preset::validate(&definition).is_err());
    document["view_presets"][0]["plots"][0]["bounds"] = serde_json::json!([0, 1, 0, 1]);
    assert!(serde_json::from_value::<profile::Profile>(document).is_err());
}

#[test]
fn saving_then_loading_in_same_log_borrows_live_filters_and_geometry() {
    use crate::{
        derived::{DerivedField, FilterSpec},
        geometry::{Geometry, GeometryKind},
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("live.json");
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log()));
    let data = log().prepare(SOURCE).unwrap();
    let id = workbench.id();
    let source = crate::derived::key(id);
    workbench.derived.set(vec![DerivedField {
        id,
        name: "Filtered RPM".into(),
        sources: vec![SOURCE.into()],
        group: id,
        filter: FilterSpec::MovingAverage(1),
        data: Some(data.clone()),
    }]);
    add_line(workbench, &source, data);
    let geometry_id = workbench.id();
    let mut item = Geometry::new(geometry_id, GeometryKind::Band, kurbo::Point::ZERO);
    item.coordinates = [0., 0., 1., 1.];
    workbench.geometry.items.set(vec![item]);
    workbench.pane().geometry.update(|ids| {
        ids.insert(geometry_id);
    });
    crate::preset_store::save(
        capture(workbench, &CaptureKind::Plot(workbench.pane().id), "Live").unwrap(),
        &path,
        false,
        None,
    )
    .unwrap();
    workbench.apply_profile(path.clone());
    settle(&window, || !workbench.profiles.busy.get_untracked());
    assert!(
        workbench.profiles.error.get_untracked().is_empty(),
        "{}",
        workbench.profiles.error.get_untracked()
    );
    let key = ProfileKey::new(path.canonicalize().unwrap(), 1);
    let instance = workbench.profiles.instances.get_untracked()[&key].clone();
    assert_eq!(workbench.derived.with_untracked(Vec::len), 1);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 1);
    assert!(instance.fields.is_empty());
    assert!(instance.geometry.is_empty());
    assert_eq!(
        instance.resolve(&instance.definition.plot_presets[0].lines[0].source),
        Some(source)
    );
    workbench
        .apply_plot_preset(&key, &instance.definition.plot_presets[0], None)
        .unwrap();
    let restored = workbench.pane();
    settle(&window, || restored.preset.get_untracked().is_some());
    assert!(
        restored
            .geometry
            .with_untracked(|ids| ids.contains(&geometry_id))
    );
    assert_eq!(restored.lines.with_untracked(Vec::len), 1);
    workbench.remove_profile(key);
    assert_eq!(workbench.derived.with_untracked(Vec::len), 1);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 1);
}

#[test]
fn profile_label_save_clears_local_override_and_preserves_live_plot_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("labels.json");
    let definition:profile::Profile=serde_json::from_value(serde_json::json!({"version":1,"name":"Labels","signals":[{"id":"rpm","source":SOURCE,"metadata":{"name":"RPM"}}]})).unwrap();
    std::fs::write(&path, serde_json::to_vec(&definition).unwrap()).unwrap();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let key = prepare(workbench, definition, &path.canonicalize().unwrap());
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let ranges = Ranges::new((0.1, 0.2), (-1., 1.));
    workbench.pane().plot.set_view(ranges).unwrap();
    workbench.set_field_metadata(
        SOURCE.into(),
        profile::Metadata {
            name: "Local".into(),
            ..Default::default()
        },
    );
    workbench.save_field_to_profile(
        key.path,
        SOURCE.into(),
        profile::Metadata {
            name: "Saved".into(),
            unit: "rpm".into(),
            axis: String::new(),
        },
    );
    settle(&window, || {
        workbench.profiles.metadata.with_untracked(|metadata| {
            metadata
                .get(SOURCE)
                .is_some_and(|meta| meta.name == "Saved")
        })
    });
    assert!(
        !workbench
            .profiles
            .overrides
            .with_untracked(|overrides| overrides.contains_key(SOURCE))
    );
    assert_eq!(
        profile::load(&path).unwrap().signals[0].metadata.name,
        "Saved"
    );
    assert_eq!(workbench.pane().plot.ranges.get_untracked(), ranges);
}

#[test]
fn saving_preview_label_edits_establishes_clean_source_after_profile_preparation() {
    let directory = tempfile::tempdir().unwrap();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log()));
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let pane = workbench.pane();
    workbench.begin_save(CaptureKind::Plot(pane.id));
    workbench
        .authoring
        .path
        .set(directory.path().join("preview.json").display().to_string());
    workbench.authoring.session.update(|session| {
        session.as_mut().unwrap().bundle.document["signals"][0]["metadata"]["name"] =
            serde_json::json!("Motor speed")
    });
    workbench.save_authoring();
    settle(&window, || {
        !workbench.authoring.busy.get_untracked()
            && !workbench.profiles.busy.get_untracked()
            && workbench.profiles.metadata.with_untracked(|metadata| {
                metadata
                    .get(SOURCE)
                    .is_some_and(|meta| meta.name == "Motor speed")
            })
    });
    let source = pane.preset.get_untracked().unwrap();
    assert!(!workbench.preset_modified(&CaptureKind::Plot(pane.id), &source));
}

#[test]
fn legacy_browser_keeps_usable_fields_when_inputs_are_missing() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let definition:profile::Profile=serde_json::from_value(serde_json::json!({"version":1,"name":"Legacy","views":[{"name":"Motor","fields":[SOURCE,"missing/0/field"]}]})).unwrap();
    let key = prepare(workbench, definition, Path::new("C:/legacy-preset.json"));
    let instance = workbench.profiles.instances.get_untracked()[&key].clone();
    assert!(!instance.diagnostics.is_empty());
    let choices = crate::preset_apply::available_plots(&instance);
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].lines.len(), 1);
    workbench.use_saved_preset(key, "Motor".into(), crate::preset_ui::ApplyAction::Plot);
    let pane = workbench.pane();
    settle(&window, || pane.preset.get_untracked().is_some());
    assert_eq!(pane.lines.with_untracked(Vec::len), 1);
    assert_eq!(instance.definition.version, 1);
    assert!(instance.definition.plot_presets.is_empty());
}
#[test]
fn plot_restore_and_field_subset_preserve_destination_bounds() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    add_line(workbench, SOURCE, log().prepare(SOURCE).unwrap());
    let pane = workbench.pane();
    pane.background.set(Some([35, 45, 55]));
    let bundle = capture(workbench, &CaptureKind::Plot(pane.id), "Motor").unwrap();
    let definition: profile::Profile = serde_json::from_value(bundle.document).unwrap();
    let recipe = definition.plot_presets[0].clone();
    assert_eq!(recipe.background, Some([35, 45, 55]));
    let key = prepare(workbench, definition, Path::new("C:/preset-test.json"));
    let ranges = Ranges::new((0.1, 0.2), (-4., 4.));
    pane.plot.set_view(ranges).unwrap();
    workbench
        .apply_plot_preset(&key, &recipe, Some(pane.id))
        .unwrap();
    // Flush worker by adding a distinct new plot too.
    workbench.apply_plot_preset(&key, &recipe, None).unwrap();
    let restored = workbench.pane();
    settle(&window, || restored.preset.get_untracked().is_some());
    assert_eq!(pane.plot.ranges.get_untracked(), ranges);
    assert_eq!(pane.lines.with_untracked(Vec::len), 1);
    assert_eq!(restored.lines.with_untracked(Vec::len), 1);
    assert_eq!(restored.background.get_untracked(), Some([35, 45, 55]));
    assert!(restored.plot.ranges.get_untracked().x.max > ranges.x.max);
    let source = restored.preset.get_untracked().unwrap();
    assert!(!workbench.preset_modified(&CaptureKind::Plot(restored.id), &source));
    restored.plot.set_view(ranges).unwrap();
    assert!(!workbench.preset_modified(&CaptureKind::Plot(restored.id), &source));
    restored.plot.hidden.update(|hidden| {
        hidden.insert(restored.lines.get_untracked()[0].id.to_string());
    });
    assert!(workbench.preset_modified(&CaptureKind::Plot(restored.id), &source));
    let mut fft = recipe;
    fft.analysis = Some(crate::preset::Analysis {
        mode: "fft".into(),
        window: "hann".into(),
        size: 1024,
        overlap: 50,
        heatmap_bins: 32,
        remove_dc: true,
        condition: None,
    });
    assert!(
        workbench
            .apply_plot_preset(&key, &fft, Some(pane.id))
            .is_err()
    );
    assert_eq!(pane.plot.ranges.get_untracked(), ranges);
}
#[test]
fn analysis_capture_omits_intervals_and_restore_uses_current_selection() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let data = SeriesData::new(
        (0..4096)
            .map(|i| PlotPoint {
                x: i as f64 / 1024.,
                y: (i as f64 * 0.1).sin() + 5.,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let options = crate::spectral::Options {
        mode: crate::spectral::Mode::Fft,
        window: crate::spectral::Window::Hann,
        size: 1024,
        overlap: 50,
        heatmap_bins: 32,
        remove_dc: true,
        start: Some(1.),
        end: Some(2.),
    };
    workbench.set_analysis_panel(
        workbench.pane(),
        crate::analysis_state::AnalysisResult {
            mode: options.mode,
            options,
            title: "FFT".into(),
            units: "rpm".into(),
            summary: String::new(),
            condition: String::new(),
            series: plot::time_series::TimeSeriesSet::new(vec![
                plot::time_series::TimeSeries::new(SOURCE, "RPM", data),
            ])
            .unwrap(),
            heatmap: None,
        },
    );
    let bundle = capture(
        workbench,
        &CaptureKind::Plot(workbench.pane().id),
        "Spectrum",
    )
    .unwrap();
    assert!(
        bundle.document["plot_presets"][0]["analysis"]
            .get("start")
            .is_none()
    );
    let definition: profile::Profile = serde_json::from_value(bundle.document).unwrap();
    let recipe = definition.plot_presets[0].clone();
    let key = prepare(workbench, definition, Path::new("C:/spectrum-test.json"));
    workbench.analysis().start.set(String::new());
    workbench.analysis().end.set(String::new());
    workbench.apply_plot_preset(&key, &recipe, None).unwrap();
    let restored = workbench.pane();
    settle(&window, || {
        restored.analysis.with_untracked(Option::is_some)
    });
    let result = restored.analysis.get_untracked().unwrap();
    assert_eq!(result.options.start, None);
    assert_eq!(result.options.end, None);
    assert!(result.options.remove_dc);
    assert!(workbench.analysis().start.get_untracked().is_empty());
    assert!(restored.lines.with_untracked(|lines| {
        lines
            .iter()
            .all(|line| line.source.starts_with("@spectrum/"))
    }));
}

#[test]
fn windowed_preset_restore_keeps_conditioning_identity_and_axis_description() {
    use crate::{
        derived::{DerivedField, FilterSpec},
        spectral::{Mode, Options, Window},
    };
    for mode in [Mode::Heatmap, Mode::Spectrogram] {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let signal = SeriesData::new(
            (0..512)
                .map(|i| PlotPoint {
                    x: i as f64 / 1000.,
                    y: (i as f64 * 0.2).sin(),
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let condition = SeriesData::new(
            (0..512)
                .map(|i| PlotPoint {
                    x: i as f64 / 1000.,
                    y: i as f64,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        workbench.derived.set(vec![
            DerivedField {
                id: 70,
                name: "Signal".into(),
                sources: vec![SOURCE.into()],
                group: 70,
                filter: FilterSpec::MovingAverage(1),
                data: Some(signal.clone()),
            },
            DerivedField {
                id: 71,
                name: "Condition".into(),
                sources: vec![SOURCE.into()],
                group: 71,
                filter: FilterSpec::MovingAverage(1),
                data: Some(condition.clone()),
            },
        ]);
        let options = Options {
            mode,
            window: Window::Hann,
            size: 128,
            overlap: 50,
            heatmap_bins: 8,
            remove_dc: true,
            start: None,
            end: None,
        };
        let spectrum =
            crate::spectral::compute(&signal, Some(&condition), &options, &AtomicBool::new(false))
                .unwrap();
        workbench.set_analysis_panel(
            workbench.pane(),
            crate::analysis_state::AnalysisResult {
                mode,
                options,
                title: "Map".into(),
                units: String::new(),
                summary: String::new(),
                condition: if mode == Mode::Heatmap {
                    "@derived/71".into()
                } else {
                    String::new()
                },
                series: plot::time_series::TimeSeriesSet::new(vec![
                    plot::time_series::TimeSeries::new("@derived/70", "Signal", signal.clone()),
                ])
                .unwrap(),
                heatmap: spectrum.heatmap,
            },
        );
        let definition: profile::Profile = serde_json::from_value(
            capture(workbench, &CaptureKind::Plot(workbench.pane().id), "Map")
                .unwrap()
                .document,
        )
        .unwrap();
        let recipe = definition.plot_presets[0].clone();
        let key = prepare(workbench, definition, Path::new("C:/map-preset.json"));
        workbench.derived.update(|fields| {
            for field in fields {
                field.data = Some(if field.name == "Condition" {
                    condition.clone()
                } else {
                    signal.clone()
                });
            }
        });
        workbench.apply_plot_preset(&key, &recipe, None).unwrap();
        let restored = workbench.pane();
        settle(&window, || {
            restored.analysis.with_untracked(Option::is_some)
        });
        let result = restored.analysis.get_untracked().unwrap();
        assert!(result.mode == mode);
        assert!(result.heatmap.is_some());
        let map = result.heatmap.as_ref().unwrap();
        restored
            .plot
            .set_view(Ranges::new((1., 2.), (3., 4.)))
            .unwrap();
        restored.fit_axes(plot::interaction::Axes::BOTH);
        assert_eq!(
            restored.plot.ranges.get_untracked(),
            Ranges::new((map.x_low, map.x_high), (map.y_low, map.y_high))
        );
        assert!(result.units.contains("Y:"));
        if mode == Mode::Heatmap {
            assert!(result.units.contains("Condition"));
            assert_eq!(
                result.condition,
                workbench.profiles.instances.get_untracked()[&key]
                    .resolve(
                        recipe
                            .analysis
                            .as_ref()
                            .unwrap()
                            .condition
                            .as_ref()
                            .unwrap()
                    )
                    .unwrap()
            );
        } else {
            assert!(result.units.contains("Frequency (Hz)"));
        }
    }
}
