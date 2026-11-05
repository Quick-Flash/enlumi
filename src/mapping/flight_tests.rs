use super::*;
use crate::{state::Line, trajectory::Tracks};
use std::sync::Arc;

fn lines(lon: &[(f64, f64)], lat: &[(f64, f64)]) -> Vec<Line> {
    [("lon", lon), ("lat", lat)]
        .into_iter()
        .enumerate()
        .map(|(i, (name, samples))| Line {
            id: i as u64 + 1,
            source: name.into(),
            name: name.into(),
            revision: 0,
            data: plot::time_series::SeriesData::new(
                samples
                    .iter()
                    .map(|&(x, y)| PlotPoint { x, y })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        })
        .collect()
}
fn spec() -> Spec {
    Spec {
        x: "lon".into(),
        y: "lat".into(),
        gps_scale: Some(1.),
        map: Some(Config::default()),
        ..Default::default()
    }
}

#[test]
fn map_tracks_keep_coordinates_unwrap_dateline_pick_segments_and_preserve_gaps() {
    let columns = lines(
        &[(0., 179.99), (1., -179.99), (2., -179.98), (100., -179.97)],
        &[(0., 10.), (1., 10.001), (2., 10.002), (100., 10.003)],
    );
    let data = Tracks::prepare(&columns, &spec()).unwrap();
    let track = &data.traces[0].track;
    assert_eq!(track.geographic.as_ref().unwrap()[1], [-179.99, 10.001]);
    assert!(track.points[1].x - track.points[0].x < 3000.);
    assert_eq!(track.times.len(), track.geographic.as_ref().unwrap().len());
    assert!(track.at(50.).is_none());
    assert!(track.at(101.).is_none());
    let camera = fit(&data, [800., 600.], &Default::default()).unwrap();
    assert!(
        camera.zoom > 10.,
        "A dateline crossing must fit its local extent, not the whole planet"
    );
    let midpoint = track.at(0.5).unwrap();
    let time = pick_time(
        &data,
        midpoint,
        camera.ranges([800., 600.]),
        [800., 600.],
        &Default::default(),
    )
    .unwrap();
    assert!((time - 0.5).abs() < 1e-8);
    let before_gap = track.points[2];
    let after_gap = *track.points.last().unwrap();
    let midpoint = PlotPoint {
        x: (before_gap.x + after_gap.x) / 2.,
        y: (before_gap.y + after_gap.y) / 2.,
    };
    let tiny = Ranges::new(
        (midpoint.x - 10., midpoint.x + 10.),
        (midpoint.y - 10., midpoint.y + 10.),
    );
    assert!(
        pick_time(&data, midpoint, tiny, [800., 600.], &Default::default()).is_none(),
        "Picking cannot bridge a GPS outage"
    );
    assert!(data.matches(&columns, &spec()));
    let mut xy = spec();
    xy.map = None;
    assert!(!data.matches(&columns, &xy));
}

#[test]
fn map_spec_round_trips_camera_and_rejects_non_geographic_or_corrupt_input() {
    let mut map = spec();
    map.map.as_mut().unwrap().camera = Some(Camera {
        center: project(7., 46.).unwrap(),
        zoom: 14.5,
    });
    let encoded = serde_json::to_string(&map).unwrap();
    let decoded: Spec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(map, decoded);
    decoded.validate().unwrap();
    map.map.as_mut().unwrap().camera.as_mut().unwrap().zoom = f64::NAN;
    assert!(map.validate().is_err());
    map.map = Some(Config::default());
    map.gps_scale = None;
    assert!(map.validate().is_err());
    let old = serde_json::to_value(Spec::default()).unwrap();
    assert!(old.get("map").is_none());
}

#[test]
fn scaled_tlog_sources_stay_within_their_vehicle_and_receiver() {
    let ids = [
        "mavlink/1.1/GPS_RAW_INT.lon",
        "mavlink/1.1/GPS_RAW_INT.lat",
        "mavlink/1.1/GPS_RAW_INT.fix_type",
        "mavlink/2.1/GPS_RAW_INT.lon",
        "mavlink/2.1/GPS_RAW_INT.lat",
        "mavlink/3.1/GPS_RAW_INT.lon",
    ]
    .map(String::from);
    let choices = crate::trajectory::gps_pairs(&ids);
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].gps_scale, Some(1e-7));
    assert!(choices[0].fix.starts_with("mavlink/1.1/"));
    let mut s = choices[0].clone();
    s.map = Some(Config::default());
    let mut columns = lines(
        &[(0., 70_000_000.), (1., 70_010_000.)],
        &[(0., 460_000_000.), (1., 460_010_000.)],
    );
    columns[0].source = s.x.clone();
    columns[1].source = s.y.clone();
    columns.push(Line {
        id: 3,
        source: s.fix.clone(),
        name: "fix".into(),
        revision: 0,
        data: plot::time_series::SeriesData::new(vec![
            PlotPoint { x: 0., y: 2. },
            PlotPoint { x: 1., y: 1. },
        ])
        .unwrap(),
    });
    let data = Tracks::prepare(&columns, &s).unwrap();
    let track = &data.traces[0].track;
    assert_eq!(track.geographic.as_ref().unwrap()[0], [7., 46.]);
    assert!(track.points[1].x.is_nan());
    assert!(track.at(0.5).is_none());
}

