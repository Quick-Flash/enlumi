use crate::{
    preset_capture::{self, CaptureKind},
    state::Workbench,
    workspace_state::SavedWorkspace,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use zgui::prelude::*;
use zgui_testkit_view::Window;

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    ))
}

#[test]
fn workspace_geometry_conflicts_and_invalid_resources_do_not_block_load_or_reload() {
    use crate::{
        geometry::{Geometry, GeometryKind},
        preset_ui::ApplyAction,
        profile::identity::ProfileKey,
    };
    use serde_json::json;
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(Arc::new(
        crate::log::LogStore::read(&fixture()).unwrap(),
    )));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("resilient.json");
    let line = json!({"source":"esc_status/0/esc[5].esc_rpm","color":[255,0,0,255],"width":1.5,"radius":2,"marker":"circle","connection":"linear","pattern":"solid","markers":false,"hidden":false});
    let mut recipe = json!({"version":2,"name":"Resilient","geometry":[
        {"id":"a","name":"Cutoff","shape":{"kind":"line","coordinates":[0,1,1,1]}},
        {"id":"b","name":"Cutoff","shape":{"kind":"line","coordinates":[0,1,1,1]}},
        {"id":"c","name":"Cutoff","shape":{"kind":"line","coordinates":[0,2,1,2]}},
        {"id":"unnamed","name":"","shape":{"kind":"line","coordinates":[0,3,1,3]}},
        {"id":"invalid","name":"Broken","shape":{"kind":"line","coordinates":[0,0,0,0]}}
    ],"workspace_presets":[{"name":"Resilient","views":[{"name":"View","plots":[
        {"name":"First","lines":[line.clone()],"geometry":["a"],"analysis":null},
        {"name":"Second","lines":[line],"geometry":["b","c","unnamed","invalid"],"analysis":null}
    ],"layout":{"kind":"split","stacked":true,"ratio":0.5,"first":{"kind":"plot","plot":0},"second":{"kind":"plot","plot":1}}}]}]});
    std::fs::write(&path, recipe.to_string()).unwrap();
    let key = ProfileKey::new(path.canonicalize().unwrap(), 1);
    let mut manual = Geometry::new(workbench.id(), GeometryKind::Line, kurbo::Point::ZERO);
    manual.name = "Cutoff".into();
    manual.coordinates = [0., 99., 1., 99.];
    let manual_id = manual.id;
    workbench.geometry.items.update(|items| items.push(manual));
    workbench.use_saved_preset(key.clone(), "Resilient".into(), ApplyAction::Workspace);
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
            && workbench.workspace().name.get_untracked() == "Resilient"
            && workbench.panes().with_untracked(|panes| {
                panes.len() == 2 && panes.iter().all(|p| p.lines.with_untracked(Vec::len) == 1)
            })
    });
    assert!(workbench.profiles.error.with_untracked(String::is_empty));
    let ids = workbench
        .profiles
        .instances
        .with_untracked(|instances| instances[&key].ids.geometry.clone());
    assert_eq!(ids["a"], ids["b"]);
    assert_ne!(ids["a"], ids["c"]);
    assert!(!ids.contains_key("unnamed") && !ids.contains_key("invalid"));
    assert!(workbench.profiles.warnings.with_untracked(|warnings| {
        warnings
            .iter()
            .any(|w| w.contains("unnamed") && w.contains("needs a name"))
    }));
    assert!(workbench.profiles.warnings.with_untracked(|warnings| {
        warnings
            .iter()
            .any(|w| w.contains("Invalid geometry Broken"))
    }));
    let panes = workbench.panes().get_untracked();
    assert_eq!(panes[1].geometry.with_untracked(|ids| ids.len()), 2);
    recipe["geometry"][1]["shape"]["coordinates"] = json!([0, 3, 1, 3]);
    std::fs::write(&path, recipe.to_string()).unwrap();
    workbench.reload_profile(key.clone());
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
    });
    assert!(workbench.profiles.error.with_untracked(String::is_empty));
    let split = workbench
        .profiles
        .instances
        .with_untracked(|instances| instances[&key].ids.geometry.clone());
    assert_eq!(split["a"], ids["a"]);
    assert_ne!(split["a"], split["b"]);
    assert_eq!(
        panes[1].geometry.get_untracked(),
        std::collections::BTreeSet::from([split["b"], split["c"]])
    );
    recipe["geometry"][0]["shape"]["coordinates"] = json!([0, 0, 0, 0]);
    recipe["geometry"][3]["name"] = json!("Repaired");
    std::fs::write(&path, recipe.to_string()).unwrap();
    workbench.reload_profile(key.clone());
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
    });
    assert!(workbench.profiles.error.with_untracked(String::is_empty));
    assert!(panes[0].geometry.with_untracked(|ids| ids.is_empty()));
    assert_eq!(panes[0].lines.with_untracked(Vec::len), 1);
    let repaired = workbench
        .profiles
        .instances
        .with_untracked(|instances| instances[&key].ids.geometry["unnamed"]);
    assert!(
        panes[1]
            .geometry
            .with_untracked(|ids| ids.contains(&repaired))
    );
    assert!(workbench.geometry_item(manual_id).is_some());
    assert!(workbench.status.get_untracked().contains("warnings"));
}

