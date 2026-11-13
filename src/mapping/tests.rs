use super::{camera::*, gpu::Stats, preview::Model};
use std::sync::Arc;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use zgui::{
    geom::{CssPx, DevicePx, Point, Size},
    platform::SurfaceEvent,
    prelude::*,
};
use zgui_platform_headless::Harness;

#[derive(Clone, Default)]
struct Probe(Rc<RefCell<ProbeData>>);
#[derive(Default)]
struct ProbeData {
    model: Option<Model>,
    producers: Vec<Arc<Stats>>,
}
pub(super) fn record_model(model: Model) {
    if let Some(probe) = use_local_context::<Probe>() {
        probe.0.borrow_mut().model = Some(model);
    }
}
pub(super) fn record_producer(stats: Arc<Stats>) {
    if let Some(probe) = use_local_context::<Probe>() {
        probe.0.borrow_mut().producers.push(stats);
    }
}

fn pump(harness: &mut Harness<zgui::runtime::Runtime>, until: impl Fn() -> bool) {
    // Cold GPU submissions can drain after the final fixture tile is decoded.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        if until() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for map render/lifecycle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn capture(pixels: &zgui_render_wgpu::Pixels, path: impl AsRef<std::path::Path>) {
    use std::io::Write;
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let size = pixels.size();
    write!(file, "P6\n{} {}\n255\n", size.width, size.height).unwrap();
    for y in 0..size.height {
        for x in 0..size.width {
            file.write_all(&pixels.rgba(x, y)[..3]).unwrap();
        }
    }
}