fn log(offset: f64) -> Arc<crate::log::LogStore> {
    use px4_ulog::full_parser::{Completeness, MultiId, ParsedData, SomeVec};
    use std::collections::HashMap;
    let mut instances = HashMap::new();
    for instance in 0..2 {
        instances.insert(
            MultiId::new(instance),
            HashMap::from([
                (
                    "timestamp".into(),
                    SomeVec::UInt64((0..20).map(|i| i * 1_000_000).collect()),
                ),
                (
                    "longitude_deg".into(),
                    SomeVec::Double(
                        (0..20)
                            .map(|i| {
                                7.6 + offset + f64::from(instance) * 0.0004 + i as f64 * 0.0002
                            })
                            .collect(),
                    ),
                ),
                (
                    "latitude_deg".into(),
                    SomeVec::Double(
                        (0..20)
                            .map(|i| 46. + offset + (i as f64 * 0.35).sin() * 0.001)
                            .collect(),
                    ),
                ),
                ("fix_type".into(), SomeVec::UInt8(vec![3; 20])),
            ]),
        );
    }
    Arc::new(
        crate::log::LogStore::from_parsed(
            "Synthetic GPS flight".into(),
            0,
            ParsedData {
                completeness: Completeness::Complete,
                messages: HashMap::from([
                    ("sensor_gps".into(), instances),
                    (
                        "vehicle_local_position".into(),
                        HashMap::from([(
                            MultiId::new(0),
                            HashMap::from([
                                (
                                    "timestamp".into(),
                                    SomeVec::UInt64((0..20).map(|i| i * 1_000_000).collect()),
                                ),
                                (
                                    "x".into(),
                                    SomeVec::Double(
                                        (0..20).map(|i| 100. + i as f64 * 20.).collect(),
                                    ),
                                ),
                                (
                                    "y".into(),
                                    SomeVec::Double(
                                        (0..20)
                                            .map(|i| 200. + (i as f64 * 0.3).sin() * 100.)
                                            .collect(),
                                    ),
                                ),
                            ]),
                        )]),
                    ),
                ]),
            },
        )
        .unwrap(),
    )
}

