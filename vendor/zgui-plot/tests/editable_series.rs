#![cfg(feature = "workspace")]
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
use zgui::{
    geom::{CssPx, DevicePx, Point, Size},
    platform::SurfaceEvent,
    prelude::*,
    render::Renderer,
    vocab::{Modifiers, PointerAction, PointerEvent, Timestamp},
};
use zgui_platform_headless::Harness;
use zgui_plot as plot;
use zgui_testkit_scene::CaptureRenderer;
fn pointer(action: PointerAction, x: f32, y: f32) -> SurfaceEvent {
    let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(y)));
    event.button = matches!(action, PointerAction::Pressed | PointerAction::Released)
        .then_some(zgui::vocab::PointerButton::Primary);
    SurfaceEvent::Pointer {
        action,
        event,
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    }
}
#[test]
fn line_settings_keep_geometry_fixed_during_selection_changes() {
    use plot::zgui_series::{SeriesActions, SeriesEditorProps, SeriesLine as Line, SeriesPane};
    use zgui_ui_tokens::prelude::*;
    let built = Rc::new(RefCell::new(None));
    let model = Rc::clone(&built);
    let handler = app().with_size(900.0, 700.0)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer))
        }))
        .with_stylesheet(format!("{}\n.test-editor-root {{width:680px;}}", plot::zgui_series::stylesheet()))
        .into_handler(move || {
            let lines = RwSignal::new_local(vec![]);
            let pane = SeriesPane::new(1, Signal::derive_local(move || lines.get()));
            let actions = SeriesActions { extra: Some(UnsyncCallback::new(|_| {})), extra_label: "Extra action", extra_class: "test-extra", ..SeriesActions::default() };
            let data = plot::time_series::SeriesData::new(vec![]).unwrap();
            lines.set((1..=2).map(|id| Line { id, name: format!("Line {id}"), data: data.clone(), revision: 0 }).collect());
            *model.borrow_mut() = Some(pane);
            pane.selected.set(Some(1));
            pane.drawing.update(|drawing| { drawing.entry(2).or_default().markers = true; });
            view! { ThemeProvider(scheme = ColorScheme::Dark) {
                column(class = "plot-settings-surface test-editor-root") { SeriesEditor(actions = actions, pane = pane) }
            } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(700.0),
    )));
    harness.settle(256);
    let classes = [
        "editor",
        "appearance-row",
        "color-picker",
        "color-square",
        "appearance-controls",
        "line-width",
        "line-radius",
        "test-extra",
    ];
    let before: Vec<_> = classes
        .iter()
        .map(|class| harness.app().windows()[0].box_with_class(class).unwrap())
        .collect();
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("marker-radius-disabled")
            .len(),
        1
    );
    let rows = harness.app().windows()[0].nodes_with_class("line-button");
    let rect =
        zgui::view::ViewHost::window_box(harness.app().windows()[0].host().as_ref(), rows[1])
            .unwrap();
    let point = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    assert!(
        rect.size.height.0 <= 26.0,
        "Rows should stay compact: {rect:?}"
    );
    for modifiers in [Modifiers::CONTROL, Modifiers::NONE, Modifiers::CONTROL] {
        for action in [PointerAction::Pressed, PointerAction::Released] {
            let mut event = pointer(action, point.0, point.1);
            if let SurfaceEvent::Pointer {
                modifiers: held, ..
            } = &mut event
            {
                *held = modifiers;
            }
            harness.deliver_to_first(event);
        }
        harness.settle(128);
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("marker-radius-disabled")
                .is_empty()
        );
        for (class, before) in classes.iter().zip(&before) {
            assert_eq!(
                harness.app().windows()[0].box_with_class(class).unwrap(),
                *before,
                "Selection must not move {class}"
            );
        }
    }
    // Selecting None must gray radius again, even after mixed selection changes.
    let window = &harness.app().windows()[0];
    let none = window.nodes_with_class("line-marker")[0];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), none).unwrap();
    for action in [PointerAction::Pressed, PointerAction::Released] {
        harness.deliver_to_first(pointer(
            action,
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        ));
    }
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("marker-radius-disabled")
            .len(),
        1
    );
    for (class, before) in classes.iter().zip(&before) {
        assert_eq!(
            harness.app().windows()[0].box_with_class(class).unwrap(),
            *before
        );
    }
    let window = &harness.app().windows()[0];
    let no_line = window.nodes_with_class("line-pattern")[4];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), no_line).unwrap();
    for action in [PointerAction::Pressed, PointerAction::Released] {
        harness.deliver_to_first(pointer(
            action,
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        ));
    }
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("marker-radius-disabled")
            .is_empty(),
        "No-line mode enables sample markers"
    );
    for (class, before) in classes.iter().zip(&before) {
        assert_eq!(
            harness.app().windows()[0].box_with_class(class).unwrap(),
            *before
        );
    }
    let pane = built.borrow().unwrap();
    assert_eq!(
        pane.drawing.get_untracked()[&2].pattern,
        plot::line_draw::LinePattern::None
    );
    assert!(pane.drawing.get_untracked()[&2].markers);
    let window = &harness.app().windows()[0];
    let none = window.nodes_with_class("line-marker")[0];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), none).unwrap();
    for action in [PointerAction::Pressed, PointerAction::Released] {
        harness.deliver_to_first(pointer(
            action,
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        ));
    }
    harness.settle(128);
    let draw = pane.drawing.get_untracked()[&2];
    assert!(!draw.markers);
    assert_eq!(
        draw.pattern,
        plot::line_draw::LinePattern::Solid,
        "Disabling markers must restore visible connecting line"
    );
}

