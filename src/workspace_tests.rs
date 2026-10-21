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
fn settle(window: &Window, workbench: Workbench, ready: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        window.frame();
        if ready() {
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