#[test]
fn normal_map_workspace_recipe_keeps_binding_and_camera() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log(0.)));
    let mut source = explicit_source(0);
    source.map = Some(Config {
        camera: Some(Camera {
            center: project(7.65, 46.).unwrap(),
            zoom: 14.,
        }),
        ..Config::default()
    });
    let pane = workbench.pane();
    let fields = source
        .sources_mut()
        .filter(|s| !s.is_empty())
        .map(|s| s.clone())
        .collect::<Vec<_>>();
    let columns = fields
        .iter()
        .enumerate()
        .map(|(index, field)| Line {
            id: index as u64 + 1,
            source: field.clone(),
            name: field.clone(),
            revision: 0,
            data: workbench
                .log
                .with_untracked(|l| l.as_ref().unwrap().prepare(field).unwrap()),
        })
        .collect();
    pane.lines.set(columns);
    pane.trajectory.set(Some(source.clone()));
    let bundle = crate::preset_capture::capture(
        workbench,
        &crate::preset_capture::CaptureKind::Workspace,
        "GPS flight",
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flight.json");
    crate::preset_store::save(bundle, &path, false, None).unwrap();
    let definition = Arc::new(crate::profile::load(&path).unwrap());
    let saved = definition.workspace_presets[0].views[0].plots[0]
        .trajectory
        .as_ref()
        .unwrap();
    assert_eq!(saved.map, source.map);
    assert!(saved.gps_scale.is_some());
    let restored = window.scope.with(Workbench::new);
    restored.log.set(workbench.log.get_untracked());
    restored.open_saved_workspace(crate::workspace_state::SavedWorkspace {
        path,
        name: "GPS flight".into(),
        views: 1,
        definition,
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while restored.pane().trajectory.with_untracked(Option::is_none) {
        window.frame();
        assert!(
            std::time::Instant::now() < deadline,
            "Saved map workspace did not restore"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        restored.pane().trajectory.get_untracked().unwrap().map,
        source.map
    );
}

#[test]
fn normal_map_renders_picks_time_reuses_geometry_and_handles_hide_reload_and_close() {
    use crate::{trajectory_plot::OverlayProbe, workspace::PlotWorkspaceProps};
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };
    use zgui::{
        geom::{CssPx, DevicePx, Point, Size},
        platform::SurfaceEvent,
        vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp},
    };
    use zgui_platform_headless::Harness;
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let Some(graphics) = super::super::test_renderer::graphics() else {
        return;
    };
    let directory = tempfile::tempdir_in("target").unwrap();
    let local = directory.path().to_path_buf();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let probe = OverlayProbe(Rc::new(RefCell::new(None)));
    let observed = probe.clone();
    let handler=app().with_size(1200.,800.).with_stylesheet(format!("{}\n{}",super::super::preview::stylesheet(),plot::zgui_time_series::stylesheet()))
        .with_renderer(Box::new(move |_,target|{
            let mut renderer=graphics.renderer_offscreen(target,zgui::surface::wgpu::TextureFormat::Bgra8Unorm,false)?;
            zgui_render_vector_vello::attach(&mut renderer,target.size);Ok(Box::new(super::super::test_renderer::ReadbackRenderer(renderer)))
        })).into_handler(move ||{
            provide_local_context(probe.clone());let workbench=Workbench::new();workbench.log.set(Some(log(0.)));
            let service=Service::new(super::super::TileConfig{cache:local.join("cache"),local:Some(local.clone()),network:false});workbench.map_tiles.set_value(Some(service));
            let original=workbench.pane();let pane=workbench.add_plot(original.id,false,true);bind(workbench,pane,0,0);*saved.borrow_mut()=Some((workbench,pane,original,service));
            view!{column(class="page workbench"){row(class="body"){PlotWorkspace(workbench=workbench)}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.),
        DevicePx(800.),
    )));
    harness.settle(128);
    let (workbench, pane, original, service) = captured.borrow().unwrap();
    let pump = |h: &mut Harness<zgui::runtime::Runtime>, until: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            h.advance(Duration::from_millis(17));
            h.settle(64);
            if until() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Normal flight map did not settle"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    pump(&mut harness, &|| {
        pane.trajectory_data.with_untracked(Option::is_some)
            && pane
                .trajectory
                .with_untracked(|s| s.as_ref().unwrap().map.unwrap().camera.is_some())
            && observed
                .0
                .borrow()
                .as_ref()
                .is_some_and(|p| p.paths.with_untracked(|p| !p.is_empty()))
    });
    // Generate only the currently demanded visible local PNGs; no HTTP requests.
    for key in service.requested() {
        assert!(!key.dem);
        let path = directory
            .path()
            .join("osm")
            .join(key.z.to_string())
            .join(key.x.to_string())
            .join(format!("{}.png", key.y));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let image = image::RgbaImage::from_fn(256, 256, |x, y| {
            image::Rgba(if x % 64 < 2 || y % 64 < 2 {
                [95, 115, 125, 255]
            } else {
                [35, 65, 70, 255]
            })
        });
        image.save(path).unwrap();
    }
    service.retry();
    pump(&mut harness, &|| service.complete());
    let data = pane.trajectory_data.get_untracked().unwrap();
    assert!(data.traces[0].track.geographic.is_some());
    let canvas = observed.0.borrow().as_ref().unwrap().canvas.clone();
    let revision = canvas.scene().revision();
    let loads = service.loads();
    for time in [1., 2., 3., 4.] {
        workbench.timeline.time.set(Some(time));
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert_eq!(
        canvas.scene().revision(),
        revision,
        "Marker movement must not re-encode the flight path"
    );
    assert_eq!(service.loads(), loads);
    assert!(Arc::ptr_eq(
        &data,
        &pane.trajectory_data.get_untracked().unwrap()
    ));
    assert!(
        harness.app().windows()[0]
            .box_with_class("workbench-legend")
            .is_some()
    );
    let svg = workbench.capture_screenshot(Some(pane)).unwrap();
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.contains("OpenStreetMap contributors"));
    assert!(svg.contains("latitude_deg"));
    assert!(svg.contains("<path"));
    let png = plot::zgui_svg::ScreenshotFormat::Png.encode(&svg).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let raster = image::load_from_memory(&png).unwrap().to_rgba8();
    assert!(
        raster
            .pixels()
            .filter(|p| p[0] == 35 && p[1] == 65 && p[2] == 70)
            .count()
            > 10_000,
        "PNG must contain loaded basemap pixels, not only vector paths"
    );
    if std::env::var_os("ENLUMI_FLIGHT_MAP_PREVIEW").is_some() {
        std::fs::write("target/flight-map-export.png", &png).unwrap();
        std::fs::write("target/flight-map-export.svg", &svg).unwrap();
    }
    assert!(
        workbench
            .capture_screenshot(None)
            .unwrap()
            .contains("data:image/png;base64,")
    );
    assert_eq!(service.loads(), loads, "Snapshotting cannot fetch tiles");

    if let Some(path) = std::env::var_os("ENLUMI_FLIGHT_MAP_PREVIEW") {
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
        super::super::tests::capture(&pixels, path);
    }
    let area = harness.app().windows()[0]
        .box_with_class("flight-map-view")
        .unwrap();
    let pixels = [f64::from(area.size.width.0), f64::from(area.size.height.0)];
    let g = Camera::geometry(pixels).unwrap();
    let ranges = pane.plot.ranges.get_untracked();
    let sample = data.traces[0].track.at(7.).unwrap();
    let cursor = Point::new(
        CssPx(area.origin.x.0 + g.x(sample.x, ranges.x) as f32 + 3.),
        CssPx(area.origin.y.0 + g.y(sample.y, ranges.y) as f32 + 2.),
    );
    let pinned = workbench.timeline.time.get_untracked();
    harness.deliver_to_first(SurfaceEvent::Pointer {
        action: PointerAction::Moved,
        event: PointerEvent::mouse(cursor),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(64);
    let window = &harness.app().windows()[0];
    let tooltip = window.nodes_with_class("map-hover-tooltip");
    assert_eq!(tooltip.len(), 1, "2D hover must show sample location");
    let label = window.dom().text_content(tooltip[0]);
    assert!(
        label.contains("t: 7.000 s") && label.contains("Lat:") && label.contains("Lon:"),
        "{label}"
    );
    assert!(window.box_with_class("map-hover-pin").is_some());
    if std::env::var_os("ENLUMI_MAP_HOVER_PREVIEW").is_some() {
        let pixels = window
            .renderer()
            .as_any()
            .unwrap()
            .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
            .unwrap()
            .read_composed();
        super::super::tests::capture(&pixels, "target/map-hover-2d.ppm");
    }
    assert_eq!(
        workbench.timeline.time.get_untracked(),
        pinned,
        "Hover cannot change pinned timeline"
    );
    workbench.timeline.follow.set(true);
    harness.deliver_to_first(SurfaceEvent::Pointer {
        action: PointerAction::Moved,
        event: PointerEvent::mouse(cursor),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(64);
    assert_eq!(workbench.timeline.time.get_untracked(), Some(7.));
    workbench.timeline.follow.set(false);
    let point = data.traces[0].track.at(7.5).unwrap();
    let x = area.origin.x.0 + g.x(point.x, ranges.x) as f32;
    let y = area.origin.y.0 + g.y(point.y, ranges.y) as f32;
    for action in [PointerAction::Pressed, PointerAction::Released] {
        let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(y)));
        event.button = Some(PointerButton::Primary);
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert!((workbench.timeline.time.get_untracked().unwrap() - 7.5).abs() < 1e-4);
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-hover-tooltip")
            .is_none(),
        "Press clears hover before panning or pinning"
    );
    let camera = pane
        .trajectory
        .get_untracked()
        .unwrap()
        .map
        .unwrap()
        .camera
        .unwrap();
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1000.),
        DevicePx(700.),
    )));
    harness.settle(128);
    assert_eq!(
        pane.trajectory
            .get_untracked()
            .unwrap()
            .map
            .unwrap()
            .camera
            .unwrap(),
        camera,
        "Resize preserves map scale and center"
    );
    workbench.activate(original.id);
    harness.settle(128);
    assert!(
        service.requested().is_empty(),
        "Hidden map must release demand"
    );
    workbench.activate(pane.id);
    harness.settle(128);
    assert!(Arc::ptr_eq(
        &data,
        &pane.trajectory_data.get_untracked().unwrap()
    ));
    bind(workbench, pane, 1, 1);
    harness.settle(64);
    pump(&mut harness, &|| {
        pane.trajectory_data
            .with_untracked(|d| d.as_ref().is_some_and(|d| !Arc::ptr_eq(d, &data)))
            && pane
                .trajectory
                .get_untracked()
                .unwrap()
                .map
                .unwrap()
                .camera
                .is_some()
    });
    assert_eq!(pane.trajectory.get_untracked().unwrap().pairs().len(), 2);
    assert_eq!(pane.view_lines.get_untracked().len(), 2);
    assert_ne!(
        pane.plot
            .style(&pane.view_lines.get_untracked()[0].id.to_string())
            .color,
        pane.plot
            .style(&pane.view_lines.get_untracked()[1].id.to_string())
            .color
    );
    workbench.new_flight_map();
    let builder = workbench.trajectory_builder();
    builder.pane.trajectory.set(Some(Spec {
        map: Some(Config::default()),
        gps_scale: Some(1.),
        ..Default::default()
    }));
    crate::trajectory_axes::assign(
        builder.pane,
        2,
        false,
        &["vehicle_local_position/0/x".into()],
    )
    .unwrap();
    crate::trajectory_axes::assign(
        builder.pane,
        2,
        true,
        &["vehicle_local_position/0/y".into()],
    )
    .unwrap();
    crate::trajectory_axes::local_pair(builder.pane, 2).unwrap();
    builder.target.set(Some(pane.id));
    workbench.drop_trajectory_pair(2, pane.id);
    pump(&mut harness, &|| {
        pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .any(|l| l.source == "vehicle_local_position/0/y")
        })
    });
    pane.map_origin_target
        .set(Some("vehicle_local_position/0/y".into()));
    harness.settle(128);
    let area = harness.app().windows()[0]
        .box_with_class("flight-map-view")
        .unwrap();
    let size = [f64::from(area.size.width.0), f64::from(area.size.height.0)];
    let ranges = pane.plot.ranges.get_untracked();
    let g = Camera::geometry(size).unwrap();
    let local = [size[0] * 0.4, size[1] * 0.55];
    let expected = geographic([g.x_value(local[0], ranges.x), g.y_value(local[1], ranges.y)]);
    for action in [PointerAction::Pressed, PointerAction::Released] {
        let mut event = PointerEvent::mouse(Point::new(
            CssPx(area.origin.x.0 + local[0] as f32),
            CssPx(area.origin.y.0 + local[1] as f32),
        ));
        event.button = Some(PointerButton::Primary);
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action,
            event,
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    pump(&mut harness, &|| {
        pane.trajectory_data
            .with_untracked(|d| d.as_ref().is_some_and(|d| d.traces.len() == 3))
    });
    let set = pane.trajectory.get_untracked().unwrap().extra[1]
        .local
        .unwrap()
        .origin
        .unwrap();
    assert!((set[0] - expected[0]).abs() < 1e-6 && (set[1] - expected[1]).abs() < 1e-6);
    assert!(pane.map_origin_target.get_untracked().is_none());
    let local_point = pane.trajectory_data.get_untracked().unwrap().traces[2]
        .track
        .points[0];
    let projected = project(set[0], set[1]).unwrap();
    assert!(
        (local_point.x - projected[0]).abs() < 1e-6 && (local_point.y - projected[1]).abs() < 1e-6
    );
    let mixed = workbench.capture_screenshot(Some(pane)).unwrap();
    assert!(mixed.contains("vehicle_local_position/0/y"));
    let hidden = pane.trajectory_data.get_untracked().unwrap().traces[0]
        .id
        .to_string();
    pane.plot
        .hidden
        .set(std::collections::BTreeSet::from([hidden]));
    harness.settle(64);
    let scene = zgui::canvas::resolve(observed.0.borrow().as_ref().unwrap().canvas.scene().token())
        .unwrap();
    assert_eq!(scene.lock().unwrap().series().len(), 2);
    pane.plot.hidden.set(Default::default());
    harness.settle(64);
    let selected = pane.trajectory_data.get_untracked().unwrap();
    let before = selected.traces[0].track.points[0];
    let new_log = log(1.);
    let session = crate::log_session::Session::capture(workbench, None)
        .unwrap()
        .refresh(new_log.clone());
    workbench.log.set(Some(new_log));
    workbench.restore_log_session(session);
    harness.settle(64);
    pump(&mut harness, &|| {
        pane.trajectory_data
            .with_untracked(|d| d.as_ref().is_some_and(|d| !Arc::ptr_eq(d, &selected)))
            && pane
                .trajectory
                .get_untracked()
                .unwrap()
                .map
                .unwrap()
                .camera
                .is_some()
    });
    let after = pane.trajectory_data.get_untracked().unwrap().traces[0]
        .track
        .points[0];
    assert!((after.x - before.x).abs() > 100_000.);
    workbench.close_plot(pane.id);
    harness.settle(128);
    assert!(service.requested().is_empty());
}