#[test]
fn stock_time_plot_prepares_step_markers_and_exports_them_without_app_renderer() {
    use plot::{
        line_draw::{Connection, LineDraw, LinePattern},
        time_series::{SeriesData, TimeSeries, TimeSeriesSet},
        zgui_plot::PlotState,
        zgui_svg::PlotSnapshotSource,
        zgui_time_series::TimeSeriesPlotProps,
    };
    use std::collections::BTreeMap;
    let built = Rc::new(RefCell::new(None));
    let model = Rc::clone(&built);
    let handler=app().with_size(900.0,600.0)
        .with_renderer(Box::new(|_,target| { let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer)) }))
        .with_stylesheet(format!("{}\n{}\n.root {{width:100vw;height:100vh;}}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet()))
        .into_handler(move || {
            let data=SeriesData::new(vec![plot::PlotPoint{x:0.0,y:0.0},plot::PlotPoint{x:10.0,y:10.0}]).unwrap();
            let series=TimeSeriesSet::new(vec![TimeSeries::new("sample","sample",data.clone())]).unwrap();
            let state=PlotState::default();
            state.styles.update(|styles| { let mut style=plot::plot_data::SeriesStyle::for_id("sample");style.width=7.0;styles.insert("sample".into(),style); });
            let drawing=RwSignal::new_local(BTreeMap::new());
            let source: PlotSnapshotSource=StoredValue::new_local(None);
            *model.borrow_mut()=Some((source,drawing,data));
            view! { column(class="root") { TimeSeriesPlot(series=Signal::stored_local(series),state=state,options={plot::zgui_plot::PlotOptions {line_drawing:Signal::derive_local(move || drawing.get()),snapshot:Some(source),..Default::default()}}) } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(128);
    let (source, drawing, data) = built.borrow().as_ref().cloned().unwrap();
    let snapshot = |harness: &mut Harness<zgui::runtime::Runtime>,
                    previous: Option<&str>,
                    has_line: bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            harness.settle(128);
            let svg = source.with_value(|callback| callback.as_ref().unwrap()().unwrap().svg.clean);
            if svg.contains("stroke-width=\"7\"") == has_line && previous != Some(svg.as_str()) {
                break svg;
            }
            assert!(
                Instant::now() < deadline,
                "Prepared time geometry never reached export"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    let linear = snapshot(&mut harness, None, true);
    drawing.set(
        [(
            "sample".into(),
            LineDraw {
                connection: Connection::Pre,
                markers: false,
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
    );
    let pre = snapshot(&mut harness, Some(&linear), true);
    drawing.set(
        [(
            "sample".into(),
            LineDraw {
                connection: Connection::Post,
                markers: false,
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
    );
    let post = snapshot(&mut harness, Some(&pre), true);
    assert_ne!(linear, post);
    drawing.update(|drawing| drawing.get_mut("sample").unwrap().markers = true);
    let marked = snapshot(&mut harness, Some(&post), true);
    assert_ne!(marked, post);
    let mut previous = marked;
    for pattern in [
        LinePattern::Dashed,
        LinePattern::Dotted,
        LinePattern::DashDot,
    ] {
        drawing.update(|drawing| {
            let draw = drawing.get_mut("sample").unwrap();
            draw.pattern = pattern;
        });
        let patterned = snapshot(&mut harness, Some(&previous), true);
        assert_ne!(patterned, post, "Pattern must change exported geometry");
        previous = patterned;
    }
    drawing.update(|drawing| {
        let draw = drawing.get_mut("sample").unwrap();
        draw.pattern = LinePattern::None;
        draw.markers = true;
    });
    let markers_only = snapshot(&mut harness, Some(&previous), false);
    assert!(
        markers_only.contains("<circle"),
        "Markers-only export must retain sample dots"
    );

    assert_eq!(
        data.samples().len(),
        2,
        "Render expansion must not replace raw samples"
    );
}
