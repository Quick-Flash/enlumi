use crate::{
    app::WorkbenchAppProps, browser::BrowserProps, state::Workbench, workspace::PlotWorkspaceProps,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use zgui::prelude::*;
use zgui::vocab::{EventKind, Payload, PointerEvent};
use zgui_testkit_view::Window;
use zgui_ui_tokens::prelude::*;

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    ))
}
fn all(window: &Window) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending = vec![window.root];
    while let Some(node) = pending.pop() {
        result.push(node);
        let mut children = window.dom.tree().children(node);
        children.reverse();
        pending.extend(children);
    }
    result
}
fn class(window: &Window, name: &str) -> Vec<NodeId> {
    all(window)
        .into_iter()
        .filter(|node| {
            window
                .dom
                .tree()
                .classes(*node)
                .contains(&zgui::view::ClassName::new(name))
        })
        .collect()
}
fn button(window: &Window, label: &str) -> NodeId {
    all(window)
        .into_iter()
        .find(|node| {
            window
                .dom
                .tree()
                .semantics(*node)
                .is_some_and(|s| s.role == Role::Button)
                && window.dom.tree().text_content(*node) == label
        })
        .unwrap_or_else(|| panic!("Missing button: {label}"))
}
fn click(window: &Window, node: NodeId) {
    window.dispatcher().send_to(
        node,
        EventKind::Click,
        Payload::Pointer(PointerEvent::mouse(zgui::geom::Point::new(
            zgui::geom::CssPx(0.0),
            zgui::geom::CssPx(0.0),
        ))),
    );
    window.frame();
}
fn settle(window: &Window, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.frame();
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "Worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn load(window: &Window, workbench: Workbench) {
    window.scope.with(|| workbench.open(fixture()));
    settle(window, || {
        !workbench.loading.get_untracked()
            && !workbench.profiles.busy.get_untracked()
            && !workbench.derived_busy.get_untracked()
    });
    assert!(
        workbench.log.get_untracked().is_some(),
        "{}",
        workbench.status.get_untracked()
    );
}

#[test]
fn plot_series_snapshot_is_shared_during_navigation_and_refreshes_after_line_edits() {
    let window = Window::open();
    let pane = window.scope.with(|| crate::state::Pane::new(1));
    let data = plot::time_series::SeriesData::new(vec![
        plot::PlotPoint { x: 0., y: 1. },
        plot::PlotPoint { x: 1., y: 2. },
    ])
    .unwrap();
    pane.lines.set(vec![crate::state::Line {
        id: 1,
        source: "sample".into(),
        name: "Original".into(),
        data: data.clone(),
        revision: 0,
    }]);
    let original = pane.series.get_untracked();
    for index in 0..100 {
        let x = index as f64;
        pane.plot
            .ranges
            .set(plot::Ranges::new((x, x + 1.), (0., 2.)));
        let next = pane.series.get_untracked();
        assert!(
            std::ptr::eq(original.series(), next.series()),
            "Navigation must not reconstruct immutable series lists"
        );
    }
    pane.lines.update(|lines| lines[0].name = "Renamed".into());
    let renamed = pane.series.get_untracked();
    assert_eq!(renamed.series()[0].label(), "Renamed");
    assert!(!std::ptr::eq(original.series(), renamed.series()));
    assert!(std::ptr::eq(
        original.series()[0].data().samples(),
        renamed.series()[0].data().samples()
    ));
    pane.lines.update(|lines| {
        lines[0].data =
            plot::time_series::SeriesData::new(vec![plot::PlotPoint { x: 0., y: 9. }]).unwrap();
        lines[0].revision += 1;
    });
    assert_eq!(
        pane.series.get_untracked().series()[0].data().samples()[0].y,
        9.
    );
    pane.lines.set(Vec::new());
    assert!(pane.series.get_untracked().series().is_empty());
}

