use super::{
    camera::Camera,
    flight::{Config, FlightMapProps},
};
use crate::{state::Workbench, theme::AppThemeProps, trajectory_plot::OverlayProbe};
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
    vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp},
};
use zgui_platform_headless::Harness;

fn click(harness: &mut Harness<zgui::runtime::Runtime>, class: &str, label: &str) {
    let window = &harness.app().windows()[0];
    let (scope, class) = class
        .split_once(' ')
        .map_or((None, class), |(scope, class)| (Some(scope), class));
    let roots = scope.map(|scope| window.nodes_with_class(scope));
    let node = window
        .nodes_with_class(class)
        .into_iter()
        .find(|node| {
            roots.as_ref().is_none_or(|roots| {
                let mut parent = window.dom().parent(*node);
                while let Some(id) = parent {
                    if roots.contains(&id) {
                        return true;
                    }
                    parent = window.dom().parent(id);
                }
                false
            }) && window.dom().text_content(*node).contains(label)
                && zgui::view::ViewHost::window_box(window.host().as_ref(), *node).is_some()
        })
        .unwrap_or_else(|| panic!("Missing {class}: {label}"));
    let bounds = zgui::view::ViewHost::window_box(window.host().as_ref(), node).unwrap();
    for action in [PointerAction::Pressed, PointerAction::Released] {
        let mut event = PointerEvent::mouse(Point::new(
            CssPx(bounds.origin.x.0 + bounds.size.width.0 / 2.),
            CssPx(bounds.origin.y.0 + bounds.size.height.0 / 2.),
        ));
        event.button = Some(PointerButton::Primary);
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
    }
    harness.settle(128);
}
fn settle_until(
    harness: &mut Harness<zgui::runtime::Runtime>,
    until: impl Fn(&Harness<zgui::runtime::Runtime>) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !until(harness) {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        assert!(Instant::now() < deadline, "Map controls did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn key(harness: &mut Harness<zgui::runtime::Runtime>, event: zgui::vocab::KeyEvent) {
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Pressed,
        event,
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(128);
}
fn replace(harness: &mut Harness<zgui::runtime::Runtime>, class: &str, text: &str) {
    click(harness, class, "");
    for (text, modifiers) in [("a", Modifiers::PRIMARY), (text, Modifiers::NONE)] {
        harness.deliver_to_first(SurfaceEvent::Key {
            state: zgui::vocab::KeyState::Pressed,
            event: zgui::vocab::KeyEvent::character(text),
            modifiers,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(64);
    }
}

#[test]
fn map_trace_cards_use_demo_slots_axes_start_and_color_controls() {
    use crate::trajectory_cards::TrajectorySetupProps;
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler=app().with_size(350.,900.).with_stylesheet(super::preview::stylesheet())
        .with_renderer(Box::new(|_,target|{let mut renderer=zgui_testkit_scene::CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
        .into_handler(move ||{
            let workbench=Workbench::new();
            let builder=workbench.trajectory_builder();
            let (lines,spec,_)=super::telemetry::tests::fixture();
            builder.open.set(true);builder.pane.lines.set(lines);builder.pane.trajectory.set(Some(spec));
            *saved.borrow_mut()=Some((workbench,builder.pane));
            view!{AppTheme(palette=crate::theme::Palette::Forest){column(class="page workbench"){TrajectorySetup(workbench=workbench,pane=builder.pane)}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(350.),
        DevicePx(900.),
    )));
    harness.settle(128);
    let (workbench, pane) = captured.borrow().unwrap();
    let slot_box = |_h: &Harness<zgui::runtime::Runtime>, y| {
        let slot = pane.trajectory_slots.with_value(|slots| slots[&(0, y)]);
        let b = slot.window_bounds().unwrap();
        (b.origin.x.0, b.origin.y.0, b.size.width.0)
    };
    let east = slot_box(&harness, false);
    let north = slot_box(&harness, true);
    assert_eq!(east.0, north.0);
    assert!(east.1 < north.1);
    assert!(east.2 > 200.);
    click(&mut harness, "trace-axes-ned", "NED");
    assert!(
        pane.trajectory
            .get_untracked()
            .unwrap()
            .local
            .unwrap()
            .north_first
    );
    assert!(slot_box(&harness, true).1 < slot_box(&harness, false).1);
    assert_eq!(pane.trajectory.get_untracked().unwrap().x, "x");
    assert_eq!(pane.trajectory.get_untracked().unwrap().y, "y");
    click(&mut harness, "trace-axes-enu", "ENU");
    assert!(slot_box(&harness, false).1 < slot_box(&harness, true).1);
    click(&mut harness, "start-coordinates", "Lat, lon");
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("start-lat-lon")
            .len(),
        1
    );
    click(&mut harness, "start-default", "Default");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("start-lat-lon")
            .is_empty()
    );
    click(&mut harness, "map-trace-color-mode", "Speed");
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].mode,
        crate::trace_color::Mode::Speed
    );
    click(&mut harness, "map-trace-color-mode", "Field");
    assert!(
        harness.app().windows()[0]
            .box_with_class("color-field-slot")
            .is_some()
    );
    pane.draft_trace_colors.update(|colors| {
        colors.insert(
            "y2".into(),
            crate::trace_color::TraceColor {
                mode: crate::trace_color::Mode::Field,
                ..Default::default()
            },
        );
    });
    pane.trajectory.update(|spec| {
        spec.as_mut().unwrap().extra.push(crate::trajectory::Pair {
            x: "x2".into(),
            y: "y2".into(),
            local: Some(Default::default()),
            gps_scale: None,
            fix: String::new(),
        })
    });
    harness.settle(128);
    let first = workbench
        .trajectory_builder()
        .color_slots
        .with_value(|slots| slots[&0]);
    let bounds = first.window_bounds().unwrap();
    assert!(
        workbench.builder_color_slot_at(
            (bounds.origin.x.0 + bounds.size.width.0 / 2.) / first.scale(),
            (bounds.origin.y.0 + bounds.size.height.0 / 2.) / first.scale()
        ),
        "First color field must remain a drop target when another Field card opens"
    );
    let columns = pane.lines.get_untracked();
    pane.lines.set(Vec::new());
    harness.settle(128);
    click(&mut harness, "map-trace-color-mode", "Speed");
    assert_eq!(
        pane.draft_trace_colors.get_untracked()["y"].mode,
        crate::trace_color::Mode::Speed,
        "Colors must be editable before field data loads"
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("trace-card-detail")
            .iter()
            .any(|node| harness.app().windows()[0]
                .dom()
                .text_content(*node)
                .contains("· speed")),
        "Card summary must show the draft color mode"
    );
    let destination = workbench.pane();
    workbench.drop_trajectory_pair(0, destination.id);
    assert_eq!(
        destination.draft_trace_colors.get_untracked()["y"].mode,
        crate::trace_color::Mode::Speed
    );
    pane.lines.set(columns);
    let destination = workbench.add_plot(destination.id, false, false);
    workbench.drop_trajectory_pair(0, destination.id);
    let id = destination
        .lines
        .with_untracked(|lines| lines.iter().find(|line| line.source == "y").unwrap().id);
    assert_eq!(
        destination.trace_colors.get_untracked()[&id].mode,
        crate::trace_color::Mode::Speed,
        "Dropping a draft must carry its color mode onto the new trace"
    );
    let spec = pane.trajectory.get_untracked().unwrap();
    assert_eq!(
        serde_json::from_str::<crate::trajectory::Spec>(&serde_json::to_string(&spec).unwrap())
            .unwrap(),
        spec
    );
}

#[test]
fn spatial_annotations_follow_orbit_and_hide_unprojectable_ends() {
    use super::{
        measure::{Mark, MeasureViewProps},
        modes::ModeMarkersProps,
        terrain::{Orbit, Scene},
    };
    use std::sync::Arc;
    let preview = std::env::var_os("ENLUMI_3D_ANNOTATIONS_PREVIEW");
    let render_preview = preview.is_some();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler = app().with_size(800., 600.).with_stylesheet(super::preview::stylesheet())
        .with_renderer(Box::new(move |_, target| {
            if render_preview {
                let graphics = super::test_renderer::graphics().expect("Preview needs a graphics device");
                let mut renderer = graphics.renderer_offscreen(target, zgui::surface::wgpu::TextureFormat::Bgra8Unorm, false)?;
                zgui_render_vector_vello::attach(&mut renderer, target.size);
                return Ok(Box::new(super::test_renderer::ReadbackRenderer(renderer)));
            }
            let mut renderer = zgui_testkit_scene::CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .into_handler(move || {
            let workbench = Workbench::new();
            let pane = workbench.pane();
            let (lines, spec, tracks) = super::telemetry::tests::fixture();
            pane.lines.set(lines);
            pane.trajectory.set(Some(spec));
            pane.trajectory_data.set(Some(tracks));
            let mut band = crate::geometry::Geometry::new(99, crate::geometry::GeometryKind::Band, kurbo::Point::ZERO);
            band.states = Some(crate::state_tint::tests::fixture().states);
            workbench.geometry.items.update(|items|items.push(band));
            workbench.attach_geometry(99, pane, true);
            let mut frame = Scene::default();
            frame.paths = Arc::from([super::flight3::Path3 {
                id: 2, origin: [0., 0.], points: Arc::from([[0., 0., 100.], [100., 0., 150.]]),
                times: Arc::from([0., 1.]), color: [255; 4], width: 2., drawing: Default::default(),
                radius: 3., marker: plot::plot_data::MarkerShape::Circle, local: false, arrows: false, arrow_color: crate::trajectory::arrow_color(), modes: Arc::from([]),
            }]);
            let scene = RwSignal::new_local(Some(Arc::new(frame)));
            let camera = RwSignal::new_local(Camera {center: [0., 0.], zoom: 14.});
            let orbit = RwSignal::new_local(Orbit::default());
            let spatial = RwSignal::new_local(true);
            let terrain = Memo::new(move |_|spatial.get());
            let size = Memo::new(move |_|[800., 600.]);
            let measuring = RwSignal::new_local(true);
            let ghost = RwSignal::new_local(None::<Mark>);
            let marks = RwSignal::new_local(vec![
                Mark {point: [0., 0.], sample: Some((2, 0.)), elevation: Some(100.)},
                Mark {point: [100., 0.], sample: Some((2, 1.)), elevation: Some(150.)},
            ]);
            *saved.borrow_mut() = Some((pane, orbit, spatial, measuring, marks, scene, ghost));
            view! {AppTheme(palette=crate::theme::Palette::Forest) {
                box(style:position="relative", style:width="800px", style:height="600px") {
                    ModeMarkers(workbench=workbench,pane=pane,camera=camera,size=size,terrain=terrain,orbit=orbit,scene=scene)
                    MeasureView(pane=pane,marks=marks,measuring=measuring,camera=camera,size=size,terrain=terrain,orbit=orbit,scene=scene,preview=Some(Signal::derive_local(move ||ghost.get())))
                }
            }}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(800.),
        DevicePx(600.),
    )));
    harness.settle(128);
    let (pane, orbit, spatial, measuring, marks, scene, ghost) = captured.borrow().unwrap();
    let count = |h: &Harness<zgui::runtime::Runtime>, class: &str| {
        h.app().windows()[0].nodes_with_class(class).len()
    };
    assert_eq!(count(&harness, "map-measure-line"), 1);
    assert_eq!(count(&harness, "map-measure-end"), 2);
    assert_eq!(count(&harness, "map-mode-label"), 2);
    assert_eq!(count(&harness, "map-measure-banner"), 1);
    if let Some(path) = preview {
        let pixels = harness.app().windows()[0]
            .renderer()
            .as_any()
            .unwrap()
            .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
            .unwrap()
            .read_composed();
        super::tests::capture(&pixels, path);
    }
    let end_positions = |h: &Harness<zgui::runtime::Runtime>| {
        let window = &h.app().windows()[0];
        window
            .nodes_with_class("map-measure-end")
            .iter()
            .map(|node| {
                zgui::view::ViewHost::window_box(window.host().as_ref(), *node)
                    .unwrap()
                    .origin
            })
            .collect::<Vec<_>>()
    };
    let before = end_positions(&harness);
    let readout = harness.app().windows()[0]
        .box_with_class("map-measure-readout")
        .unwrap();
    assert!(
        (f64::from(readout.origin.x.0) - (f64::from(before[0].x.0 + before[1].x.0) / 2. + 14.))
            .abs()
            < 1.
    );
    assert!(
        (f64::from(readout.origin.y.0) - (f64::from(before[0].y.0 + before[1].y.0) / 2. - 4.))
            .abs()
            < 1.
    );
    let second = marks.get_untracked()[1];
    marks.update(|marks| {
        marks.pop();
    });
    harness.settle(128);
    assert_eq!(count(&harness, "map-measure-line"), 0);
    assert_eq!(count(&harness, "map-measure-readout"), 0);
    ghost.set(Some(second));
    harness.settle(128);
    assert_eq!(count(&harness, "map-measure-line"), 1);
    assert_eq!(count(&harness, "map-measure-end"), 2);
    assert_eq!(
        marks.with_untracked(Vec::len),
        1,
        "Preview must not commit second end"
    );
    marks.update(|marks| marks.push(second));
    ghost.set(None);
    orbit.update(|orbit| {
        orbit.bearing = 90.;
        orbit.exaggeration = 2.;
    });
    harness.settle(128);
    assert_ne!(before, end_positions(&harness));
    let frame = scene.get_untracked().unwrap();
    let expected = super::hover::Projection::new(
        &frame,
        Camera {
            center: [0., 0.],
            zoom: 14.,
        },
        orbit.get_untracked(),
        [800., 600.],
    )
    .point([100., 0., 150.], 0.)
    .unwrap()
    .0;
    let actual = end_positions(&harness)[1];
    assert!((f64::from(actual.x.0) + 4. - expected[0]).abs() < 1.);
    assert!((f64::from(actual.y.0) + 4. - expected[1]).abs() < 1.);
    marks.update(|marks| marks[0].elevation = Some(f64::NAN));
    harness.settle(128);
    assert_eq!(count(&harness, "map-measure-line"), 0);
    assert_eq!(count(&harness, "map-measure-end"), 1);
    scene.set(None);
    harness.settle(128);
    assert_eq!(count(&harness, "map-mode-label"), 0);
    assert_eq!(count(&harness, "map-measure-end"), 0);
    spatial.set(false);
    harness.settle(128);
    assert_eq!(count(&harness, "map-mode-label"), 2);
    assert_eq!(count(&harness, "map-measure-line"), 1);
    pane.plot
        .hidden
        .set(std::collections::BTreeSet::from(["2".into()]));
    measuring.set(false);
    harness.settle(128);
    assert_eq!(count(&harness, "map-mode-label"), 0);
    assert_eq!(count(&harness, "map-measure"), 0);
}

#[test]
fn toolbar_follow_camera_memory_and_color_picker_work_in_map() {
    let directory = tempfile::tempdir().unwrap();
    let local = directory.path().to_path_buf();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let probe = OverlayProbe(Rc::new(RefCell::new(None)));
    let observed = probe.clone();
    let spatial = Camera {
        center: [300., 400.],
        zoom: 9.,
    };
    let flat = Camera {
        center: [0., 0.],
        zoom: 14.,
    };
    let handler=app().with_size(1050.,700.).with_stylesheet(super::preview::stylesheet())
        .with_renderer(Box::new(|_,target|{let mut renderer=zgui_testkit_scene::CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
        .into_handler(move || {
            provide_local_context(probe.clone());
            let workbench=Workbench::new();let pane=workbench.pane();
            let (lines,mut spec,tracks)=super::telemetry::tests::fixture();
            spec.map=Some(Config{camera:Some(flat),camera_3d:Some(spatial),..Default::default()});
            pane.lines.set(lines);pane.selected.set(Some(2));pane.trajectory_data.set(Some(tracks));pane.trajectory.set(Some(spec));
            let tint=crate::state_tint::tests::fixture();
            let mut band=crate::geometry::Geometry::new(99,crate::geometry::GeometryKind::Band,kurbo::Point::ZERO);
            band.name="Flight mode".into();band.states=Some(tint.states);band.opacity=tint.opacity;
            workbench.geometry.items.update(|items|items.push(band));workbench.attach_geometry(99,pane,true);
            workbench.profiles.metadata.update(|all|{all.insert("battery".into(),crate::profile::Metadata{name:"voltage".into(),unit:"V".into(),axis:String::new()});});
            workbench.map_tiles.set_value(Some(super::tiles::Service::new(super::tiles::Config{cache:local.join("cache"),local:Some(local.clone()),network:false})));
            *saved.borrow_mut()=Some((workbench,pane));
            view!{AppTheme(palette=crate::theme::Palette::Forest){column(class="page workbench",on:key_down:capture=move |event|if workbench.measurement_shortcut(event,event.modifiers){event.prevent_default();event.stop_propagation();}){FlightMap(workbench=workbench,pane=pane,visible=Signal::stored_local(true))}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1050.),
        DevicePx(700.),
    )));
    harness.settle(128);
    let (workbench, pane) = captured.borrow().unwrap();
    let map = || pane.trajectory.get_untracked().unwrap().map.unwrap();
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-toolbar")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-backend-trigger")
            .is_empty()
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-toolbar-settings")
            .is_empty()
    );
    let map_box = harness.app().windows()[0]
        .box_with_class("flight-map")
        .unwrap();
    let viewport = harness.app().windows()[0]
        .box_with_class("flight-map-view")
        .unwrap();
    let profile = harness.app().windows()[0]
        .box_with_class("map-profile")
        .map_or(0., |b| b.size.height.0);
    assert!(
        (map_box.size.height.0 - viewport.size.height.0 - profile).abs() < 2.,
        "Toolbar must not consume map height; only the profile strip may"
    );
    let expanded = harness.app().windows()[0]
        .box_with_class("map-legend")
        .unwrap();
    click(&mut harness, "legend-collapse", "Legend");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-trace-summary")
            .is_empty()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-legend")
            .unwrap()
            .size
            .height
            .0
            < expanded.size.height.0
    );
    click(&mut harness, "plot-settings-trigger", "");
    assert!(
        harness.app().windows()[0]
            .box_with_class("line-width")
            .is_some()
    );
    click(&mut harness, "line-pattern", "Dashed");
    assert_eq!(
        pane.drawing.get_untracked()[&2].pattern,
        plot::line_draw::LinePattern::Dashed
    );
    let width = pane.plot.style("2").width;
    click(&mut harness, "line-width", "");
    assert_ne!(pane.plot.style("2").width, width);
    click(&mut harness, "line-pattern", "Solid");
    click(&mut harness, "map-arrows-editor", "Direction arrows");
    assert!(!pane.trajectory.get_untracked().unwrap().arrows);
    click(&mut harness, "map-arrows-editor", "Direction arrows");
    click(&mut harness, "map-arrow-color", "Arrow color");
    replace(&mut harness, "map-arrow-color-popup color-hex", "#12ABCD");
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].arrow_color,
        Some([18, 171, 205, 255])
    );
    key(
        &mut harness,
        zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Escape,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("plot-settings-surface")
            .is_some()
    );
    click(&mut harness, "settings-done", "Done");
    click(&mut harness, "legend-collapse", "Legend");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-place-start")
            .is_empty()
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-trace-summary")
            .len()
            == 1
    );
    click(&mut harness, "map-layers", "");
    assert!(pane.trajectory.get_untracked().unwrap().arrows);
    click(&mut harness, "map-arrows", "Direction arrows");
    assert!(!pane.trajectory.get_untracked().unwrap().arrows);
    click(&mut harness, "map-arrows", "Direction arrows");
    assert!(pane.trajectory.get_untracked().unwrap().arrows);
    click(&mut harness, "map-layer-option", "None (grid)");
    assert!(!map().basemap);
    click(&mut harness, "map-layer-option", "Esri World Imagery");
    assert!(map().basemap);
    assert_eq!(map().provider, super::tiles::Provider::EsriImagery);
    click(&mut harness, "map-layers", "");
    workbench.timeline.time.set(Some(1.5));
    click(&mut harness, "map-follow", "");
    assert!(map().follow);
    assert!((map().camera.unwrap().center[0] - 20.).abs() < 1e-6);
    assert_eq!(map().camera.unwrap().zoom, 14.);
    workbench.timeline.time.set(Some(2.));
    harness.settle(128);
    assert!((map().camera.unwrap().center[0] - 30.).abs() < 1e-6);
    let region = harness.app().windows()[0]
        .box_with_class("map-viewport")
        .unwrap();
    for (action, dx) in [
        (PointerAction::Pressed, 0.),
        (PointerAction::Moved, 20.),
        (PointerAction::Released, 20.),
    ] {
        let mut event = PointerEvent::mouse(Point::new(
            CssPx(region.origin.x.0 + region.size.width.0 / 2. + dx),
            CssPx(region.origin.y.0 + region.size.height.0 / 2.),
        ));
        event.button = matches!(action, PointerAction::Pressed | PointerAction::Released)
            .then_some(PointerButton::Primary);
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert!(!map().follow);
    let remembered = map().camera.unwrap();
    click(&mut harness, "map-mode-3d", "3D");
    assert_eq!(map().camera, Some(spatial));
    key(&mut harness, zgui::vocab::KeyEvent::character("m"));
    assert!(pane.map_measuring.get_untracked());
    assert!(!workbench.measurement.active.get_untracked());
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-measure-banner")
            .is_some()
    );
    key(&mut harness, zgui::vocab::KeyEvent::character("m"));
    assert!(!pane.map_measuring.get_untracked());
    key(&mut harness, zgui::vocab::KeyEvent::character("m"));
    key(
        &mut harness,
        zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Escape,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
    );
    assert!(!pane.map_measuring.get_untracked());

    super::settings::update(workbench, pane, |s| {
        s.map.as_mut().unwrap().orbit.bearing = 72.
    });
    harness.settle(128);
    click(&mut harness, "map-compass", "");
    assert_eq!(map().orbit.bearing, 0.);
    click(&mut harness, "map-fit", "");
    settle_until(&mut harness, |_| {
        map().camera.is_some_and(|camera| camera != spatial)
    });
    let fitted_3d = map().camera;
    assert!(!map().follow);
    assert_eq!(map().orbit.bearing, 0.);
    click(&mut harness, "map-layers", "");
    click(&mut harness, "map-ground-projection", "Ground path");
    assert!(map().ground_projection);
    click(&mut harness, "map-layers", "");
    click(&mut harness, "map-mode-2d", "Map");
    assert_eq!(map().camera, Some(remembered));
    assert_eq!(map().camera_3d, fitted_3d);
    click(&mut harness, "map-trace-summary", "y");
    click(&mut harness, "map-trace-color-mode", "Speed");
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].mode,
        crate::trace_color::Mode::Speed
    );
    replace(&mut harness, "map-trace-color-low", "5");
    replace(&mut harness, "map-trace-color-high", "50");
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Pressed,
        event: zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Enter,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(128);
    assert_eq!(pane.trace_colors.get_untracked()[&2].speed.range, [5., 50.]);
    click(&mut harness, "map-trace-color-mode", "Altitude");
    settle_until(&mut harness, |h| {
        h.app().windows()[0]
            .nodes_with_class("map-color-label")
            .iter()
            .any(|node| {
                h.app().windows()[0]
                    .dom()
                    .text_content(*node)
                    .contains("height (m)")
            })
    });
    click(&mut harness, "map-trace-color-auto", "Auto");
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].height.range,
        [100., 150.]
    );
    assert_eq!(pane.trace_colors.get_untracked()[&2].speed.range, [5., 50.]);
    click(&mut harness, "map-trace-color-mode", "Solid");
    click(&mut harness, "map-trace-summary", "y");
    pane.fit_axes(plot::interaction::Axes::BOTH);
    settle_until(&mut harness, |_| {
        observed.0.borrow().as_ref().is_some_and(|probe| {
            probe
                .paths
                .with_untracked(|paths| paths.values().any(|path| path.arrow_count() > 0))
        })
    });
    assert!(
        observed
            .0
            .borrow()
            .as_ref()
            .unwrap()
            .paths
            .with_untracked(|paths| paths
                .values()
                .all(|path| path.arrow_color() == [18, 171, 205, 255]))
    );
    click(&mut harness, "map-layers", "");
    click(&mut harness, "map-arrows", "Direction arrows");
    settle_until(&mut harness, |_| {
        observed.0.borrow().as_ref().is_some_and(|probe| {
            probe.paths.with_untracked(|paths| {
                !paths.is_empty() && paths.values().all(|path| path.arrow_count() == 0)
            })
        })
    });
    click(&mut harness, "map-arrows", "Direction arrows");
    settle_until(&mut harness, |_| {
        observed.0.borrow().as_ref().is_some_and(|probe| {
            probe
                .paths
                .with_untracked(|paths| paths.values().any(|path| path.arrow_count() > 0))
        })
    });
    click(&mut harness, "map-layers", "");
    click(&mut harness, "map-trace-visibility", "");
    assert!(pane.plot.hidden.get_untracked().contains("2"));
    click(&mut harness, "map-trace-visibility", "");
    assert!(!pane.plot.hidden.get_untracked().contains("2"));

    // A second line gets independent arrow color and state-marker visibility.
    let mut second = pane
        .lines
        .with_untracked(|lines| lines.iter().find(|line| line.id == 2).unwrap().clone());
    second.id = 5;
    second.source = "y2".into();
    second.name = "Second trace".into();
    pane.lines.update(|lines| lines.push(second));
    pane.trajectory.update(|spec| {
        let spec = spec.as_mut().unwrap();
        spec.extra.push(crate::trajectory::Pair {
            x: spec.x.clone(),
            y: "y2".into(),
            local: spec.local,
            gps_scale: spec.gps_scale,
            fix: spec.fix.clone(),
        });
    });
    pane.trajectory_data.set(Some(std::sync::Arc::new(
        crate::trajectory::Tracks::prepare(
            &pane.lines.get_untracked(),
            &pane.trajectory.get_untracked().unwrap(),
        )
        .unwrap(),
    )));
    harness.settle(128);
    click(&mut harness, "map-trace-summary", "Second trace");
    click(&mut harness, "map-arrow-color", "Arrow color");
    replace(&mut harness, "map-arrow-color-popup color-hex", "#EE3344");
    assert_eq!(
        pane.trace_colors.get_untracked()[&5].arrow_color,
        Some([238, 51, 68, 255])
    );
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].arrow_color,
        Some([18, 171, 205, 255])
    );
    key(
        &mut harness,
        zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Escape,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
    );
    settle_until(&mut harness, |_| {
        observed.0.borrow().as_ref().is_some_and(|probe| {
            probe.paths.with_untracked(|paths| {
                paths
                    .get(&5)
                    .is_some_and(|path| path.arrow_color() == [238, 51, 68, 255])
                    && paths
                        .get(&2)
                        .is_some_and(|path| path.arrow_color() == [18, 171, 205, 255])
            })
        })
    });
    let markers = harness.app().windows()[0]
        .nodes_with_class("map-mode-marker")
        .len();
    assert!(markers >= 4);
    click(&mut harness, "map-trace-states", "State markers");
    assert!(!pane.trace_colors.get_untracked()[&5].state_markers);
    assert!(pane.trace_colors.get_untracked()[&2].state_markers);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("map-mode-marker")
            .len()
            * 2,
        markers
    );
    let colors = pane.trace_colors.get_untracked();
    assert_eq!(
        serde_json::from_str::<std::collections::BTreeMap<u64, crate::trace_color::TraceColor>>(
            &serde_json::to_string(&colors).unwrap()
        )
        .unwrap(),
        colors
    );
    workbench.edit_trajectory(pane);
    click(&mut harness, "map-legend-trace map-trace-remove", "");
    assert_eq!(pane.view_lines.get_untracked().len(), 1);
    assert!(!pane.trace_colors.get_untracked().contains_key(&2));
    assert_eq!(pane.view_lines.get_untracked()[0].id, 5);
    assert_eq!(
        workbench
            .trajectory_builder()
            .pane
            .view_lines
            .get_untracked()
            .len(),
        1
    );
    click(&mut harness, "map-trace-remove", "");
    assert!(pane.view_lines.get_untracked().is_empty());
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-trace-summary")
            .is_empty()
    );
    let spec = pane.trajectory.get_untracked().unwrap();
    let restored: crate::trajectory::Spec =
        serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(restored, spec);
}

