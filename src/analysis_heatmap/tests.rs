use super::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
use zgui::{
    geom::{DevicePx, Size},
    platform::SurfaceEvent,
    render::Renderer,
};
use zgui_platform_headless::Harness;
use zgui_testkit_scene::CaptureRenderer;

fn data(columns: usize, rows: usize) -> Heatmap {
    Heatmap {
        values: (0..columns * rows)
            .map(|i| Some((i % 256) as f64))
            .collect::<Vec<_>>()
            .into(),
        columns,
        rows,
        x_low: 0.,
        x_high: columns as f64,
        y_low: 0.,
        y_high: rows as f64,
        x_label: "x".into(),
        y_label: "y".into(),
        min_db: 0.,
        max_db: 255.,
    }
}
fn settle_until(harness: &mut Harness<zgui::runtime::Runtime>, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        harness.settle(64);
        assert!(Instant::now() < deadline, "Heatmap preparation timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn cell_tiles_preserve_missing_bins_merge_runs_and_cancel() {
    use kurbo::Shape as _;
    let mut data = data(5, 1);
    data.values = vec![Some(0.), Some(0.), None, Some(0.), Some(255.)].into();
    let tile = Tile::visible(&data, Ranges::new((0., 5.), (0., 1.))).unwrap();
    let prepared = geometry::prepare(&data, tile, &AtomicBool::new(false)).unwrap();
    assert_eq!(prepared.shapes.len(), 2);
    let blue = &prepared.shapes[0].path;
    assert_eq!(
        blue.elements().len(),
        10,
        "Adjacent same-color cells become one rectangle"
    );
    for x in [0.5, 1.5, 3.5] {
        assert_ne!(blue.winding((x, 0.5).into()), 0);
    }
    assert_eq!(
        blue.winding((2.5, 0.5).into()),
        0,
        "Missing bins stay transparent"
    );
    assert_eq!(blue.winding((4.5, 0.5).into()), 0);
    assert!(geometry::prepare(&data, tile, &AtomicBool::new(true)).is_none());
    assert!(Tile::visible(&data, Ranges::new((10., 20.), (0., 1.))).is_none());
    data.values = vec![].into();
    assert!(Tile::visible(&data, Ranges::new((0., 5.), (0., 1.))).is_none());
}

#[test]
fn cell_transform_matches_signal_coordinates_at_offset_origins() {
    let mut data = data(64, 64);
    data.x_low = 1000.;
    data.x_high = 1128.;
    data.y_low = -20.;
    data.y_high = 108.;
    let view = project(&data, Ranges::new((1040., 1072.), (28., 60.)), 320., 200.).unwrap();
    let point = view.transform * kurbo::Point::new(20.5, 24.5);
    assert!((point.x - 10.).abs() < 1e-9);
    assert!((point.y - 193.75).abs() < 1e-9);
    assert!(view.tile.unwrap().contains(Tile {
        left: 20,
        right: 36,
        bottom: 24,
        top: 40
    }));
}

#[test]
fn overscan_is_bounded_without_discarding_visible_cells() {
    let mut dimensions = data(1, 1);
    dimensions.columns = 4096;
    dimensions.rows = 4096;
    let small = Tile {
        left: 1000,
        right: 1256,
        bottom: 1000,
        top: 1256,
    };
    let padded = small.padded(&dimensions);
    assert!(padded.contains(small));
    assert!(padded.cells() > small.cells());
    assert!(padded.cells() <= 262_144);
    let large = Tile {
        left: 1000,
        right: 2024,
        bottom: 1000,
        top: 2024,
    };
    assert_eq!(large.padded(&dimensions), large);
}

#[test]
fn retained_heatmap_exports_match_clipped_cell_reference_during_pan_and_zoom() {
    use kurbo::Shape as _;
    use zgui::canvas::{Brush, CanvasScene, ShapeBuilder, zgui_color::Color};
    let mut data = data(4, 4);
    data.values = (0..16)
        .map(|i| {
            if i % 5 == 0 {
                None
            } else {
                Some(if i % 2 == 0 { 0. } else { 255. })
            }
        })
        .collect::<Vec<_>>()
        .into();
    let full = Tile::visible(&data, Ranges::new((0., 4.), (0., 4.))).unwrap();
    let prepared = geometry::prepare(&data, full, &AtomicBool::new(false)).unwrap();
    for ranges in [
        Ranges::new((0., 4.), (0., 4.)),
        Ranges::new((1.5, 3.5), (0.5, 2.5)),
        Ranges::new((2., 3.), (1., 2.)),
        Ranges::new((-1., 2.), (-1., 2.)),
    ] {
        let mut retained = CanvasScene::default();
        for shape in prepared.shapes.iter() {
            retained.push(shape.clone());
        }
        retained.set_transform(project(&data, ranges, 60., 60.).unwrap().transform);
        let mut reference = CanvasScene::default();
        let g = PlotGeometry::with_bounds(0., 0., 60., 60., 60., 60.).unwrap();
        for row in 0..4 {
            for col in 0..4 {
                let Some(value) = data.values[row * 4 + col] else {
                    continue;
                };
                let (left, right) = (
                    (col as f64).max(ranges.x.min),
                    (col as f64 + 1.).min(ranges.x.max),
                );
                let (bottom, top) = (
                    (row as f64).max(ranges.y.min),
                    (row as f64 + 1.).min(ranges.y.max),
                );
                if left >= right || bottom >= top {
                    continue;
                }
                let path = kurbo::Rect::new(
                    g.x(left, ranges.x),
                    g.y(top, ranges.y),
                    g.x(right, ranges.x),
                    g.y(bottom, ranges.y),
                )
                .to_path(0.1);
                let color = if value == 0. {
                    Color::srgb_u8(0, 0, 255, 255)
                } else {
                    Color::srgb_u8(255, 0, 0, 255)
                };
                reference.push(ShapeBuilder::new(path).fill(Brush::Solid(color)).build());
            }
        }
        let description = plot::PlotDescription::new(60., 60., "#123456");
        let render = |scene: &CanvasScene| {
            plot::zgui_svg::ScreenshotFormat::Png
                .encode(&plot::zgui_svg::scenes_to_svg(&[scene], &description).unwrap())
                .unwrap()
        };
        assert_eq!(
            render(&retained),
            render(&reference),
            "Heatmap pixels must match clipped signal-space cells at {ranges:?}"
        );
    }
}

#[test]
fn pans_and_zooms_keep_geometry_while_latest_tiles_prepare_on_workers() {
    let (started, starts) = mpsc::channel();
    let (resume, permission) = mpsc::channel();
    let permission = Mutex::new(permission);
    let ui = std::thread::current().id();
    let hook = PreparationHook(Arc::new(move |data, tile, _| {
        assert_ne!(std::thread::current().id(), ui);
        let _ = started.send(tile);
        let _ = permission
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10));
        // Deliberately complete superseded work to exercise the UI publication
        // guard, independently of cooperative cancellation inside the builder.
        geometry::prepare(data, tile, &AtomicBool::new(false))
    }));
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler = app()
        .with_size(400., 300.)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .with_stylesheet("canvas {width:400px;height:300px;}")
        .into_handler(move || {
            provide_local_context(hook.clone());
            let state = PlotState::default();
            state.ranges.set(Ranges::new((16., 32.), (16., 32.)));
            let pixels = RwSignal::new_local((400., 300.));
            let visible = RwSignal::new_local(true);
            let data = Arc::new(data(64, 64));
            let input = data.clone();
            let view = Memo::new(move |_| {
                if !visible.get() {
                    return None;
                }
                let (w, h) = pixels.get();
                project(&input, state.ranges.get(), w, h)
            });
            let handle = CanvasHandle::new();
            let running = install(handle.clone(), view, data);
            *saved.borrow_mut() = Some((state, pixels, handle.clone(), running, visible));
            zgui::elements::canvas().scene(&handle).into_view()
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(400.),
        DevicePx(300.),
    )));
    harness.settle(64);
    let (state, pixels, handle, running, visible) = captured.borrow().as_ref().unwrap().clone();
    settle_until(&mut harness, || starts.try_recv().is_ok());
    resume.send(()).unwrap();
    settle_until(&mut harness, || !running.get_untracked());
    let paths = || {
        zgui::canvas::resolve(handle.scene().token())
            .unwrap()
            .lock()
            .unwrap()
            .shapes()
            .iter()
            .map(|shape| shape.path.clone())
            .collect::<Vec<_>>()
    };
    let retained = paths();
    assert!(!retained.is_empty());
    let revision = handle.scene().revision();
    let view = handle.scene().view();
    for i in 1..=10 {
        let offset = i as f64 * 0.5;
        state.ranges.set(Ranges::new(
            (16. + offset, 32. + offset),
            (16. + offset, 32. + offset),
        ));
        harness.settle(16);
    }
    state.ranges.set(Ranges::new((24., 28.), (24., 28.)));
    pixels.set((800., 600.));
    harness.settle(64);
    assert_eq!(
        handle.scene().revision(),
        revision,
        "Pan, zoom and resize must only change the matrix inside cached coverage"
    );
    assert!(handle.scene().view() > view);
    assert!(starts.try_recv().is_err());
    assert!(
        retained
            .iter()
            .zip(paths())
            .all(|(old, new)| Arc::ptr_eq(old, &new))
    );

    state.ranges.set(Ranges::new((40., 48.), (40., 48.)));
    settle_until(&mut harness, || starts.try_recv().is_ok());
    assert_eq!(
        handle.scene().revision(),
        revision,
        "Old geometry survives while new cells prepare"
    );
    state.ranges.set(Ranges::new((55., 63.), (55., 63.)));
    harness.settle(16);
    state.ranges.set(Ranges::new((16., 32.), (16., 32.)));
    harness.settle(16);
    resume.send(()).unwrap();
    settle_until(&mut harness, || !running.get_untracked());
    assert_eq!(
        handle.scene().revision(),
        revision,
        "Returning to cached coverage rejects stale completions and pending work"
    );
    assert!(starts.try_recv().is_err());

    state.ranges.set(Ranges::new((55., 63.), (55., 63.)));
    settle_until(&mut harness, || starts.try_recv().is_ok());
    state.ranges.set(Ranges::new((56., 64.), (56., 64.)));
    harness.settle(16);
    assert!(
        starts.try_recv().is_err(),
        "One compatible job covers nearby pending views"
    );
    resume.send(()).unwrap();
    settle_until(&mut harness, || !running.get_untracked());
    assert!(handle.scene().revision() > revision);
    let revision = handle.scene().revision();
    state.ranges.set(Ranges::new((0., 4.), (0., 4.)));
    settle_until(&mut harness, || starts.try_recv().is_ok());
    visible.set(false);
    harness.settle(16);
    resume.send(()).unwrap();
    settle_until(&mut harness, || !running.get_untracked());
    assert_eq!(
        handle.scene().revision(),
        revision,
        "Hidden charts reject in-flight completion"
    );
    assert!(starts.try_recv().is_err());
    visible.set(true);
    settle_until(&mut harness, || starts.try_recv().is_ok());
    resume.send(()).unwrap();
    settle_until(&mut harness, || !running.get_untracked());
    assert!(
        handle.scene().revision() > revision,
        "Showing the chart prepares newly visible cells"
    );
}