#[test]
fn analysis_drops_reuse_spectral_plots_preserve_styles_and_keep_time_fields_separate() {
    use plot::line_draw::Connection;
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    load(&window, workbench);
    let normal = workbench.pane();
    let sources = workbench
        .log
        .get_untracked()
        .unwrap()
        .catalogue
        .iter()
        .filter(|field| field.count > 2)
        .take(2)
        .map(|field| field.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    window.scope.with(|| workbench.add(sources[0].clone()));
    settle(&window, || !normal.lines.get_untracked().is_empty());
    window
        .scope
        .with(|| workbench.drop_analysis_sources(vec![sources[0].clone()], Some(normal.id)));
    settle(&window, || !workbench.analysis().busy.get_untracked());
    assert_eq!(
        workbench.panes().get_untracked().len(),
        2,
        "{}",
        workbench.analysis().message.get_untracked()
    );
    let fft = workbench.pane();
    let id = fft.lines.get_untracked()[0].id;
    assert_eq!(
        fft.drawing.get_untracked()[&id].connection,
        Connection::Linear
    );
    fft.drawing
        .update(|drawing| drawing.get_mut(&id).unwrap().connection = Connection::Post);
    window
        .scope
        .with(|| workbench.drop_analysis_sources(vec![sources[1].clone()], Some(normal.id)));
    settle(&window, || !workbench.analysis().busy.get_untracked());
    assert_eq!(workbench.panes().get_untracked().len(), 2);
    assert_eq!(fft.lines.get_untracked().len(), 2);
    assert_eq!(normal.lines.get_untracked().len(), 1);
    assert_eq!(
        fft.drawing.get_untracked()[&id].connection,
        Connection::Post
    );
    assert!(!workbench.can_place_line(fft.id, id, normal.id));
    window.scope.with(|| {
        workbench.edit_analysis(fft);
        workbench.analysis().remove_dc.set(false);
        workbench.run_analysis();
    });
    let updated_view = plot::Ranges::new((10., 20.), (-4., 8.));
    fft.plot.set_view(updated_view).unwrap();
    settle(&window, || !workbench.analysis().busy.get_untracked());
    assert_eq!(fft.plot.ranges.get_untracked(), updated_view);
    assert_eq!(
        fft.drawing.get_untracked()[&id].connection,
        Connection::Post
    );
    assert!(!fft.analysis.get_untracked().unwrap().options.remove_dc);
    window.scope.with(|| workbench.edit_analysis(fft));
    assert!(!workbench.analysis().remove_dc.get_untracked());
    let second = window
        .scope
        .with(|| workbench.add_analysis_panel(fft.analysis.get_untracked().unwrap()));
    assert!(!workbench.can_place_line(fft.id, id, second.id));
    let other_id = second.lines.get_untracked()[0].id;
    second.remove_line(other_id);
    assert!(workbench.can_place_line(fft.id, id, second.id));
}

#[test]
fn analysis_selection_stays_separate_and_conditioner_is_not_a_spectral_input() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench
        .selected_fields
        .set(std::collections::BTreeSet::from(["raw".into()]));
    window.scope.with(|| {
        workbench.select_field(
            "@analysis/raw".into(),
            false,
            false,
            &["@analysis/raw".into()],
        )
    });
    assert_eq!(
        workbench
            .selected_fields
            .get_untracked()
            .into_iter()
            .collect::<Vec<_>>(),
        ["@analysis/raw"]
    );
    workbench.set_analysis_condition(vec!["rpm".into()]);
    assert_eq!(workbench.analysis().condition.get_untracked(), "rpm");
    assert!(workbench.analysis().sources.get_untracked().is_empty());
    workbench.set_analysis_condition(vec!["x".into(), "y".into()]);
    assert_eq!(workbench.analysis().condition.get_untracked(), "rpm");
}

#[test]
fn app_starts_with_one_full_workspace_and_adds_plots_with_keyboard() {
    let window = Window::open();
    let mut built = window.scope.with(|| {
        provide_local_context(crate::update_state::UpdateState::isolated());
        view! { WorkbenchApp(initial_path = None) }
            .into_view()
            .build(&mut window.cx.cx())
    });
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    assert_eq!(class(&window, "workbench-plot").len(), 1);
    assert!(
        class(&window, "editor").is_empty(),
        "Filter editor belongs in settings, not permanent sidebar"
    );
    assert!(class(&window, "new-plot-button").is_empty());
    assert_eq!(class(&window, "sidebar-toggle-icon").len(), 1);
    assert!(
        !all(&window)
            .iter()
            .any(|node| window.dom.tree().text_content(*node) == "Fit all")
    );
    assert!(class(&window, "plot-actions-trigger").is_empty());
    let add = button(&window, "Duplicate");
    window
        .dispatcher()
        .key(add, zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter));
    window.frame();
    assert_eq!(class(&window, "workbench-plot").len(), 2);
    assert_eq!(class(&window, "split-grip").len(), 1);
    assert_eq!(
        window
            .dom
            .tree()
            .semantics(class(&window, "status")[0])
            .unwrap()
            .role,
        Role::Status
    );
}

