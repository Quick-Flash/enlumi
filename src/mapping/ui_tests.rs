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
    let node = window
        .nodes_with_class(class)
        .into_iter()
        .find(|node| {
            window.dom().text_content(*node).contains(label)
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
            view!{AppTheme(palette=crate::theme::Palette::Forest){column(class="page workbench"){FlightMap(workbench=workbench,pane=pane,visible=Signal::stored_local(true))}}}
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
    assert_eq!(
        map_box.size.height, viewport.size.height,
        "Toolbar must not consume map height"
    );
    let gear = harness.app().windows()[0]
        .box_with_class("plot-settings-trigger")
        .unwrap();
    let icon = harness.app().windows()[0]
        .box_with_class("plot-settings-icon")
        .unwrap();
    assert_eq!(
        (icon.size.width.0, icon.size.height.0),
        (18., 18.),
        "Settings icon must render before opening editor"
    );
    let legend = harness.app().windows()[0]
        .box_with_class("trajectory-legend")
        .unwrap();
    assert!(
        gear.origin.x.0 >= legend.origin.x.0
            && gear.origin.x.0 + gear.size.width.0 <= legend.origin.x.0 + legend.size.width.0,
        "Settings gear must stay inside legend"
    );
    click(&mut harness, "plot-settings-trigger", "");
    click(&mut harness, "map-backend-trigger", "OpenStreetMap");
    click(&mut harness, "zui-select__item", "Esri World Imagery");
    assert_eq!(map().provider, super::tiles::Provider::EsriImagery);
    click(&mut harness, "settings-done", "Done");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("plot-settings-surface")
            .is_empty(),
        "Done must close legend settings"
    );
    workbench.timeline.time.set(Some(1.5));
    click(&mut harness, "map-follow", "Follow");
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
    super::settings::update(workbench, pane, |s| {
        s.map.as_mut().unwrap().orbit.bearing = 72.
    });
    harness.settle(128);
    click(&mut harness, "map-compass", "");
    assert_eq!(map().orbit.bearing, 0.);
    click(&mut harness, "map-fit", "Fit");
    settle_until(&mut harness, |_| {
        map().camera.is_some_and(|camera| camera != spatial)
    });
    let fitted_3d = map().camera;
    assert!(!map().follow);
    assert_eq!(map().orbit.bearing, 0.);
    click(&mut harness, "plot-settings-trigger", "");
    click(&mut harness, "map-ground-projection", "Ground path");
    assert!(map().ground_projection);
    click(&mut harness, "settings-done", "Done");
    click(&mut harness, "map-mode-2d", "2D");
    assert_eq!(map().camera, Some(remembered));
    assert_eq!(map().camera_3d, fitted_3d);
    click(&mut harness, "plot-settings-trigger", "");
    click(&mut harness, "map-color-mode", "Solid color");
    click(&mut harness, "zui-select__item", "Speed gradient");
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].mode,
        crate::trace_color::Mode::Speed
    );
    settle_until(&mut harness, |h| {
        h.app().windows()[0]
            .nodes_with_class("map-color-label")
            .iter()
            .any(|node| {
                h.app().windows()[0]
                    .dom()
                    .text_content(*node)
                    .contains("speed (m/s)")
            })
    });
    settle_until(&mut harness, |_| {
        observed.0.borrow().as_ref().is_some_and(|probe| {
            let scene = zgui::canvas::resolve(probe.canvas.scene().token()).unwrap();
            scene.lock().unwrap().series().len() > 1
        })
    });
    replace(&mut harness, "trace-gradient-min", "5");
    replace(&mut harness, "trace-gradient-max", "50");
    click(&mut harness, "trace-gradient-apply", "Apply limits");
    assert_eq!(pane.trace_colors.get_untracked()[&2].speed.range, [5., 50.]);
    replace(&mut harness, "trace-gradient-max", "3");
    click(&mut harness, "trace-gradient-apply", "Apply limits");
    assert_eq!(pane.trace_colors.get_untracked()[&2].speed.range, [5., 50.]);
    assert!(
        harness.app().windows()[0]
            .box_with_class("trace-gradient-error")
            .is_some()
    );
    replace(&mut harness, "trace-gradient-max", "50");
    click(&mut harness, "trace-gradient-apply", "Apply limits");
    click(&mut harness, "trace-gradient-low-color", "Low color");
    replace(&mut harness, "color-hex", "#123456");
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Pressed,
        event: zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Enter,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(64);
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].speed.colors[0],
        [18, 52, 86]
    );
    click(&mut harness, "trace-gradient-low-color", "Low color");
    click(&mut harness, "map-color-mode", "Speed gradient");
    click(&mut harness, "zui-select__item", "Height gradient");
    assert_eq!(pane.trace_colors.get_untracked()[&2].speed.range, [5., 50.]);
    assert_eq!(
        pane.trace_colors.get_untracked()[&2].height.range,
        [0., 100.]
    );
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
    click(
        &mut harness,
        "state-trace-tint-toggle",
        "State-band trace tint: off",
    );
    assert!(pane.state_tint.get_untracked());
    let names = harness.app().windows()[0]
        .nodes_with_class("state-color-key-rule")
        .iter()
        .map(|node| harness.app().windows()[0].dom().text_content(*node))
        .collect::<Vec<_>>();
    assert!(names.iter().any(|name| name.contains("Disarmed")));
    assert!(names.iter().any(|name| name.contains("Armed")));
    assert_eq!(
        workbench
            .geometry_item(99)
            .unwrap()
            .states
            .unwrap()
            .rules
            .len(),
        2
    );
    click(&mut harness, "settings-done", "Done");
    let spec = pane.trajectory.get_untracked().unwrap();
    let restored: crate::trajectory::Spec =
        serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(restored, spec);
}