#[test]
fn offline_preview_renders_and_releases_hidden_gpu_panes() {
    use super::{
        preview::{PreviewProps, stylesheet},
        test_renderer::ReadbackRenderer,
    };
    let probe = Probe::default();
    let saved = probe.clone();
    let Some(graphics) = super::test_renderer::graphics() else {
        return;
    };
    let handler = app()
        .with_size(1200., 760.)
        .with_stylesheet(stylesheet())
        .with_renderer(Box::new(move |_, target| {
            let mut renderer = graphics.renderer_offscreen(
                target,
                zgui::surface::wgpu::TextureFormat::Bgra8Unorm,
                false,
            )?;
            zgui_render_vector_vello::attach(&mut renderer, target.size);
            Ok(Box::new(ReadbackRenderer(renderer)))
        }))
        .into_handler(move || {
            provide_local_context(saved.clone());
            view! {Preview()}
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.),
        DevicePx(760.),
    )));
    harness.settle(128);
    let model = probe.0.borrow().model.expect("Preview mounted");
    let stats = probe.0.borrow().producers[0].clone();
    pump(&mut harness, || stats.frames.load(Ordering::Acquire) > 0);
    // Allow the asynchronous image decode and the presented surface to enter the scene.
    for _ in 0..8 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        std::thread::sleep(Duration::from_millis(5));
    }
    let pixels = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .read_composed();
    if let Some(path) = std::env::var_os("ENLUMI_MAP_PREVIEW") {
        capture(&pixels, path);
    }
    let gpu = harness.app().windows()[0]
        .box_with_class("map-gpu")
        .expect("GPU viewport");
    assert!(
        gpu.size.width.0 > 100. && gpu.size.height.0 > 100.,
        "Pane must fill available workspace: {gpu:?}"
    );
    let x = (gpu.origin.x.0 + 100.) as i32;
    let y = (gpu.origin.y.0 + 100.) as i32;
    assert_ne!(
        pixels.rgba(x, y),
        pixels.rgba(x + 32, y),
        "GPU checkerboard must reach composed pixels"
    );
    let image = harness.app().windows()[0]
        .box_with_class("map-tiles")
        .unwrap();
    let ix = (image.origin.x.0 + 100.) as i32;
    let iy = (image.origin.y.0 + 100.) as i32;
    assert_ne!(
        pixels.rgba(ix, iy),
        pixels.rgba(ix + 32, iy),
        "Decoded image tiles must reach composed pixels"
    );
    for x in (image.origin.x.0.ceil() as i32 + 2)
        ..((image.origin.x.0 + image.size.width.0).floor() as i32 - 2)
    {
        assert_ne!(
            pixels.rgba(x, iy),
            [18, 24, 30, 255],
            "No background seam between clipped image tiles at {x}"
        );
    }
    assert_ne!(
        pixels.rgba(x, y),
        pixels.rgba(x, gpu.origin.y.0 as i32 - 4),
        "GPU surface must be clipped below its tab strip"
    );
    let tiles = harness.app().windows()[0].nodes_with_class("map-tile");
    assert!(!tiles.is_empty());
    assert!(tiles.len() <= 256);
    let before = stats.frames.load(Ordering::Acquire);
    for _ in 0..8 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert_eq!(
        stats.frames.load(Ordering::Acquire),
        before,
        "Idle map must not produce frames"
    );
    model.camera.update(|camera| camera.center[0] += 1000.);
    pump(&mut harness, || {
        stats.frames.load(Ordering::Acquire) > before
    });
    assert_eq!(
        harness.app().windows()[0].nodes_with_class("map-tile"),
        tiles,
        "Small pan retains tile nodes"
    );
    use zgui::vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp};
    let initial = model.camera.get_untracked();
    let px = gpu.origin.x.0 + gpu.size.width.0 / 2.;
    let py = gpu.origin.y.0 + gpu.size.height.0 / 2.;
    for (action, dx) in [
        (PointerAction::Pressed, 0.),
        (PointerAction::Moved, 1.),
        (PointerAction::Moved, 30.),
        (PointerAction::Released, 30.),
    ] {
        let mut event = PointerEvent::mouse(Point::new(CssPx(px + dx), CssPx(py)));
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
        assert_eq!(
            model.camera.get_untracked().zoom,
            initial.zoom,
            "Every pan frame keeps the same zoom and tile level"
        );
    }
    let panned = model.camera.get_untracked();
    assert!(
        (panned.center[0] - initial.center[0] + 30. / initial.scale()).abs() < 1e-6,
        "Pan uses camera scale"
    );
    assert_eq!(
        panned.zoom, initial.zoom,
        "Dragging must preserve the exact zoom"
    );
    harness.deliver_to_first(SurfaceEvent::Wheel {
        event: zgui::vocab::WheelEvent {
            delta: zgui::vocab::ScrollDelta::Lines { x: 0., y: 1. },
            phase: zgui::vocab::ScrollPhase::Moved,
            position: Point::new(CssPx(px), CssPx(py)),
            id: zgui::vocab::PointerId::MOUSE,
            kind: zgui::vocab::PointerKind::Mouse,
        },
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.advance(Duration::from_millis(17));
    harness.settle(64);
    assert_ne!(
        model.camera.get_untracked().zoom,
        panned.zoom,
        "Wheel changes camera scale"
    );
    let old_gpu = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .gpu()
        .clone();
    old_gpu.loss().report(
        zgui::surface::wgpu::DeviceLostReason::Unknown,
        "map preview recovery test",
    );
    harness.deliver_to_first(SurfaceEvent::RedrawRequested);
    model.camera.update(|camera| camera.center[0] += 1000.);
    pump(&mut harness, || stats.rebuilds.load(Ordering::Acquire) > 1);
    // The pinned runtime only re-notes retained chunks after offscreen device
    // recovery; its CPU atlas still names the old GPU's textures. Reset the
    // host caches explicitly so subsequent image/DPI checks test the map,
    // rather than relying on automatic atlas recovery that zgui lacks today.
    // This is a known upstream acceptance gap, documented in docs/maps.md.
    harness.app_mut().windows_mut()[0].forget_caches();
    harness.settle(128);
    model.maximized.set(Some(1));
    pump(&mut harness, || stats.stopped.load(Ordering::Acquire) == 1);
    let stopped = stats.frames.load(Ordering::Acquire);
    model.camera.update(|camera| camera.zoom += 0.5);
    harness.settle(128);
    assert_eq!(stats.frames.load(Ordering::Acquire), stopped);
    model.maximized.set(None);
    harness.settle(128);
    let reopened = probe.0.borrow().producers.last().unwrap().clone();
    assert!(!Arc::ptr_eq(&stats, &reopened));
    pump(&mut harness, || reopened.frames.load(Ordering::Acquire) > 0);
    let before = reopened.frames.load(Ordering::Acquire);
    harness.deliver_to_first(SurfaceEvent::ScaleFactorChanged {
        scale_factor: 2.,
        size: Size::new(DevicePx(1600.), DevicePx(1000.)),
    });
    pump(&mut harness, || {
        reopened.frames.load(Ordering::Acquire) > before
    });
    // Decoding images at the new physical size is asynchronous. Wait for
    // actual coverage, with a deadline that still rejects persistent seams.
    let deadline = Instant::now() + Duration::from_secs(15);
    let pixels = loop {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        let pixels = harness.app().windows()[0]
            .renderer()
            .as_any()
            .unwrap()
            .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
            .unwrap()
            .read_composed();
        let image = harness.app().windows()[0]
            .box_with_class("map-tiles")
            .unwrap();
        let y = (image.origin.y.0 + 100.) as i32;
        let covered = ((image.origin.x.0.ceil() as i32 + 2)
            ..((image.origin.x.0 + image.size.width.0).floor() as i32 - 2))
            .all(|x| pixels.rgba(x, y) != [18, 24, 30, 255]);
        if covered || Instant::now() >= deadline {
            if let Some(path) = std::env::var_os("ENLUMI_MAP_PREVIEW") {
                capture(
                    &pixels,
                    std::path::PathBuf::from(path).with_extension("2x.ppm"),
                );
            }
            assert!(
                covered,
                "Image coverage still has gaps at 2x DPI after decode deadline"
            );
            break pixels;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let gpu = harness.app().windows()[0]
        .box_with_class("map-gpu")
        .unwrap();
    let x = (gpu.origin.x.0 + 100.) as i32;
    let y = (gpu.origin.y.0 + 100.) as i32;
    let step = (64. * model.camera.get_untracked().zoom.fract().exp2()).round() as i32;
    assert_ne!(
        pixels.rgba(x, y),
        pixels.rgba(x + step, y),
        "Checker scale follows 2x DPI"
    );
    assert_ne!(
        pixels.rgba(x, y),
        pixels.rgba(x, gpu.origin.y.0 as i32 - 4),
        "Surface stays below tab strip at 2x DPI"
    );
    let image = harness.app().windows()[0]
        .box_with_class("map-tiles")
        .unwrap();
    let y = (image.origin.y.0 + 100.) as i32;
    for x in (image.origin.x.0.ceil() as i32 + 2)
        ..((image.origin.x.0 + image.size.width.0).floor() as i32 - 2)
    {
        assert_ne!(
            pixels.rgba(x, y),
            [18, 24, 30, 255],
            "No image seam at 2x DPI at {x}"
        );
    }
    model.layout.update(|layout| layout.remove(2));
    pump(&mut harness, || {
        reopened.stopped.load(Ordering::Acquire) == 1
    });
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("map-gpu")
            .is_empty()
    );
}

#[test]
fn mercator_round_trip_and_polar_clamp() {
    for (lon, lat) in [(0., 0.), (-104.9, 39.7), (179.99, -80.)] {
        let [x, y] = geographic(project(lon, lat).unwrap());
        assert!((x - lon).abs() < 1e-9);
        assert!((y - lat).abs() < 1e-9);
    }
    assert!((project(0., 90.).unwrap()[1] - WORLD / 2.).abs() < 1e-7);
    assert!(project(0., 91.).is_none());
    assert!(project(f64::NAN, 0.).is_none());
}

#[test]
fn camera_round_trip_keeps_uniform_scale_and_bounds() {
    let camera = Camera {
        center: project(-105., 40.).unwrap(),
        zoom: 9.25,
    };
    for size in [[800., 600.], [300., 900.]] {
        let result = Camera::from_ranges(camera.ranges(size), size).unwrap();
        assert!((result.zoom - camera.zoom).abs() < 1e-10);
        for (a, b) in result.center.into_iter().zip(camera.center) {
            assert!((a - b).abs() < 1e-7);
        }
    }
    assert!(Camera::geometry([0., 200.]).is_none());
    let ranges = Camera::default().ranges([800., 600.]);
    assert!(Camera::from_ranges(ranges, [0., 0.]).is_none());
}

#[test]
fn pans_preserve_zoom_and_tile_level_at_integer_boundaries() {
    use plot::interaction::{Axes, Gesture, drag};
    for zoom in [2., 3., 9., 22., 1.999_999, 2.000_001, 9.25] {
        for size in [[800., 600.], [583., 618.], [388., 358.]] {
            let mut camera = Camera {
                center: project(179., 40.).unwrap(),
                zoom,
            };
            let geometry = Camera::geometry(size).unwrap();
            for delta in [1., 30., -100., 1000., -0.25] {
                let ranges = drag(
                    Gesture::Pan,
                    camera.ranges(size),
                    (100., 100.),
                    (100. + delta, 150.),
                    geometry,
                    Axes::BOTH,
                )
                .unwrap();
                camera = camera.pan_to(ranges, size).unwrap();
                assert_eq!(camera.zoom, zoom);
                assert!(
                    camera
                        .tiles(size)
                        .iter()
                        .all(|tile| tile.z == zoom.floor() as u8)
                );
                assert_eq!(
                    camera.uniforms(size, 1.)[2],
                    (zoom - zoom.floor()).exp2() as f32
                );
            }
        }
    }
    // Genuine zoom updates must still change scale.
    let camera = Camera::default();
    let size = [800., 600.];
    let zoomed = Camera {
        zoom: 2.5,
        ..camera
    };
    assert_eq!(
        Camera::from_ranges(zoomed.ranges(size), size).unwrap().zoom,
        2.5
    );
}

#[test]
fn tile_coverage_wraps_x_clamps_y_and_limits_work() {
    let camera = Camera {
        center: [WORLD / 2. - 1., WORLD / 2.],
        zoom: 2.5,
    };
    let tiles = camera.tiles([800., 600.]);
    assert!(tiles.iter().any(|t| t.x >= 4 && t.canonical_x() == 0));
    assert!(tiles.iter().all(|t| t.y < 1 << t.z));
    assert!(tiles.iter().any(|t| t.y == 0));
    assert!(camera.tiles([100_000., 100_000.]).len() <= 256);
    assert!(camera.tiles([f64::INFINITY, 500.]).is_empty());
    let a = Camera::default();
    let b = Camera {
        center: [10., 0.],
        ..a
    };
    assert_eq!(a.tiles([500., 400.]), b.tiles([500., 400.]));
    let tile = a.tiles([500., 400.])[0];
    assert!(b.place(tile, [500., 400.])[0] < a.place(tile, [500., 400.])[0]);
}

pub(super) fn map_fixture(root: &std::path::Path, keys: &[super::tiles::Key]) -> bool {
    let camera = Camera {
        center: project(7.6586, 45.9763).unwrap(),
        zoom: 12.,
    };
    let tile_center = [
        (camera.center[0] + WORLD / 2.) / WORLD * 4096.,
        (WORLD / 2. - camera.center[1]) / WORLD * 4096.,
    ];
    let mut added = false;
    for key in keys {
        let (source, ext, dimension) = if key.dem {
            ("mapterhorn", "webp", 512)
        } else {
            ("osm", "png", 256)
        };
        let path = root
            .join(source)
            .join(key.z.to_string())
            .join(key.x.to_string())
            .join(format!("{}.{ext}", key.y));
        if path.exists() {
            continue;
        }
        added = true;
        let mut data = image::RgbImage::new(dimension, dimension);
        for (x, y, pixel) in data.enumerate_pixels_mut() {
            *pixel = if key.dem {
                let factor = 2_f64.powi(12 - i32::from(key.z));
                let dx = (f64::from(key.x) + (f64::from(x) + 0.5) / 512.) * factor - tile_center[0];
                let dy = (f64::from(key.y) + (f64::from(y) + 0.5) / 512.) * factor - tile_center[1];
                let height = 1200.
                    + 2300. * (-(dx * dx + dy * dy) * 1.8).exp()
                    + 150. * (dx * 5.).sin() * (dy * 4.).cos();
                let encoded = ((height + 32768.) * 256.).round() as u32;
                image::Rgb([(encoded >> 16) as u8, (encoded >> 8) as u8, encoded as u8])
            } else if x % 64 < 3 || y % 64 < 3 {
                image::Rgb([210, 204, 172])
            } else {
                image::Rgb([90 + (key.x % 20) as u8, 150 + (key.y % 20) as u8, 80])
            };
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        data.save(path).unwrap();
    }
    added
}

fn load_map_fixture(
    harness: &mut Harness<zgui::runtime::Runtime>,
    model: Model,
    root: &std::path::Path,
    live: bool,
) {
    let deadline = Instant::now() + Duration::from_secs(if live { 120 } else { 45 });
    while !model.service.complete() {
        if !live && (map_fixture(root, &model.service.requested()) || model.service.failed()) {
            harness.settle(128);
            model.service.retry();
        }
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            Instant::now() < deadline,
            "Tile loading failed: {}",
            model.service.status()
        );
    }
}

fn read_pixels(harness: &Harness<zgui::runtime::Runtime>) -> zgui_render_wgpu::Pixels {
    harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap()
        .read_composed()
}

#[test]
fn map_and_terrain_render_local_xyz_and_reuse_textures_while_navigating() {
    use super::{
        preview::{PreviewProps, stylesheet},
        test_renderer::ReadbackRenderer,
        tiles::Config,
    };
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let Some(graphics) = super::test_renderer::graphics() else {
        return;
    };
    // Live checks opt in to one fixed visible view only; CI uses generated data.
    let live = std::env::var_os("ENLUMI_MAP_LIVE").is_some();
    let directory = tempfile::Builder::new()
        .prefix("map-fixture-")
        .tempdir_in("target")
        .unwrap();
    let config = if live {
        Config::default()
    } else {
        Config {
            cache: directory.path().join("cache"),
            local: Some(directory.path().to_path_buf()),
            network: false,
        }
    };
    let probe = Probe::default();
    let saved = probe.clone();
    let handler = app()
        .with_size(1200., 800.)
        .with_stylesheet(stylesheet())
        .with_renderer(Box::new(move |_, target| {
            let mut renderer = graphics.renderer_offscreen(
                target,
                zgui::surface::wgpu::TextureFormat::Bgra8Unorm,
                false,
            )?;
            zgui_render_vector_vello::attach(&mut renderer, target.size);
            Ok(Box::new(ReadbackRenderer(renderer)))
        }))
        .into_handler(move || {
            provide_local_context(saved.clone());
            view! {Preview(online=true,config=Some(config.clone()))}
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.),
        DevicePx(800.),
    )));
    harness.settle(128);
    let model = probe.0.borrow().model.unwrap();
    let stats = probe.0.borrow().producers.last().unwrap().clone();
    load_map_fixture(&mut harness, model, directory.path(), live);
    pump(&mut harness, || {
        stats.uploads.load(Ordering::Acquire) > 0
            && stats.frames.load(Ordering::Acquire) > 0
            && stats.scene_revision.load(Ordering::Acquire)
                == model.service.revision.get_untracked()
    });
    if !live {
        use super::camera::TileId;
        use super::tiles::Key;
        let child = model
            .service
            .requested()
            .into_iter()
            .filter(|k| !k.dem && k.z > 0)
            .map(|k| TileId {
                z: k.z,
                x: i64::from(k.x),
                y: k.y,
            })
            .find(|&id| model.service.payload(Key::elevation(id)).is_some())
            .unwrap();
        let parent = TileId {
            z: child.z - 1,
            x: child.x / 2,
            y: child.y / 2,
        };
        assert!(model.service.cached_children(parent).contains(&child));
        let wrapped = TileId {
            x: parent.x + (1_i64 << parent.z),
            ..parent
        };
        assert!(model.service.cached_children(wrapped).contains(&TileId {
            x: child.x + (1_i64 << child.z),
            ..child
        }));
    }
    let region = harness.app().windows()[0]
        .box_with_class("map-terrain-surface")
        .unwrap();
    let mut pixels = read_pixels(&harness);
    // Wait for the completed terrain submission to enter the UI composition.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let colored = (region.origin.y.0 as i32 + 10
            ..(region.origin.y.0 + region.size.height.0) as i32 - 10)
            .step_by(4)
            .flat_map(|y| {
                (region.origin.x.0 as i32 + 10
                    ..(region.origin.x.0 + region.size.width.0) as i32 - 10)
                    .step_by(4)
                    .map(move |x| (x, y))
            })
            .filter(|&(x, y)| {
                let p = pixels.rgba(x, y);
                if live {
                    p[0].abs_diff(41) > 5 || p[1].abs_diff(56) > 5
                } else {
                    p[1] > p[0].saturating_add(10) && p[1] > p[2].saturating_add(10)
                }
            })
            .count();
        if colored > 500 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Terrain must produce a shaded mesh, not only sky"
        );
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        std::thread::sleep(Duration::from_millis(5));
        pixels = read_pixels(&harness);
    }
    let image = harness.app().windows()[0]
        .box_with_class("map-tiles")
        .unwrap();
    let p = pixels.rgba(
        (image.origin.x.0 + 100.) as i32,
        (image.origin.y.0 + 100.) as i32,
    );
    assert_ne!(p, [18, 24, 30, 255], "2D map has decoded imagery");
    if let Some(path) = std::env::var_os("ENLUMI_MAP_TERRAIN_PREVIEW") {
        capture(&pixels, path);
    }
    if live {
        return;
    }
    let before = stats.frames.load(Ordering::Acquire);
    for _ in 0..8 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert_eq!(
        stats.frames.load(Ordering::Acquire),
        before,
        "Idle 3D has no rendering loop"
    );
    // Drag the actual input layer; camera location stays put while the orbit changes.
    let initial = model.camera.get_untracked();
    let orbit = model.orbit.get_untracked();
    let x = region.origin.x.0 + region.size.width.0 / 2.;
    let y = region.origin.y.0 + region.size.height.0 / 2.;
    use zgui::vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp};
    for (action, dx) in [
        (PointerAction::Pressed, 0.),
        (PointerAction::Moved, 100.),
        (PointerAction::Released, 100.),
    ] {
        let mut event = PointerEvent::mouse(Point::new(CssPx(x + dx), CssPx(y)));
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
    assert!(
        model.orbit.get_untracked().bearing < orbit.bearing,
        "Dragging right orbits in the corrected horizontal direction"
    );
    assert_eq!(model.orbit.get_untracked().pitch, orbit.pitch);
    assert_eq!(model.camera.get_untracked(), initial);
    load_map_fixture(&mut harness, model, directory.path(), false);
    pump(&mut harness, || {
        stats.scene_revision.load(Ordering::Acquire) == model.service.revision.get_untracked()
    });
    // Warm both ends of an orbit, then repeat with no I/O or GPU uploads.
    let away = model.orbit.get_untracked();
    let mut warm = None;
    for cycle in 0..3 {
        for pose in [orbit, away] {
            let frames = stats.frames.load(Ordering::Acquire);
            model.orbit.set(pose);
            harness.settle(64);
            load_map_fixture(&mut harness, model, directory.path(), false);
            pump(&mut harness, || {
                stats.frames.load(Ordering::Acquire) > frames
                    && stats.scene_revision.load(Ordering::Acquire)
                        == model.service.revision.get_untracked()
            });
        }
        let counts = (model.service.loads(), stats.uploads.load(Ordering::Acquire));
        if cycle == 1 {
            warm = Some(counts);
        }
        if cycle == 2 {
            eprintln!(
                "Warm orbit round trip: {} loads, {} uploads; GPU resident {} MiB",
                counts.0 - warm.unwrap().0,
                counts.1 - warm.unwrap().1,
                stats.resident_bytes.load(Ordering::Acquire) / 1024 / 1024
            );
            assert_eq!(
                Some(counts),
                warm,
                "Warm views must reuse CPU tiles and GPU textures"
            );
        }
    }
    for _ in 0..3 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
    }
    assert!(
        pixels.max_difference(&read_pixels(&harness)) > 10,
        "Orbit changes the projected terrain"
    );
    // Regional zooms must retain terrain across the upper viewport, beyond the
    // old fixed distance past focus. Use generated local tiles, never network.
    let near_camera = model.camera.get_untracked();
    let near_orbit = model.orbit.get_untracked();
    model.camera.set(Camera {
        zoom: 8.,
        ..near_camera
    });
    model.orbit.set(super::terrain::Orbit::default());
    harness.settle(128);
    load_map_fixture(&mut harness, model, directory.path(), false);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        let frame = read_pixels(&harness);
        let left = (region.origin.x.0 + region.size.width.0 * 0.2) as i32;
        let right = (region.origin.x.0 + region.size.width.0 * 0.8) as i32;
        let y = (region.origin.y.0 + 40.) as i32;
        let terrain = (left..right)
            .filter(|&x| {
                let p = frame.rgba(x, y);
                p[1] > p[0].saturating_add(10) && p[1] > p[2].saturating_add(10)
            })
            .count();
        if terrain > (right - left) as usize / 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Zoomed-out upper viewport must contain terrain, got {terrain} pixels"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    model.camera.set(near_camera);
    model.orbit.set(near_orbit);
    harness.settle(128);
    load_map_fixture(&mut harness, model, directory.path(), false);
    // Both supported pan gestures preserve zoom and orbit.
    let orbit_before_pan = model.orbit.get_untracked();
    for (button, modifiers) in [
        (PointerButton::Primary, Modifiers::CONTROL),
        (PointerButton::Middle, Modifiers::NONE),
    ] {
        let panned_from = model.camera.get_untracked();
        for (action, dx, dy) in [
            (PointerAction::Pressed, 0., 0.),
            (PointerAction::Moved, 20., 10.),
            (PointerAction::Released, 20., 10.),
        ] {
            let mut event = PointerEvent::mouse(Point::new(CssPx(x + dx), CssPx(y + dy)));
            event.button = matches!(action, PointerAction::Pressed | PointerAction::Released)
                .then_some(button);
            harness.deliver_to_first(SurfaceEvent::Pointer {
                action,
                event,
                modifiers,
                timestamp: Timestamp::ORIGIN,
            });
            harness.advance(Duration::from_millis(17));
            harness.settle(64);
        }
        assert_ne!(model.camera.get_untracked().center, panned_from.center);
        assert_eq!(model.camera.get_untracked().zoom, panned_from.zoom);
        assert_eq!(model.orbit.get_untracked(), orbit_before_pan);
        load_map_fixture(&mut harness, model, directory.path(), false);
        pump(&mut harness, || {
            stats.scene_revision.load(Ordering::Acquire) == model.service.revision.get_untracked()
        });
    }
    // Upward dragging retains the existing direction and reaches the new
    // grazing angle through the real input layer, with distant data loaded.
    for (action, dy) in [
        (PointerAction::Pressed, 0.),
        (PointerAction::Moved, -300.),
        (PointerAction::Released, -300.),
    ] {
        let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(y + dy)));
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
    assert_eq!(model.orbit.get_untracked().pitch, super::terrain::MIN_PITCH);
    assert_eq!(
        model.orbit.get_untracked().bearing,
        orbit_before_pan.bearing
    );
    load_map_fixture(&mut harness, model, directory.path(), false);
    pump(&mut harness, || {
        stats.scene_revision.load(Ordering::Acquire) == model.service.revision.get_untracked()
    });
    if let Some(path) = std::env::var_os("ENLUMI_MAP_TERRAIN_LOW_PREVIEW") {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        capture(&read_pixels(&harness), path);
    }
    model.maximized.set(Some(1));
    pump(&mut harness, || stats.stopped.load(Ordering::Acquire) == 1);
}