#[test]
fn preset_editor_hides_obsolete_plot_groups_without_deleting_profile_data() {
    use crate::{preset_capture::Bundle, preset_dialog::PresetDialogProps, preset_ui::SaveSession};
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.authoring.name.set("Example".into());
    workbench.authoring.session.set(Some(SaveSession{view:1,kind:None,origin:None,previous:None,bundle:Bundle{
        document:serde_json::json!({"version":1,"name":"Example","views":[{"name":"Motor","fields":["sensor/0/rpm"]}]}),files:Default::default(),
    }}));
    workbench.authoring.open.set(true);
    let mut built = window.scope.with(|| {
        view! {ThemeProvider(scheme=ColorScheme::Dark){PresetDialog(workbench=workbench)}}
            .into_view()
            .build(&mut window.cx.cx())
    });
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    assert_eq!(class(&window, "profile-builder-entry").len(), 0);
    assert_eq!(
        workbench.authoring.session.with_untracked(|session| session
            .as_ref()
            .unwrap()
            .bundle
            .document["views"]
            .as_array()
            .unwrap()
            .len()),
        1
    );
}

#[test]
fn field_search_preserves_saved_outputs_and_their_expansion() {
    use crate::{profile_library::ProfileFile, profile_preset_tree::ProfilePresetTreeProps};
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let files = window.scope.with(|| {
        RwSignal::new(vec![ProfileFile {
            path: PathBuf::from("example.json"),
            name: "Example".into(),
            compatible: Ok(()),
            auto_apply: false,
            matched: String::new(),
            views: vec![("Timeline".into(), 1)],
            workspaces: vec!["Flight view".into()],
            workspace_plots: vec![("Flight view".into(), vec![])],
            collections: vec![("Flight workspace".into(), vec![])],
            fields: vec![
                ("rpm".into(), "Motor RPM".into()),
                ("voltage".into(), "Voltage".into()),
            ],
            geometry: vec![("position".into(), "Position".into())],
        }])
    });
    let catalogue_reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let reads = catalogue_reads.clone();
    let catalogue = window.scope.with(|| {
        Signal::derive(move || {
            reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            files.get()
        })
    });
    let mut built = window.scope.with(|| {
        view! { ThemeProvider(scheme=ColorScheme::Dark) {
            ProfilePresetTree(workbench=workbench, files=catalogue, field_query=workbench.field_query.into())
        }}.into_view().build(&mut window.cx.cx())
    });
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    click(&window, button(&window, "Fields2"));
    click(&window, button(&window, "Geometry1"));
    // Keep a nested view expanded while the field results change.
    let toggle =
        class(&window, "profile-members-toggle")
            .into_iter()
            .find(|node| {
                window.dom.tree().semantics(*node).is_some_and(|s| {
                    s.label.as_deref() == Some("Show plots and fields in view preset")
                })
            })
            .unwrap();
    click(&window, toggle);
    let preserved = [
        "workspace-collection",
        "workspace-preset",
        "profile-plot-source",
    ]
    .map(|name| class(&window, name));
    assert!(preserved.iter().all(|nodes| !nodes.is_empty()));
    let reads_before_search = catalogue_reads.load(std::sync::atomic::Ordering::Relaxed);
    for query in ["rpm", "no matching field", ""] {
        workbench.field_query.set(query.into());
        window.frame();
        let text = window.dom.tree().text_content(window.root);
        assert!(text.contains("Position"));
        assert_eq!(text.contains("Motor RPM"), query != "no matching field");
        assert_eq!(text.contains("Voltage"), query.is_empty());
        for (name, nodes) in [
            "workspace-collection",
            "workspace-preset",
            "profile-plot-source",
        ]
        .into_iter()
        .zip(&preserved)
        {
            assert_eq!(
                &class(&window, name),
                nodes,
                "Search must preserve {name} controls"
            );
        }
        assert_eq!(
            window.dom.tree().semantics(toggle).unwrap().expanded,
            Some(true)
        );
        assert_eq!(
            catalogue_reads.load(std::sync::atomic::Ordering::Relaxed),
            reads_before_search,
            "Typing a query must reuse the indexed catalogue"
        );
    }
    files.update(|files| files[0].fields[0].1 = "Updated RPM".into());
    window.frame();
    assert!(
        window
            .dom
            .tree()
            .text_content(window.root)
            .contains("Updated RPM"),
        "Catalogue changes refresh field rows"
    );
}