/// Renders the map controls, profile, state markers and a measurement.
/// Run with `ENLUMI_MAP_CONTROLS_PREVIEW=path.ppm cargo test -- --ignored map_controls_preview`.
#[test]
#[ignore = "visual preview; needs a graphics device"]
fn map_controls_preview() {
    let Some(path) = std::env::var_os("ENLUMI_MAP_CONTROLS_PREVIEW") else {
        return;
    };
    let Some(graphics) = super::test_renderer::graphics() else {
        return;
    };
    let palette = std::env::var("ENLUMI_MAP_PREVIEW_THEME")
        .ok()
        .and_then(|id| crate::theme::Palette::from_id(&id))
        .unwrap_or(crate::theme::Palette::Forest);
    let directory = tempfile::tempdir().unwrap();
    let local = directory.path().to_path_buf();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler=app().with_size(1400.,760.).with_stylesheet(super::preview::stylesheet())
        .with_renderer(Box::new(move |_,target|{
            let mut renderer=graphics.renderer_offscreen(target,zgui::surface::wgpu::TextureFormat::Bgra8Unorm,false)?;
            zgui_render_vector_vello::attach(&mut renderer,target.size);Ok(Box::new(super::test_renderer::ReadbackRenderer(renderer)))
        }))
        .into_handler(move || {
            let workbench=Workbench::new();let pane=workbench.pane();
            let (lines,mut spec,tracks)=super::telemetry::tests::fixture();
            spec.map=Some(Config{trail:super::flight::Trail::Flown,..Default::default()});
            pane.lines.set(lines);pane.trajectory_data.set(Some(tracks));pane.trajectory.set(Some(spec));
            let tint=crate::state_tint::tests::fixture();
            let mut band=crate::geometry::Geometry::new(99,crate::geometry::GeometryKind::Band,kurbo::Point::ZERO);
            band.name="Flight mode".into();band.states=Some(tint.states);band.opacity=tint.opacity;
            workbench.geometry.items.update(|items|items.push(band));workbench.attach_geometry(99,pane,true);
            workbench.timeline.time.set(Some(2.5));
            workbench.map_tiles.set_value(Some(super::tiles::Service::new(super::tiles::Config{cache:local.join("cache"),local:Some(local.clone()),network:false})));
            workbench.edit_trajectory(pane);
            *saved.borrow_mut()=Some((workbench,pane));
            use crate::trajectory_cards::TrajectorySetupProps;
            let builder=workbench.trajectory_builder().pane;
            view!{AppTheme(palette=palette){row(class="page workbench",style:width="1400px",style:height="760px"){column(style:width="1050px",style:height="760px"){FlightMap(workbench=workbench,pane=pane,visible=Signal::stored_local(true))} column(style:width="350px",style:height="760px"){TrajectorySetup(workbench=workbench,pane=builder)}}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1400.),
        DevicePx(760.),
    )));
    for _ in 0..20 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    click(&mut harness, "map-measure-tool", "");
    let view = harness.app().windows()[0]
        .box_with_class("flight-map-view")
        .unwrap();
    for x in [0.3, 0.7] {
        let mut event = PointerEvent::mouse(Point::new(
            CssPx(view.origin.x.0 + view.size.width.0 * x),
            CssPx(view.origin.y.0 + view.size.height.0 * 0.35),
        ));
        event.button = Some(PointerButton::Primary);
        for action in [PointerAction::Pressed, PointerAction::Released] {
            harness.deliver_to_first(SurfaceEvent::Pointer {
                action,
                event,
                modifiers: Modifiers::NONE,
                timestamp: Timestamp::ORIGIN,
            });
        }
        harness.settle(64);
    }
    for _ in 0..10 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    let pixels = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .read_composed();
    super::tests::capture(&pixels, &path);
    click(&mut harness, "map-layers", "");
    for _ in 0..4 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    let pixels = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .read_composed();
    let mut layers = std::path::PathBuf::from(&path);
    layers.set_extension("layers.ppm");
    super::tests::capture(&pixels, layers);
    click(&mut harness, "map-layers", "");
    click(&mut harness, "map-measure-tool", "");
    click(&mut harness, "plot-settings-trigger", "");
    for _ in 0..4 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    let pixels = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .read_composed();
    let mut options = std::path::PathBuf::from(&path);
    options.set_extension("options.ppm");
    super::tests::capture(&pixels, options);
}