#[test]
fn changing_position_source_ignores_pending_old_receiver_fields() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log(0.)));
    let pane = workbench.pane();
    bind(workbench, pane, 0, 0);
    bind(workbench, pane, 0, 1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while pane.lines.with_untracked(|lines| lines.len() != 3) {
        window.frame();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..10 {
        window.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        pane.lines
            .with_untracked(|lines| lines.iter().all(|line| line.source.contains("/1/")))
    );
    assert_eq!(pane.trajectory.get_untracked().unwrap().pairs().len(), 1);
}

fn explicit_source(receiver: usize) -> Spec {
    Spec {
        x: format!("sensor_gps/{receiver}/longitude_deg"),
        y: format!("sensor_gps/{receiver}/latitude_deg"),
        fix: format!("sensor_gps/{receiver}/fix_type"),
        gps_scale: Some(1.),
        ..Default::default()
    }
}
fn bind(workbench: Workbench, pane: Pane, index: usize, receiver: usize) {
    workbench.new_flight_map();
    let builder = workbench.trajectory_builder();
    builder.pane.trajectory.set(Some(Spec {
        map: Some(Config::default()),
        gps_scale: Some(1.),
        ..Default::default()
    }));
    let source = explicit_source(receiver);
    crate::trajectory_axes::assign(builder.pane, index, false, &[source.x]).unwrap();
    crate::trajectory_axes::assign(builder.pane, index, true, &[source.y]).unwrap();
    crate::trajectory_axes::map_pair_options(builder.pane, index, 1., Some(source.fix)).unwrap();
    if pane.trajectory.with_untracked(Option::is_some) {
        builder.target.set(Some(pane.id));
    }
    workbench.drop_trajectory_pair(index, pane.id);
}

#[test]
fn map_builder_is_explicit_supports_multiple_traces_and_independent_maps() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log(0.)));
    let first = workbench.pane();
    let count = workbench.panes().get_untracked().len();
    workbench.new_flight_map();
    assert_eq!(workbench.panes().get_untracked().len(), count);
    let blank = workbench
        .trajectory_builder()
        .pane
        .trajectory
        .get_untracked()
        .unwrap();
    assert!(blank.x.is_empty() && blank.y.is_empty());
    assert!(blank.map.is_some());
    bind(workbench, first, 0, 0);
    bind(workbench, first, 1, 1);
    assert_eq!(first.trajectory.get_untracked().unwrap().pairs().len(), 2);
    let second = workbench.add_plot(first.id, false, false);
    bind(workbench, second, 0, 0);
    assert!(second.trajectory.get_untracked().unwrap().map.is_some());
    assert_eq!(first.trajectory.get_untracked().unwrap().pairs().len(), 2);
    assert_eq!(second.trajectory.get_untracked().unwrap().pairs().len(), 1);
    assert_ne!(first.id, second.id);
    let builder = workbench.trajectory_builder();
    workbench.new_trajectory(false);
    assert!(
        builder
            .pane
            .trajectory
            .get_untracked()
            .unwrap()
            .map
            .is_none(),
        "Switching to normal XY cannot retain map projection"
    );
}