#[test]
fn tree_disclosures_search_selection_and_keyboard_add_work() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let mut built = window.scope.with(|| {
        view! { ThemeProvider(scheme = ColorScheme::Dark) { Browser(workbench = workbench) } }
            .into_view()
            .build(&mut window.cx.cx())
    });
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    load(&window, workbench);
    window.place(class(&window, "signal-list")[0], 0.0, 0.0, 300.0, 600.0);
    window.frame();
    assert!(workbench.browser_fields.with_untracked(Vec::is_empty));
    click(&window, class(&window, "raw-section")[0]);
    let topic = class(&window, "topic-branch")[0];
    assert!(
        !class(&window, "zui-button").contains(&topic),
        "Tree disclosure must use plain text row"
    );
    assert_eq!(
        window.dom.tree().semantics(topic).unwrap().expanded,
        Some(false)
    );
    window.dispatcher().key(
        topic,
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight),
    );
    window.frame();
    assert_eq!(
        window.dom.tree().semantics(topic).unwrap().expanded,
        Some(true)
    );
    window.frame();
    assert!(
        !class(&window, "field-branch").is_empty(),
        "Tree after expansion: {}",
        window.dom.tree().text_content(window.root)
    );
    let expanded = workbench.expanded.get_untracked();
    workbench.field_query.set("esc_rpm".into());
    window.frame();
    assert_eq!(workbench.expanded.get_untracked(), expanded);
    assert!(workbench.browser_fields.with_untracked(Vec::is_empty));
    for _ in 0..8 {
        if !workbench.browser_fields.with_untracked(Vec::is_empty) {
            break;
        }
        let branch = class(&window, "tree-branch")
            .into_iter()
            .find(|node| window.dom.tree().semantics(*node).unwrap().expanded == Some(false))
            .expect("Matching field must retain expandable ancestors");
        window.dispatcher().key(
            branch,
            zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight),
        );
        window.frame();
    }
    let leaf = class(&window, "metric-option")
        .into_iter()
        .find(|node| window.dom.tree().text_content(*node).contains("esc_rpm"))
        .unwrap();
    click(&window, leaf);
    assert!(
        workbench
            .selected_field
            .get_untracked()
            .unwrap()
            .contains("esc_rpm")
    );
    let pane = workbench.pane();
    assert!(
        pane.lines.get_untracked().is_empty(),
        "Selection alone must not add line"
    );
    window
        .dispatcher()
        .key(leaf, zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter));
    window.frame();
    settle(&window, || {
        pane.lines.with_untracked(|lines| lines.len() == 1)
    });
    assert!(pane.lines.get_untracked()[0].source.contains("esc_rpm"));
}

#[test]
fn filtering_and_plot_state_survive_transient_settings_owner() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    load(&window, workbench);
    let pane = workbench.pane();
    let source = workbench.log.get_untracked().unwrap().catalogue[0]
        .id
        .clone();
    window.scope.with(|| workbench.add(source));
    settle(&window, || {
        pane.lines.with_untracked(|lines| lines.len() == 1)
    });
    let raw = pane.selection().unwrap();
    let transient = zgui::reactive::Owner::new();
    let first = transient
        .with(|| {
            workbench.upsert_filter(
                None,
                "Smoothed".into(),
                raw.source.clone(),
                crate::derived::FilterSpec::LowPass(5.0),
            )
        })
        .unwrap();
    drop(transient);
    settle(&window, || !workbench.derived_busy.get_untracked());
    window
        .scope
        .with(|| workbench.add(crate::derived::key(first)));
    assert_eq!(pane.lines.get_untracked().len(), 2);
    assert_eq!(pane.selection().unwrap().label(), "Smoothed");
    assert_eq!(
        pane.lines.get_untracked()[0].data.samples().as_ptr(),
        raw.data.samples().as_ptr()
    );
    window
        .scope
        .with(|| {
            workbench.upsert_filter(
                Some(first),
                "Smoothed again".into(),
                raw.source.clone(),
                crate::derived::FilterSpec::LowPass(2.5),
            )
        })
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert_eq!(pane.selection().unwrap().label(), "Smoothed again");
    let transient = zgui::reactive::Owner::new();
    let second = transient.with(|| workbench.add_plot(pane.id, false, false));
    drop(transient);
    second
        .plot
        .ranges
        .set(plot::Ranges::new((1.0, 5.0), (-2.0, 2.0)));
    workbench.activate(pane.id);
    let moving = pane.selected.get_untracked().unwrap();
    let appearance = {
        plot::line_draw::LineDraw {
            connection: plot::line_draw::Connection::Post,
            markers: true,
            ..Default::default()
        }
    };
    pane.drawing.update(|drawing| {
        drawing.insert(moving, appearance);
    });
    window
        .scope
        .with(|| workbench.place_line(pane.id, moving, second.id, None));
    assert_eq!(second.lines.get_untracked().len(), 1);
    assert_eq!(pane.lines.get_untracked().len(), 1);
    assert_eq!(workbench.active().get_untracked(), second.id);
    assert_eq!(second.drawing.get_untracked()[&moving], appearance);
    assert!(!pane.drawing.get_untracked().contains_key(&moving));
}