#[test]
fn terrain_decoding_preserves_numeric_rgb_and_rejects_wrong_dimensions() {
    use super::tiles::{Key, Payload, decode};
    let key = Key {
        dem: true,
        provider: super::tiles::Provider::OpenStreetMap,
        z: 0,
        x: 0,
        y: 0,
    };
    let image = image::RgbImage::from_pixel(512, 512, image::Rgb([128, 100, 128]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let Payload::Elevation(e) = decode(key, encoded.into_inner()).unwrap() else {
        panic!("Expected raw elevation")
    };
    assert!(e.heights.iter().all(|h| *h == 100.5));
    let image = image::RgbImage::new(16, 16);
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    assert!(decode(key, encoded.into_inner()).is_err());
}

#[test]
fn orbit_projection_keeps_focus_centered_and_in_depth_range() {
    let camera = Camera {
        center: project(7.6586, 45.9763).unwrap(),
        zoom: 12.,
    };
    let matrix = super::terrain::Orbit::default().matrix(camera, [800., 600.]);
    assert!(matrix.iter().all(|f| f.is_finite()));
    assert!(matrix[15] > 0.);
    assert!(matrix[12].abs() / matrix[15] < 1e-6);
    assert!(matrix[13].abs() / matrix[15] < 1e-6);
    assert!((0. ..=1.).contains(&(matrix[14] / matrix[15])));
}

#[test]
fn terrain_navigation_keeps_anchor_zoom_and_vertical_drag_direction() {
    use super::terrain::{MIN_PITCH, Orbit};
    let camera = Camera {
        center: project(7.6586, 45.9763).unwrap(),
        zoom: 12.,
    };
    let size = [800., 600.];
    for bearing in [-170., -25., 90.] {
        let orbit = Orbit {
            bearing,
            ..Orbit::default()
        };
        assert!(orbit.dragged([30., 0.]).bearing < bearing);
        assert!(orbit.dragged([0., 30.]).pitch > orbit.pitch);
        assert_eq!(orbit.dragged([0., -1000.]).pitch, MIN_PITCH);
        for pitch in [MIN_PITCH, 45., 85.] {
            let orbit = Orbit { pitch, ..orbit };
            let start = [400., 300.];
            let end = [420., 315.];
            let anchor = orbit.ground_at(camera, size, start).unwrap();
            let panned = orbit.pan(camera, size, start, end).unwrap();
            let moved_anchor = orbit.ground_at(panned, size, end).unwrap();
            assert_eq!(panned.zoom, camera.zoom);
            assert!((anchor[0] - moved_anchor[0]).abs() < 1e-6);
            assert!((anchor[1] - moved_anchor[1]).abs() < 1e-6);
            assert!(orbit.matrix(panned, size).iter().all(|f| f.is_finite()));
        }
    }
}

#[test]
fn terrain_coverage_reaches_farther_with_bounded_nonoverlapping_lod() {
    use super::terrain::{MAX_TERRAIN_TILES, MIN_PITCH, Orbit};
    let camera = Camera {
        center: project(7.6586, 45.9763).unwrap(),
        zoom: 12.,
    };
    let size = [800., 600.];
    let max_distance = |tiles: &[TileId]| {
        tiles
            .iter()
            .map(|id| {
                let span = WORLD / f64::from(1_u32 << id.z);
                let dx = -WORLD / 2. + (id.x as f64 + 0.5) * span - camera.center[0];
                let dy = WORLD / 2. - (f64::from(id.y) + 0.5) * span - camera.center[1];
                (dx * dx + dy * dy).sqrt()
            })
            .fold(0., f64::max)
    };
    let old = max_distance(&camera.map_tiles(size));
    for pitch in [MIN_PITCH, 20., 45., 85.] {
        let orbit = Orbit {
            pitch,
            ..Orbit::default()
        };
        let tiles = orbit.tiles(camera, size);
        assert!(!tiles.is_empty());
        assert!(tiles.len() <= MAX_TERRAIN_TILES);
        assert!(tiles.iter().all(|t| t.z <= 13 && t.y < 1 << t.z));
        for (i, a) in tiles.iter().enumerate() {
            for b in &tiles[i + 1..] {
                let overlaps = |a: &TileId, b: &TileId| {
                    a.z <= b.z
                        && b.x.div_euclid(1_i64 << (b.z - a.z)) == a.x
                        && b.y >> (b.z - a.z) == a.y
                };
                assert!(
                    !overlaps(a, b) && !overlaps(b, a),
                    "No duplicate ground coverage"
                );
            }
        }
        if pitch <= 20. {
            assert!(
                max_distance(&tiles) > old * 2.,
                "Lower views need distant coverage"
            );
            assert!(
                tiles
                    .iter()
                    .map(|t| t.z)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    > 1
            );
        }
    }
    let near = Orbit {
        pitch: MIN_PITCH,
        view_distance_km: 40.,
        ..Orbit::default()
    }
    .tiles(camera, size);
    let far = Orbit {
        pitch: MIN_PITCH,
        view_distance_km: 160.,
        ..Orbit::default()
    }
    .tiles(camera, size);
    assert!(max_distance(&far) > max_distance(&near) * 1.5);
    assert!(Orbit::default().tiles(camera, [0., 600.]).is_empty());
}