#[test]
fn map_controls_follow_live_theme_corner_changes() {
    use crate::{theme::Palette, trajectory_cards::TrajectorySetupProps};
    let directory = tempfile::tempdir().unwrap();
    let local = directory.path().to_path_buf();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler=app().with_size(1400.,760.).with_stylesheet(super::preview::stylesheet())
        .with_renderer(Box::new(|_,target|{let mut renderer=zgui_testkit_scene::CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
        .into_handler(move || {
            let palette=RwSignal::new_local(Palette::Slate);let workbench=Workbench::new();let pane=workbench.pane();
            let (lines,mut spec,tracks)=super::telemetry::tests::fixture();spec.map=Some(Config{offline:true,..Default::default()});
            pane.lines.set(lines);pane.trajectory_data.set(Some(tracks));pane.trajectory.set(Some(spec));
            workbench.map_tiles.set_value(Some(super::tiles::Service::new(super::tiles::Config{cache:local.join("cache"),local:Some(local.clone()),network:false})));
            workbench.edit_trajectory(pane);let builder=workbench.trajectory_builder().pane;
            *saved.borrow_mut()=Some(palette);
            view!{AppTheme(palette=Signal::derive_local(move ||palette.get())){row(class="page workbench",style:width="1400px",style:height="760px"){column(style:width="1050px",style:height="760px"){FlightMap(workbench=workbench,pane=pane,visible=Signal::stored_local(true))}column(style:width="350px",style:height="760px"){TrajectorySetup(workbench=workbench,pane=builder)}}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1400.),
        DevicePx(760.),
    )));
    harness.settle(128);
    let palette = captured.borrow().unwrap();
    click(&mut harness, "map-layers", "");
    for (choice, radius, small) in [
        (Palette::Slate, 0., 0.),
        (Palette::Amethyst, 10., 6.),
        (Palette::Forest, 4., 3.),
    ] {
        palette.set(choice);
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(1401.),
            DevicePx(760.),
        )));
        harness.settle(128);
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(1400.),
            DevicePx(760.),
        )));
        // Retained frames contain paint deltas; keep every delta until layout settles.
        let mut transcript = String::new();
        for _ in 0..128 {
            if harness.pump() == 0 {
                break;
            }
            let window = &harness.app().windows()[0];
            let capture = window
                .renderer()
                .as_any()
                .unwrap()
                .downcast_ref::<zgui_testkit_scene::CaptureRenderer>()
                .unwrap();
            if let Some(frame) = capture.transcript() {
                transcript.push_str(frame.as_str());
            }
        }
        harness.settle(128);
        for (class, expected) in [
            ("map-legend", radius),
            ("map-toolbar", radius),
            ("map-nav", radius),
            ("map-layers-menu", radius),
            ("trace-card-map", radius),
            ("trajectory-axis-slot", small),
            ("zui-checkbox", small),
        ] {
            let window = &harness.app().windows()[0];
            let bounds = window.box_with_class(class).unwrap();
            let quad = transcript
                .lines()
                .rev()
                .find(|line| {
                    line.trim_start().starts_with("quad ")
                        && line.split_once("bounds=rect(").is_some_and(|(_, tail)| {
                            let values = tail
                                .split(')')
                                .next()
                                .unwrap()
                                .split(',')
                                .map(|v| v.trim().parse::<f32>().unwrap())
                                .collect::<Vec<_>>();
                            values
                                .iter()
                                .zip([
                                    bounds.origin.x.0,
                                    bounds.origin.y.0,
                                    bounds.size.width.0,
                                    bounds.size.height.0,
                                ])
                                .all(|(a, b)| (a - b).abs() < 0.1)
                        })
                })
                .unwrap_or_else(|| panic!("Missing painted {class}: {bounds:?}\n{transcript}"));
            let radii = quad
                .split_once("radii=[")
                .map(|(_, tail)| {
                    tail.split(']')
                        .next()
                        .unwrap()
                        .split(',')
                        .map(|v| v.trim().parse::<f32>().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![0.; 8]);
            assert!(
                radii.iter().all(|r| (r - expected).abs() < 0.1),
                "{} {class}: {radii:?}, expected {expected}; {quad}",
                choice.name()
            );
        }
    }
}