#[test]
fn latest_load_wins_and_reload_preserves_compatible_lines() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    window.scope.with(|| {
        workbench.open(PathBuf::from("does-not-exist.ulg"));
        workbench.open(fixture());
    });
    settle(&window, || !workbench.loading.get_untracked());
    assert!(workbench.log.get_untracked().is_some());
    let source = workbench.log.get_untracked().unwrap().catalogue[0]
        .id
        .clone();
    window.scope.with(|| workbench.add(source));
    settle(&window, || {
        !workbench.pane().lines.get_untracked().is_empty()
    });
    let before = workbench.pane().lines.get_untracked()[0].clone();
    workbench.selected_field.set(Some("old field".into()));
    load(&window, workbench);
    let lines = workbench.pane().lines.get_untracked();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].id, before.id);
    assert_eq!(lines[0].source, before.source);
    assert_ne!(
        lines[0].data.samples().as_ptr(),
        before.data.samples().as_ptr()
    );
    assert!(workbench.selected_field.get_untracked().is_none());
}

#[test]
fn docking_tabs_split_close_duplicate_and_maximize_preserve_data() {
    use plot::workspace::layout::DropZone;
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    load(&window, workbench);
    let first = workbench.pane();
    let source = workbench.log.get_untracked().unwrap().catalogue[0]
        .id
        .clone();
    window.scope.with(|| workbench.add(source));
    settle(&window, || {
        first.lines.with_untracked(|lines| lines.len() == 1)
    });
    let original = first.lines.get_untracked()[0].clone();
    first.plot.hidden.update(|hidden| {
        hidden.insert(original.id.to_string());
    });
    let tab = window
        .scope
        .with(|| workbench.add_plot(first.id, false, true));
    assert_eq!(
        workbench.layout().get_untracked().tab_ids(first.id),
        [first.id, tab.id]
    );
    workbench.activate(first.id);
    assert!(!workbench.layout().get_untracked().is_visible(tab.id));
    workbench.layout().update(|layout| {
        assert!(layout.dock_split(tab.id, first.id, DropZone::Bottom));
    });
    assert!(workbench.layout().get_untracked().is_visible(tab.id));
    workbench.maximize(first.id);
    assert_eq!(workbench.maximized().get_untracked(), Some(first.id));
    workbench.layout().update(|layout| {
        if let plot::workspace::layout::Layout::Split { ratio, .. } = layout {
            *ratio = 0.8;
        }
    });
    workbench.equalize_panels();
    assert!(workbench.maximized().get_untracked().is_none());
    assert!(matches!(
        workbench.layout().get_untracked(),
        plot::workspace::layout::Layout::Split { ratio: 0.5, .. }
    ));
    assert_eq!(first.lines.get_untracked()[0].id, original.id);
    let appearance = {
        plot::line_draw::LineDraw {
            connection: plot::line_draw::Connection::Pre,
            markers: true,
            ..Default::default()
        }
    };
    first.drawing.update(|drawing| {
        drawing.insert(original.id, appearance);
    });
    window.scope.with(|| workbench.duplicate_plot(first));
    let copy = workbench.pane();
    let copied = copy.lines.get_untracked()[0].clone();
    assert_ne!(original.id, copied.id);
    assert_eq!(copy.drawing.get_untracked()[&copied.id], appearance);
    assert_eq!(
        copy.plot.style(&copied.id.to_string()),
        first.plot.style(&original.id.to_string())
    );
    assert!(
        copy.plot
            .hidden
            .get_untracked()
            .contains(&copied.id.to_string())
    );
    copy.lines.set(Vec::new());
    assert_eq!(first.lines.get_untracked().len(), 1);
    workbench.close_plot(copy.id);
    workbench.close_plot(tab.id);
    workbench.close_plot(first.id);
    assert_eq!(workbench.panes().get_untracked().len(), 1);
    assert!(workbench.pane().lines.get_untracked().is_empty());
}

#[test]
fn linked_docked_plots_share_time_and_keep_independent_y_ranges() {
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let first = workbench.pane();
    let second = window
        .scope
        .with(|| workbench.add_plot(first.id, false, false));
    first.links.update(|links| links.x = true);
    second.links.update(|links| links.x = true);
    let mut built = window.scope.with(|| {
        view! { ThemeProvider(scheme = ColorScheme::Dark) { PlotWorkspace(workbench = workbench) } }
            .into_view()
            .build(&mut window.cx.cx())
    });
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    first
        .plot
        .set_view(plot::Ranges::new((2.0, 8.0), (-3.0, 3.0)))
        .unwrap();
    window.frame();
    assert_eq!(
        first.plot.ranges.get_untracked().x,
        second.plot.ranges.get_untracked().x
    );
    assert_ne!(
        first.plot.ranges.get_untracked().y,
        second.plot.ranges.get_untracked().y
    );
    second
        .plot
        .set_view(plot::Ranges::new((3.0, 7.0), (100.0, 200.0)))
        .unwrap();
    window.frame();
    assert_eq!(
        first.plot.ranges.get_untracked().x,
        plot::Range::new(3.0, 7.0)
    );
    assert_eq!(
        first.plot.ranges.get_untracked().y,
        plot::Range::new(-3.0, 3.0)
    );
}