fn settle(window: &Window, workbench: Workbench, ready: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        window.frame();
        if ready() && workbench.authoring.workspace_load.get_value().is_none() {
            return;
        }
        assert!(
            Instant::now() < end,
            "Workspace worker timed out: status={}, error={}, loading={}, busy={}, views={:?}",
            workbench.status.get_untracked(),
            workbench.profiles.error.get_untracked(),
            workbench.loading.get_untracked(),
            workbench.profiles.busy.get_untracked(),
            workbench.views.items.with_untracked(|views| views
                .iter()
                .map(|view| (
                    view.name.get_untracked(),
                    view.panes.with_untracked(|panes| panes
                        .iter()
                        .map(|pane| pane.lines.with_untracked(Vec::len))
                        .collect::<Vec<_>>())
                ))
                .collect::<Vec<_>>())
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn five_views(window: &Window, workbench: Workbench) {
    let log = Arc::new(crate::log::LogStore::read(&fixture()).unwrap());
    let source = log
        .catalogue
        .iter()
        .find(|field| field.count > 2)
        .unwrap()
        .id
        .clone();
    workbench.log.set(Some(log));
    for index in 0..5 {
        if index > 0 {
            workbench.new_view();
        }
        workbench.rename_view(format!("View {}", index + 1));
        workbench.add(source.clone());
    }
    workbench.switch_view(workbench.views.items.with_untracked(|views| views[2].id));
    settle(window, workbench, || {
        workbench.views.items.with_untracked(|views| {
            views.iter().all(|view| {
                view.panes.with_untracked(|panes| {
                    panes
                        .iter()
                        .all(|pane| pane.lines.with_untracked(|lines| lines.len() == 1))
                })
            })
        })
    });
}

#[test]
fn workspaces_keep_independent_view_lists_bounds_and_background_destinations() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    five_views(&window, workbench);
    let original = workbench.workspaces.active.get_untracked();
    let views = workbench.views.items.get_untracked();
    let first = workbench.for_view(views[0].id).unwrap();
    let pane = first.pane();
    let ranges = plot::Ranges::new((0.1, 0.5), (-3., 8.));
    pane.plot.set_view(ranges).unwrap();
    let selected = workbench.views.active.get_untracked();
    let other = workbench.new_workspace("Other".into());
    assert_eq!(workbench.views.items.with_untracked(Vec::len), 1);
    assert!(first.view_exists());
    assert!(workbench.find(pane.id).is_some());
    let captured = preset_capture::capture(first, &CaptureKind::Workspace, "Tools").unwrap();
    assert_eq!(
        captured.document["workspace_presets"][0]["views"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    workbench.switch_workspace(original);
    assert_eq!(workbench.views.items.with_untracked(Vec::len), 5);
    assert_eq!(workbench.views.active.get_untracked(), selected);
    assert_eq!(pane.plot.ranges.get_untracked(), ranges);
    workbench.close_workspace(other);
    assert_eq!(workbench.workspaces.items.with_untracked(Vec::len), 1);
}

#[test]
fn saving_workspace_keeps_deleted_filters_deleted_and_preserves_other_presets() {
    for borrowed in [false, true] {
        for shared in [false, true] {
            let window = Window::open();
            let initial = window.scope.with(Workbench::new);
            let log = Arc::new(crate::log::LogStore::read(&fixture()).unwrap());
            let raw = log
                .catalogue
                .iter()
                .find(|field| field.count > 2)
                .unwrap()
                .id
                .clone();
            let id = initial.id();
            initial.derived.set(vec![crate::derived::DerivedField {
                id,
                name: "Filtered".into(),
                sources: vec![raw.clone()],
                group: id,
                filter: crate::derived::FilterSpec::MovingAverage(1),
                data: Some(log.prepare(&raw).unwrap()),
            }]);
            initial.log.set(Some(log.clone()));
            initial.add(crate::derived::key(id));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("tools.json");
            initial.workspace().name.set("Tools".into());
            initial.begin_save(CaptureKind::Workspace);
            initial.authoring.path.set(path.display().to_string());
            initial.save_authoring();
            settle(&window, initial, || {
                !initial.authoring.busy.get_untracked() && !initial.profiles.busy.get_untracked()
            });
            assert!(initial.authoring.error.get_untracked().is_empty());
            if shared {
                let mut document: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                let mut plot = document["workspace_presets"][0]["views"][0]["plots"][0].clone();
                plot["name"] = serde_json::json!("Other preset");
                document["plot_presets"] = serde_json::json!([plot]);
                std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
            }
            let workbench = if borrowed {
                initial
            } else {
                let loaded = window.scope.with(Workbench::new);
                loaded.log.set(Some(log));
                loaded.open_saved_workspace(SavedWorkspace {
                    path: path.canonicalize().unwrap(),
                    name: "Tools".into(),
                    views: 1,
                    definition: Arc::new(crate::profile::load(&path).unwrap()),
                });
                settle(&window, loaded, || {
                    !loaded.profiles.busy.get_untracked()
                        && loaded.pane().lines.with_untracked(|lines| lines.len() == 1)
                });
                loaded
            };
            let field = workbench.derived.with_untracked(|fields| fields[0].id);
            workbench.delete_filter(field).unwrap();
            settle(&window, workbench, || {
                !workbench.derived_busy.get_untracked()
            });
            workbench.add(raw.clone());
            settle(&window, workbench, || {
                workbench
                    .pane()
                    .lines
                    .with_untracked(|lines| lines.len() == 1)
            });
            for _ in 0..2 {
                workbench.save_preset(CaptureKind::Workspace);
                settle(&window, workbench, || {
                    !workbench.authoring.busy.get_untracked()
                        && !workbench.profiles.busy.get_untracked()
                        && !workbench.derived_busy.get_untracked()
                });
                assert!(
                    workbench.authoring.error.get_untracked().is_empty(),
                    "{}",
                    workbench.authoring.error.get_untracked()
                );
                assert!(
                    workbench.profiles.error.get_untracked().is_empty(),
                    "{}",
                    workbench.profiles.error.get_untracked()
                );
                assert!(
                    workbench.derived.with_untracked(Vec::is_empty),
                    "Save resurrected filter: borrowed={borrowed}, shared={shared}"
                );
                assert_eq!(workbench.pane().lines.get_untracked()[0].source, raw);
                let saved = crate::profile::load(&path).unwrap();
                assert_eq!(saved.filters.len(), usize::from(shared));
                assert_eq!(saved.plot_presets.len(), usize::from(shared));
            }
        }
    }
}

#[test]
fn duplicate_workspace_copies_all_views_without_linking_saved_source() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    five_views(&window, workbench);
    let original = workbench.workspaces.active.get_untracked();
    workbench.workspace().name.set("Tools".into());
    let before = workbench.views.items.get_untracked();
    let bounds = plot::Ranges::new((0.1, 0.5), (-3., 8.));
    let source = workbench.pane();
    source.plot.set_view(bounds).unwrap();
    workbench
        .workspace()
        .source
        .set(Some(crate::preset::PresetSource {
            path: "tools.json".into(),
            name: "Tools".into(),
            baseline: String::new(),
        }));
    let copy = workbench.duplicate_workspace();
    assert_ne!(original, copy);
    assert_eq!(workbench.workspace().name.get_untracked(), "Tools copy");
    assert!(workbench.workspace().source.with_untracked(Option::is_none));
    let after = workbench.views.items.get_untracked();
    assert_eq!(after.len(), 5);
    assert_eq!(workbench.view().name.get_untracked(), "View 3");
    assert_eq!(workbench.pane().plot.ranges.get_untracked(), bounds);
    for (old, new) in before.iter().zip(&after) {
        assert_ne!(old.id, new.id);
        assert_eq!(old.name.get_untracked(), new.name.get_untracked());
        let old = workbench.for_view(old.id).unwrap().pane();
        let new = workbench.for_view(new.id).unwrap().pane();
        assert_ne!(old.id, new.id);
        assert_ne!(
            old.lines.get_untracked()[0].id,
            new.lines.get_untracked()[0].id
        );
        assert_eq!(
            old.lines.get_untracked()[0].source,
            new.lines.get_untracked()[0].source
        );
    }
    workbench.rename_view("Comparison".into());
    workbench.switch_workspace(original);
    assert_eq!(workbench.view().name.get_untracked(), "View 3");
    assert_eq!(source.plot.ranges.get_untracked(), bounds);
}

#[test]
fn workspace_streams_into_selected_layout_without_committing_preview_or_resetting_zoom() {
    use crate::{
        preset_ui::ApplyAction,
        profile::{identity::ProfileKey, progress::Progress},
    };
    use std::collections::BTreeMap;
    let window = Window::open();
    let source = window.scope.with(Workbench::new);
    five_views(&window, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stream.json");
    let bundle = preset_capture::capture(source, &CaptureKind::Workspace, "Streamed").unwrap();
    crate::preset_store::save(bundle, &path, false, None).unwrap();
    let path = path.canonicalize().unwrap();
    let definition = Arc::new(crate::profile::load(&path).unwrap());
    let workbench = window.scope.with(Workbench::new);
    let log = source.log.get_untracked().unwrap();
    let symbols = definition
        .signals
        .iter()
        .map(|signal| {
            (
                signal.id.clone(),
                crate::profile::resolve::source(&log, &signal.source, &BTreeMap::new()).unwrap(),
            )
        })
        .collect();
    workbench.log.set(Some(log.clone()));
    workbench.open_saved_workspace(SavedWorkspace {
        path: path.clone(),
        name: "Streamed".into(),
        views: 5,
        definition,
    });
    assert_eq!(workbench.workspace().name.get_untracked(), "Streamed");
    assert_eq!(workbench.views.items.with_untracked(Vec::len), 5);
    assert!(
        workbench
            .workspace()
            .loading
            .with_untracked(Option::is_some)
    );
    let panes = workbench.views.items.with_untracked(|views| {
        views
            .iter()
            .flat_map(|view| view.panes.get_untracked())
            .collect::<Vec<_>>()
    });
    assert!(
        panes
            .iter()
            .all(|pane| pane.lines.with_untracked(Vec::is_empty))
    );
    let key = ProfileKey::new(path, 1);
    let raw = source.pane().lines.get_untracked()[0].source.clone();
    let data = log.prepare(&raw).unwrap();
    workbench.preview_workspace(
        &key,
        Progress {
            symbols,
            metadata: BTreeMap::new(),
            data: BTreeMap::from([(raw.clone(), data)]),
        },
    );
    assert!(
        panes
            .iter()
            .all(|pane| pane.lines.with_untracked(Vec::len) == 1)
    );
    assert!(workbench.derived.with_untracked(Vec::is_empty));
    assert!(
        workbench
            .profiles
            .instances
            .with_untracked(BTreeMap::is_empty)
    );
    let bounds = plot::Ranges::new((10., 20.), (-9., 9.));
    for pane in &panes {
        pane.plot.set_view(bounds).unwrap();
    }
    let line_ids: Vec<_> = panes
        .iter()
        .map(|pane| pane.lines.get_untracked()[0].id)
        .collect();
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
            && workbench
                .workspace()
                .loading
                .with_untracked(Option::is_none)
    });
    for (pane, id) in panes.iter().zip(line_ids) {
        assert_eq!(pane.lines.with_untracked(Vec::len), 1);
        assert_eq!(pane.lines.get_untracked()[0].id, id);
        assert_eq!(pane.plot.ranges.get_untracked(), bounds);
    }
    assert!(
        workbench
            .profiles
            .instances
            .with_untracked(|instances| instances[&key].placements.len() >= panes.len())
    );
    let recipe = crate::profile::load(&key.path)
        .unwrap()
        .workspace_presets
        .remove(0);
    let instance = workbench
        .profiles
        .instances
        .with_untracked(|instances| instances[&key].clone());
    workbench.profiles.instances.update(|instances| {
        instances.remove(&key);
    });
    workbench.begin_workspace_load(&key, recipe).unwrap();
    workbench.profiles.instances.update(|instances| {
        instances.insert(key.clone(), instance.clone());
    });
    workbench.complete_workspace_load(&instance);
    assert!(workbench.workspace_finishing());
    let targets = workbench
        .panes()
        .with_untracked(|panes| panes.iter().map(|pane| pane.id).collect::<Vec<_>>());
    let workspace = workbench.workspaces.active.get_untracked();
    workbench.select_workspace(workspace);
    workbench.use_saved_preset(key.clone(), "Streamed".into(), ApplyAction::Workspace);
    workbench.complete_workspace_load(&instance);
    assert_eq!(workbench.workspaces.active.get_untracked(), workspace);
    assert!(
        !workbench.profiles.busy.get_untracked(),
        "Reselecting loading workspace must not restart preparation"
    );
    assert!(workbench.authoring.pending.get_value().is_none());
    assert_eq!(
        workbench
            .panes()
            .with_untracked(|panes| panes.iter().map(|pane| pane.id).collect::<Vec<_>>()),
        targets
    );
    settle(&window, workbench, || !workbench.workspace_finishing());
    workbench.use_saved_preset(key, "Streamed".into(), ApplyAction::Workspace);
    settle(&window, workbench, || {
        workbench.panes().with_untracked(|panes| {
            panes
                .iter()
                .all(|pane| !pane.lines.with_untracked(Vec::is_empty))
        })
    });
}

#[test]
fn closing_loading_workspace_discards_late_previews_and_profile_results() {
    let window = Window::open();
    let source = window.scope.with(Workbench::new);
    five_views(&window, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cancel.json");
    crate::preset_store::save(
        preset_capture::capture(source, &CaptureKind::Workspace, "Cancel").unwrap(),
        &path,
        false,
        None,
    )
    .unwrap();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(source.log.get_untracked());
    workbench.open_saved_workspace(SavedWorkspace {
        path: path.clone(),
        name: "Cancel".into(),
        views: 5,
        definition: Arc::new(crate::profile::load(&path).unwrap()),
    });
    workbench.close_workspace(workbench.workspaces.active.get_untracked());
    assert!(workbench.authoring.workspace_load.get_value().is_none());
    assert!(!workbench.profiles.busy.get_untracked());
    for _ in 0..10 {
        window.frame();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(workbench.workspace().name.get_untracked(), "Workspace 1");
    assert!(
        workbench
            .profiles
            .instances
            .with_untracked(|instances| instances.is_empty())
    );
    workbench.open_saved_workspace(SavedWorkspace {
        path: path.clone(),
        name: "Cancel".into(),
        views: 5,
        definition: Arc::new(crate::profile::load(&path).unwrap()),
    });
    let interrupted = workbench.workspace_id();
    assert!(workbench.authoring.workspace_load.get_value().is_some());
    workbench.open(fixture());
    settle(&window, workbench, || {
        !workbench.loading.get_untracked() && !workbench.profiles.busy.get_untracked()
    });
    // The interrupted recipe can still be selected and populated against the new log.
    workbench.select_workspace(interrupted);
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
            && workbench.views.items.with_untracked(|views| {
                views.len() == 5
                    && views.iter().all(|view| {
                        view.panes.with_untracked(|panes| {
                            panes
                                .iter()
                                .all(|pane| pane.lines.with_untracked(|lines| lines.len() == 1))
                        })
                    })
            })
    });
}

#[test]
fn saved_workspace_restores_five_views_before_log_and_populates_after_log_open() {
    let window = Window::open();
    let source = window.scope.with(Workbench::new);
    five_views(&window, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tools.json");
    let bundle = preset_capture::capture(source, &CaptureKind::Workspace, "Tools").unwrap();
    assert_eq!(bundle.document["workspace_presets"][0]["active"], 2);
    let previous = bundle.document.clone();
    source
        .pane()
        .plot
        .set_view(plot::Ranges::new((10., 20.), (-9., 9.)))
        .unwrap();
    assert_eq!(
        preset_capture::capture(source, &CaptureKind::Workspace, "Tools")
            .unwrap()
            .document,
        previous
    );
    crate::preset_store::save(bundle, &path, false, None).unwrap();
    let definition = Arc::new(crate::profile::load(&path).unwrap());
    let workbench = window.scope.with(Workbench::new);
    workbench.open_saved_workspace(SavedWorkspace {
        path: path.clone(),
        name: "Tools".into(),
        views: 5,
        definition,
    });
    let id = workbench.workspaces.active.get_untracked();
    assert_eq!(workbench.views.items.with_untracked(Vec::len), 5);
    assert_eq!(workbench.view().name.get_untracked(), "View 3");
    workbench.open(fixture());
    settle(&window, workbench, || {
        !workbench.loading.get_untracked()
            && !workbench.profiles.busy.get_untracked()
            && workbench.views.items.with_untracked(|views| {
                views.len() == 5
                    && views.iter().all(|view| {
                        view.panes.with_untracked(|panes| {
                            panes
                                .iter()
                                .all(|pane| pane.lines.with_untracked(|lines| lines.len() == 1))
                        })
                    })
            })
    });
    assert_eq!(workbench.workspaces.active.get_untracked(), id);
    assert_eq!(workbench.workspaces.items.with_untracked(Vec::len), 2);
    assert_eq!(workbench.view().name.get_untracked(), "View 3");
    let definition = crate::profile::load(&path).unwrap();
    assert_eq!(
        crate::preset_lookup::view(&definition, &crate::preset_lookup::view_key("Tools", 1))
            .unwrap()
            .name,
        "View 2"
    );
    let nested = crate::preset_lookup::plot_key(&crate::preset_lookup::view_key("Tools", 1), 0);
    assert!(crate::preset_lookup::plot(&definition, &nested).is_some());

    // Reloading a log keeps the live workspace, including edits absent from its preset.
    let extra = workbench.new_view();
    workbench.rename_view("Session edits".into());
    let plot = workbench.add_plot(workbench.pane().id, false, true);
    let view_ids = workbench
        .views
        .items
        .with_untracked(|views| views.iter().map(|view| view.id).collect::<Vec<_>>());
    let layout = workbench.layout().get_untracked();
    workbench.open(fixture());
    settle(&window, workbench, || {
        !workbench.loading.get_untracked() && !workbench.profiles.busy.get_untracked()
    });
    assert_eq!(workbench.workspace_id(), id);
    assert_eq!(workbench.views.active.get_untracked(), extra);
    assert_eq!(workbench.active().get_untracked(), plot.id);
    assert_eq!(workbench.layout().get_untracked(), layout);
    assert_eq!(
        workbench
            .views
            .items
            .with_untracked(|views| views.iter().map(|view| view.id).collect::<Vec<_>>()),
        view_ids
    );
    assert_eq!(workbench.view().name.get_untracked(), "Session edits");
    let other = workbench
        .workspaces
        .items
        .with_untracked(|items| items.iter().find(|item| item.id != id).unwrap().id);
    workbench.select_workspace(other);
    workbench.select_workspace(id);
    assert_eq!(workbench.views.active.get_untracked(), extra);
    assert!(!workbench.profiles.busy.get_untracked());
}

#[test]
fn unopened_saved_workspace_still_loads_after_replacing_the_log() {
    let window = Window::open();
    let source = window.scope.with(Workbench::new);
    five_views(&window, source);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("deferred.json");
    crate::preset_store::save(
        preset_capture::capture(source, &CaptureKind::Workspace, "Deferred").unwrap(),
        &path,
        false,
        None,
    )
    .unwrap();
    let workbench = window.scope.with(Workbench::new);
    let initial = workbench.workspace_id();
    workbench.open_saved_workspace(SavedWorkspace {
        path: path.clone(),
        name: "Deferred".into(),
        views: 5,
        definition: Arc::new(crate::profile::load(&path).unwrap()),
    });
    let deferred = workbench.workspace_id();
    workbench.select_workspace(initial);
    for _ in 0..2 {
        workbench.open(fixture());
        settle(&window, workbench, || {
            !workbench.loading.get_untracked() && !workbench.profiles.busy.get_untracked()
        });
        assert_eq!(workbench.workspace_id(), initial);
    }
    workbench.select_workspace(deferred);
    settle(&window, workbench, || {
        !workbench.profiles.busy.get_untracked()
            && workbench.views.items.with_untracked(|views| {
                views.len() == 5
                    && views.iter().all(|view| {
                        view.panes.with_untracked(|panes| {
                            panes
                                .iter()
                                .all(|pane| pane.lines.with_untracked(|lines| lines.len() == 1))
                        })
                    })
            })
    });
    assert_eq!(workbench.view().name.get_untracked(), "View 3");
}

#[test]
fn workspace_catalogue_loads_without_log_or_profiles_panel() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("default.json");
    let bundle =
        preset_capture::capture(workbench, &CaptureKind::Workspace, "Empty workspace").unwrap();
    crate::preset_store::save(bundle, &path, false, None).unwrap();
    workbench
        .profiles
        .directory
        .set(Some(directory.path().to_owned()));
    window
        .scope
        .with(|| crate::workspace_catalogue::watch(workbench));
    settle(&window, workbench, || {
        workbench
            .workspaces
            .saved
            .with_untracked(|saved| saved.len() == 1)
    });
    assert!(workbench.log.with_untracked(Option::is_none));
    assert_eq!(
        workbench.workspaces.saved.get_untracked()[0].name,
        "Empty workspace"
    );
    assert_eq!(workbench.workspaces.saved.get_untracked()[0].views, 1);
}