#[test]
fn overzoom_preserves_position_and_caps_provider_requests() {
    let camera = Camera {
        center: project(7.6, 46.).unwrap(),
        zoom: 26.,
    };
    let size = [800., 600.];
    assert!(
        (Camera::from_ranges(camera.ranges(size), size).unwrap().zoom - camera.zoom).abs() < 1e-6
    );
    assert!(camera.map_tiles(size).iter().all(|t| t.z <= 19));
    Config {
        camera: Some(camera),
        ..Config::default()
    }
    .validate()
    .unwrap();
}

#[test]
fn local_metre_frames_anchor_first_sample_or_zero_and_round_trip_with_geographic_traces() {
    let columns = lines(
        &[(0., 100.), (1., 200.), (2., 300.)],
        &[(0., 200.), (1., 250.), (2., 300.)],
    );
    let origin = [7.6, 46.];
    let world = project(origin[0], origin[1]).unwrap();
    let scale = origin[1].to_radians().cos();
    let mut setup = spec();
    setup.gps_scale = None;
    setup.local = Some(LocalFrame {
        origin: Some(origin),
        at_first: true,
    });
    let data = Tracks::prepare(&columns, &setup).unwrap();
    let track = &data.traces[0].track;
    assert!((track.points[0].x - world[0]).abs() < 1e-7);
    assert!((track.points[0].y - world[1]).abs() < 1e-7);
    assert!(((track.points[1].x - track.points[0].x) * scale - 100.).abs() < 1e-6);
    assert!(((track.points[1].y - track.points[0].y) * scale - 50.).abs() < 1e-6);
    setup.local.as_mut().unwrap().at_first = false;
    let zero = Tracks::prepare(&columns, &setup).unwrap();
    assert!(((zero.traces[0].track.points[0].x - world[0]) * scale - 100.).abs() < 1e-6);
    let saved = serde_json::to_string(&setup).unwrap();
    assert_eq!(serde_json::from_str::<Spec>(&saved).unwrap(), setup);
    setup.local.as_mut().unwrap().origin = None;
    assert!(Tracks::prepare(&columns, &setup).is_err());
    LocalFrame {
        origin: Some([0., 0.]),
        at_first: true,
    }
    .validate()
    .unwrap();
    assert!(
        LocalFrame {
            origin: Some([0., 95.]),
            at_first: true
        }
        .validate()
        .is_err()
    );
}