#[test]
fn editor_styles_follow_selected_line_only() {
    use crate::{editor::EditorProps, state::Line};
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let pane = workbench.pane();
    let data =
        plot::time_series::SeriesData::new(vec![plot::PlotPoint { x: 0.0, y: 1.0 }]).unwrap();
    pane.lines.set(
        [1, 2]
            .map(|id| Line {
                id,
                source: format!("line {id}"),
                name: format!("line {id}"),
                data: data.clone(),

                revision: 0,
            })
            .to_vec(),
    );
    workbench.select(pane.id, 1);
    let mut built = window.scope.with(|| view! { ThemeProvider(scheme = ColorScheme::Dark) { Editor(workbench = workbench, pane = pane) } }.into_view().build(&mut window.cx.cx()));
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    let untouched = pane.plot.style("2");
    let before = pane.plot.style("1").color;
    window.dispatcher().key(
        class(&window, "color-square")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight),
    );
    window.frame();
    let changed = pane.plot.style("1").color;
    assert_ne!(changed, before);
    window.dispatcher().key(
        class(&window, "line-width")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::End),
    );
    window.frame();
    click(&window, class(&window, "line-marker")[3]);
    click(&window, class(&window, "line-connection")[1]);
    assert_eq!(
        pane.plot.style("1").marker,
        plot::plot_data::MarkerShape::Triangle
    );
    assert_eq!(
        pane.drawing.get_untracked()[&1].connection,
        plot::line_draw::Connection::Pre
    );
    assert!(pane.drawing.get_untracked()[&1].markers);
    assert!(class(&window, "zui-checkbox").is_empty());
    assert_eq!(pane.plot.style("1").color, changed);
    assert_eq!(pane.plot.style("1").width, 10.0);
    assert_eq!(pane.plot.style("2"), untouched);
    click(&window, class(&window, "line-button")[1]);
    assert_eq!(class(&window, "line-style").len(), 1);
    window.dispatcher().key(
        class(&window, "color-square")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight),
    );
    window.frame();
    assert_ne!(pane.plot.style("2").color, untouched.color);
    assert_eq!(
        pane.drawing
            .with_untracked(|drawing| drawing.get(&2).copied().unwrap_or_default()),
        plot::line_draw::LineDraw::default()
    );
    assert_eq!(pane.plot.style("1").color, changed);
    let hex = class(&window, "color-hex")[0];
    for input in ["#123", "#12A0FF"] {
        let before = pane.plot.style("2").color;
        window.dispatcher().send_to(
            hex,
            EventKind::Input,
            Payload::Value(zgui::vocab::ValueEvent {
                value: input.into(),
                selection: Default::default(),
                kind: zgui::vocab::ValueChange::Input,
            }),
        );
        window.frame();
        if input == "#123" {
            assert_eq!(pane.plot.style("2").color, before);
        } else {
            assert_eq!(pane.plot.style("2").color, [18, 160, 255, 255]);
        }
    }
    assert_eq!(pane.plot.style("1").color, changed);
}

