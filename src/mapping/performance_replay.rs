//! Opt-in real GPU replay: measures complete surface frames, not projection alone.
use super::{
    camera::{Camera, TileId},
    flight3::Path3,
    gpu::Producer,
    terrain::{Orbit, Scene, Tile},
    tiles::{Elevation, Payload, Provider, Raster},
};
use std::{
    cell::RefCell,
    io::Write,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use zgui::{
    geom::{DevicePx, Size},
    platform::SurfaceEvent,
    prelude::*,
};
use zgui_platform_headless::Harness;

fn scene(count: usize, samples: usize, ground: bool) -> Scene {
    let (lines, spec, tracks) = super::telemetry::tests::fixture();
    let template = super::flight3::prepare(
        &tracks,
        &lines,
        &std::collections::BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    let paths = (0..count)
        .map(|i| {
            let mut path: Path3 = template[0].clone();
            path.id = i as u64;
            path.points = (0..samples)
                .map(|n| {
                    let angle = n as f64 / (samples - 1) as f64 * std::f64::consts::TAU;
                    [
                        300. * angle.cos(),
                        300. * angle.sin(),
                        150. + 50. * (angle * 3.).sin() + i as f64 * 15.,
                    ]
                })
                .collect::<Vec<_>>()
                .into();
            path.times = (0..samples)
                .map(|n| n as f64 * 0.02)
                .collect::<Vec<_>>()
                .into();
            path
        })
        .collect::<Vec<_>>();
    let raster = Arc::new(Payload::Raster(Raster {
        version: 1,
        width: 256,
        height: 256,
        encoded: vec![],
        rgba: [90, 150, 80, 255].repeat(256 * 256),
    }));
    let dem = Arc::new(Payload::Elevation(Elevation {
        version: 1,
        width: 2,
        height: 2,
        heights: vec![0.; 4],
    }));
    let mut scene = Scene::default();
    scene.paths = paths.into();
    scene.ground_projection = ground;
    scene.tiles = (0..9)
        .map(|i| {
            let id = TileId {
                z: 14,
                x: 8191 + i % 3,
                y: 8191 + (i / 3) as u32,
            };
            Tile {
                id,
                dem_id: id,
                raster_id: id,
                provider: Provider::OpenStreetMap,
                raster: Some(raster.clone()),
                elevation: Some(dem.clone()),
            }
        })
        .collect();
    scene
}
#[test]
#[ignore = "real GPU 3D interaction replay; writes target/map3d-replay.csv"]
fn replay_3d_interactions() {
    let Some(graphics) = super::test_renderer::graphics() else {
        return;
    };
    let gpu = graphics.open_gpu().unwrap();
    let saved = Rc::new(RefCell::new(None));
    let captured = saved.clone();
    let handler = app()
        .with_size(1000., 700.)
        .with_stylesheet(".map3d-replay {width:100%;height:100%;flex:1;}")
        .with_renderer(Box::new(move |_, target| {
            let renderer = graphics.renderer_offscreen(
                target,
                zgui::surface::wgpu::TextureFormat::Bgra8Unorm,
                false,
            )?;
            Ok(Box::new(super::test_renderer::ReadbackRenderer(renderer)))
        }))
        .into_handler(move || {
            let producer = Rc::new(Producer::new());
            let surface = zgui::elements::surface()
                .class("map3d-replay")
                .source(&producer.handle)
                .into_view();
            *captured.borrow_mut() = Some(producer);
            view! {column(style:width="100%",style:height="100%"){ {surface} }}
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1000.),
        DevicePx(700.),
    )));
    harness.settle(64);
    let viewport = harness.app().windows()[0]
        .box_with_class("map3d-replay")
        .unwrap();
    assert!(
        viewport.size.width.0 > 900. && viewport.size.height.0 > 600.,
        "Replay surface must fill viewport: {viewport:?}"
    );
    let producer = saved.borrow().as_ref().unwrap().clone();
    let stats = &producer.stats;
    let output = std::env::var_os("ENLUMI_3D_REPLAY_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/map3d-replay.csv".into());
    let mut file = std::fs::File::create(output).unwrap();
    writeln!(file,"traces,samples,ground,action,frame,ready_ms,prepare_ms,gpu_wait_ms,trace_uploads,trace_bytes,ground_builds").unwrap();
    for count in [1, 4] {
        for ground in [false, true] {
            let base = Arc::new(scene(count, 20_000, ground));
            for action in ["orbit", "pan", "zoom", "timeline"] {
                let mut times = Vec::new();
                for frame in 0..24 {
                    let mut camera = Camera {
                        center: [0., 0.],
                        zoom: 16.,
                    };
                    let mut orbit = Orbit::default();
                    let mut current = base.clone();
                    match action {
                        "orbit" => orbit.bearing += frame as f32 * 0.5,
                        "pan" => camera.center[0] = frame as f64 * 2.,
                        "zoom" => camera.zoom += frame as f64 * 0.01,
                        _ => {
                            let mut scene = (*base).clone();
                            scene.time = Some(frame as f64);
                            current = Arc::new(scene);
                        }
                    }
                    let before = (
                        stats.prepare_ns.load(Ordering::Acquire),
                        stats.wait_ns.load(Ordering::Acquire),
                        stats.trace_uploads.load(Ordering::Acquire),
                        stats.trace_bytes.load(Ordering::Acquire),
                        stats.ground_builds.load(Ordering::Acquire),
                    );
                    let start = Instant::now();
                    producer.terrain(camera, orbit, current);
                    let requested = stats.requested_revision.load(Ordering::Acquire);
                    harness.advance(Duration::from_millis(17));
                    while stats.presented_revision.load(Ordering::Acquire) < requested {
                        assert!(start.elapsed() < Duration::from_secs(10));
                        std::thread::sleep(Duration::from_millis(1));
                        harness.advance(Duration::from_millis(17));
                    }
                    harness.advance(Duration::ZERO);
                    gpu.wait();
                    let ready = start.elapsed().as_secs_f64() * 1000.;
                    let prep = (stats.prepare_ns.load(Ordering::Acquire) - before.0) as f64 / 1e6;
                    let wait = (stats.wait_ns.load(Ordering::Acquire) - before.1) as f64 / 1e6;
                    if frame >= 4 {
                        times.push(ready);
                        writeln!(file,"{count},20000,{ground},{action},{frame},{ready:.3},{prep:.3},{wait:.3},{},{},{}",stats.trace_uploads.load(Ordering::Acquire)-before.2,stats.trace_bytes.load(Ordering::Acquire)-before.3,stats.ground_builds.load(Ordering::Acquire)-before.4).unwrap();
                    }
                }
                times.sort_by(f64::total_cmp);
                eprintln!(
                    "{count} traces, ground={ground}, {action}: ready p50={:.2}ms p95={:.2}ms",
                    times[times.len() / 2],
                    times[times.len() * 95 / 100]
                );
            }
        }
    }
}