#[test]
fn local_coordinates_can_be_entered_before_fields_and_each_trace_keeps_its_own_origin() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.new_flight_map();
    let pane = workbench.trajectory_builder().pane;
    crate::trajectory_axes::local_pair(pane, 0).unwrap();
    set_origin_at(pane, 0, [7., 46.]).unwrap();
    crate::trajectory_axes::local_pair(pane, 1).unwrap();
    set_origin_at(pane, 1, [8., 47.]).unwrap();
    let pairs = pane.trajectory.get_untracked().unwrap().pairs();
    assert_eq!(pairs[0].local.unwrap().origin, Some([7., 46.]));
    assert_eq!(pairs[1].local.unwrap().origin, Some([8., 47.]));
}

#[test]
fn sidebar_z_binding_loads_arbitrary_field_without_creating_an_xy_pair() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(log(0.)));
    let pane = workbench.pane();
    bind(workbench, pane, 0, 0);
    workbench.edit_trajectory(pane);
    let source = "sensor_gps/1/fix_type".to_owned();
    super::super::settings::bind_altitude(
        workbench,
        workbench.trajectory_builder().pane,
        0,
        source.clone(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !pane
        .lines
        .with_untracked(|lines| lines.iter().any(|line| line.source == source))
    {
        window.frame();
        assert!(
            std::time::Instant::now() < deadline,
            "Altitude field must survive async map-source guard"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let spec = pane.trajectory.get_untracked().unwrap();
    assert_eq!(spec.pairs().len(), 1);
    assert_eq!(spec.altitudes[&0].source, source);
}