#[test]
fn editor_multi_selection_edits_shared_style_without_changing_colors() {
    use crate::{editor::EditorProps, state::Line};
    let window = Window::open();
    let workbench = window.scope.with(Workbench::new);
    let pane = workbench.pane();
    let data = plot::time_series::SeriesData::new(vec![]).unwrap();
    pane.lines.set(
        (1..=4)
            .map(|id| Line {
                id,
                source: id.to_string(),
                name: id.to_string(),
                data: data.clone(),
                revision: 0,
            })
            .collect(),
    );
    workbench.select(pane.id, 1);
    let mut built = window.scope.with(|| view! { ThemeProvider(scheme = ColorScheme::Dark) { Editor(workbench = workbench, pane = pane) } }.into_view().build(&mut window.cx.cx()));
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    let pick = |index, modifiers| {
        window.dispatcher().with_modifiers(modifiers).send_to(
            class(&window, "line-button")[index],
            EventKind::Click,
            Payload::Pointer(PointerEvent::mouse(zgui::geom::Point::new(
                zgui::geom::CssPx(0.0),
                zgui::geom::CssPx(0.0),
            ))),
        );
        window.frame();
    };
    let colors: Vec<_> = (1..=4)
        .map(|id| pane.plot.style(&id.to_string()).color)
        .collect();
    let untouched = pane.plot.style("4");
    pick(2, zgui::vocab::Modifiers::CONTROL);
    assert_eq!(class(&window, "color-square").len(), 1);
    assert_eq!(class(&window, "color-picker-disabled").len(), 1);
    window.dispatcher().key(
        class(&window, "color-square")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight),
    );
    window.dispatcher().key(
        class(&window, "color-hue")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::End),
    );
    window.dispatcher().send_to(
        class(&window, "color-hex")[0],
        EventKind::Input,
        Payload::Value(zgui::vocab::ValueEvent {
            value: "#FF0000".into(),
            selection: Default::default(),
            kind: zgui::vocab::ValueChange::Input,
        }),
    );
    window.frame();
    assert_eq!(
        (1..=4)
            .map(|id| pane.plot.style(&id.to_string()).color)
            .collect::<Vec<_>>(),
        colors
    );
    assert_eq!(class(&window, "selected").len(), 2);
    window.dispatcher().key(
        class(&window, "line-width")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::End),
    );
    window.frame();
    assert_eq!(pane.plot.style("1").width, 10.0);
    assert_ne!(pane.plot.style("2").width, 10.0);
    assert_eq!(pane.plot.style("3").width, 10.0);
    pick(0, zgui::vocab::Modifiers::SHIFT);
    assert_eq!(class(&window, "selected").len(), 3);
    assert!(
        window
            .dom
            .tree()
            .text_content(window.root)
            .contains("Line width: mixed")
    );
    click(&window, class(&window, "line-marker")[2]);
    window.dispatcher().key(
        class(&window, "line-radius")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::End),
    );
    window.frame();
    click(&window, class(&window, "line-connection")[2]);
    window.dispatcher().key(
        class(&window, "line-width")[0],
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::Home),
    );
    window.frame();
    for id in 1..=3 {
        let style = pane.plot.style(&id.to_string());
        assert_eq!(style.width, 0.5);
        assert_eq!(style.radius, 10.0);
        assert_eq!(style.marker, plot::plot_data::MarkerShape::Square);
        assert_eq!(
            pane.drawing.get_untracked()[&id].connection,
            plot::line_draw::Connection::Post
        );
        assert!(pane.drawing.get_untracked()[&id].markers);
    }
    assert_eq!(pane.plot.style("4"), untouched);
    assert_eq!(
        (1..=4)
            .map(|id| pane.plot.style(&id.to_string()).color)
            .collect::<Vec<_>>(),
        colors
    );
    pick(1, zgui::vocab::Modifiers::CONTROL);
    assert_eq!(class(&window, "selected").len(), 2);
    pick(3, zgui::vocab::Modifiers::NONE);
    assert_eq!(class(&window, "selected").len(), 1);
    assert_eq!(class(&window, "color-square").len(), 1);
    assert!(class(&window, "color-picker-disabled").is_empty());
    pick(3, zgui::vocab::Modifiers::CONTROL);
    assert_eq!(
        class(&window, "selected").len(),
        1,
        "Ctrl-click must retain last selection"
    );
    pick(
        3,
        zgui::vocab::Modifiers::SHIFT | zgui::vocab::Modifiers::CONTROL,
    );
    assert_eq!(class(&window, "selected").len(), 1);
    assert!(
        !window
            .dom
            .tree()
            .text_content(window.root)
            .contains("Remove line")
    );
    assert!(
        !window
            .dom
            .tree()
            .text_content(window.root)
            .contains("Move to next plot")
    );
    assert_eq!(class(&window, "legend-add-filters").len(), 1);
}

#[test]
fn outer_docking_spans_stacked_panels_and_preserves_tabs() {
    use plot::workspace::layout::{DropZone, Layout, Rect};
    for zone in [
        DropZone::Left,
        DropZone::Right,
        DropZone::Top,
        DropZone::Bottom,
    ] {
        let mut layout = Layout::Plot(1);
        assert!(layout.split(1, 2, true));
        assert!(layout.add_plot_tab(1, 3));
        assert!(layout.dock_workspace(3, zone));
        let mut panes = vec![];
        layout.geometry(
            Rect {
                width: 1600.0,
                height: 1200.0,
                ..Rect::default()
            },
            &mut panes,
            &mut vec![],
        );
        let rect = |id| panes.iter().find(|(pane, _)| *pane == id).unwrap().1;
        let (a, b, c) = (rect(1), rect(2), rect(3));
        if matches!(zone, DropZone::Left | DropZone::Right) {
            assert_eq!(c.height, 1200.0);
            assert_eq!(a.x, b.x);
            assert_eq!(a.width, b.width);
            assert!(if zone == DropZone::Left {
                c.x < a.x
            } else {
                c.x > a.x
            });
        } else {
            assert_eq!(c.width, 1600.0);
            assert!(if zone == DropZone::Top {
                c.y < a.y
            } else {
                c.y > b.y
            });
        }
        assert!(!layout.dock_workspace(99, zone));
        assert_eq!(panes.len(), 3);
    }
}

#[test]
fn group_docking_in_middle_preserves_unrelated_siblings_and_handles_collapsed_source_group() {
    use plot::workspace::layout::{DropZone, Layout, Rect};
    let mut original = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0)], false);
    assert!(original.split(2, 4, true));
    assert!(original.add_plot_tab(1, 5));
    let geometry = |layout: &Layout| {
        let mut panes = vec![];
        layout.geometry(
            Rect {
                width: 3000.0,
                height: 1600.0,
                ..Rect::default()
            },
            &mut panes,
            &mut vec![],
        );
        panes
    };
    let before = geometry(&original);
    for zone in [
        DropZone::Left,
        DropZone::Right,
        DropZone::Top,
        DropZone::Bottom,
    ] {
        let mut layout = original.clone();
        assert!(layout.dock_group(5, Some(4), zone));
        let after = geometry(&layout);
        let rect = |id| after.iter().find(|(pane, _)| *pane == id).unwrap().1;
        for id in [1, 3] {
            assert_eq!(
                rect(id),
                before.iter().find(|(pane, _)| *pane == id).unwrap().1
            );
        }
        assert_eq!(after.len(), 5);
        if matches!(zone, DropZone::Left | DropZone::Right) {
            assert_eq!(rect(5).height, rect(2).height + rect(4).height + 6.0);
            assert_eq!(rect(2).x, rect(4).x);
        } else {
            assert_eq!(rect(5).width, rect(2).width);
        }
        let unchanged = layout.clone();
        assert!(!layout.dock_group(5, Some(999), zone));
        assert_eq!(layout, unchanged);
    }
    assert!(original.dock_group(2, Some(4), DropZone::Left));
    assert_eq!(geometry(&original).len(), 5);
}

#[test]
fn contiguous_row_and_column_regions_regroup_either_pair_without_moving_other_band() {
    use plot::workspace::layout::{DropZone, Layout, Rect};
    for stacked in [true, false] {
        for members in [vec![1, 2], vec![2, 3]] {
            for zone in [
                DropZone::Left,
                DropZone::Right,
                DropZone::Top,
                DropZone::Bottom,
            ] {
                let mut layout = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0)], stacked);
                let outside = if members[0] == 1 { 3 } else { 1 };
                assert!(layout.add_plot_tab(outside, 4));
                let geometry = |layout: &Layout| {
                    let mut panes = vec![];
                    layout.geometry(
                        Rect {
                            width: 1800.0,
                            height: 1800.0,
                            ..Rect::default()
                        },
                        &mut panes,
                        &mut vec![],
                    );
                    panes
                };
                let before = geometry(&layout);
                let original = layout.clone();
                assert!(!layout.dock_region(4, &[1, 3], zone, (1800.0, 1800.0)));
                assert_eq!(layout, original);
                assert!(layout.dock_region(4, &members, zone, (1800.0, 1800.0)));
                let after = geometry(&layout);
                let rect = |id| after.iter().find(|(pane, _)| *pane == id).unwrap().1;
                let untouched = before.iter().find(|(pane, _)| *pane == outside).unwrap().1;
                let actual = rect(outside);
                assert!(
                    (actual.x - untouched.x).abs() < 1e-8
                        && (actual.y - untouched.y).abs() < 1e-8
                        && (actual.width - untouched.width).abs() < 1e-8
                        && (actual.height - untouched.height).abs() < 1e-8
                );
                let a = rect(members[0]);
                let b = rect(members[1]);
                let c = rect(4);
                if stacked && matches!(zone, DropZone::Left | DropZone::Right) {
                    assert_eq!(a.x, b.x);
                    assert!((c.height - a.height - b.height - 6.0).abs() < 1e-8);
                }
                if !stacked && matches!(zone, DropZone::Top | DropZone::Bottom) {
                    assert_eq!(a.y, b.y);
                    assert!((c.width - a.width - b.width - 6.0).abs() < 1e-8);
                }
                let mut dividers = vec![];
                layout.geometry(
                    Rect {
                        width: 1800.0,
                        height: 1800.0,
                        ..Rect::default()
                    },
                    &mut vec![],
                    &mut dividers,
                );
                let ids: std::collections::BTreeSet<_> = dividers.iter().map(|d| d.id).collect();
                assert_eq!(ids.len(), dividers.len());
                assert_eq!(after.len(), 4);
            }
        }
    }
}
