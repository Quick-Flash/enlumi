use super::*;
use crate::{state::Workbench, trajectory::Spec};
use zgui::{
    geom::{DevicePx, Size},
    platform::SurfaceEvent,
};
use zgui_platform_headless::Harness;
#[test]
fn normal_3d_map_renders_bound_altitude_and_switches_modes() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let Some(graphics) = super::super::test_renderer::graphics() else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let local = directory.path().to_path_buf();
    let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
    let saved = captured.clone();
    let handler=zgui::prelude::app().with_size(1000.,700.).with_stylesheet(super::super::preview::stylesheet()).with_renderer(Box::new(move |_,target|{
        let mut renderer=graphics.renderer_offscreen(target,zgui::surface::wgpu::TextureFormat::Bgra8Unorm,false)?;
        zgui_render_vector_vello::attach(&mut renderer,target.size);Ok(Box::new(super::super::test_renderer::ReadbackRenderer(renderer)))
    })).into_handler(move ||{
        let workbench=Workbench::new();workbench.timeline.time.set(None);let pane=workbench.pane();
        let columns=[("lon",7.65,0.001),("lat",45.97,0.001),("alt",1000.,0.)].into_iter().enumerate().map(|(index,(source,start,step))|crate::state::Line{id:index as u64+1,source:source.into(),name:source.into(),revision:0,data:plot::time_series::SeriesData::new((0..9).map(|i|plot::PlotPoint{x:i as f64,y:start+step*if source=="lat" && i>4 {(8-i) as f64}else{i as f64}}).collect::<Vec<_>>()).unwrap()}).collect::<Vec<_>>();
        let spec=Spec{x:"lon".into(),y:"lat".into(),gps_scale:Some(1.),map:Some(super::super::flight::Config{terrain:true,..Default::default()}),altitudes:BTreeMap::from([(0,Altitude{source:"alt".into(),..Default::default()})]),..Default::default()};
        let data=Arc::new(Tracks::prepare(&columns,&spec).unwrap());
        pane.lines.set(columns);pane.trajectory.set(Some(spec));pane.trajectory_data.set(Some(data));
        let mut style=pane.plot.style("2");style.color=[250,40,180,255];style.width=2.;pane.plot.styles.update(|styles|{styles.insert("2".into(),style);});
        let service=super::super::tiles::Service::new(super::super::tiles::Config{cache:local.join("cache"),local:Some(local.clone()),network:false});workbench.map_tiles.set_value(Some(service));
        workbench.edit_trajectory(pane);
        *saved.borrow_mut()=Some((workbench,pane,service));
        use super::super::flight::FlightMapProps;
        use super::super::settings::MapSettingsProps;
        view!{column(class="page workbench"){row(class="body"){column(node_ref=pane.dock.node,style:flex="1",style:min_width="0"){FlightMap(workbench=workbench,pane=pane,visible=Signal::stored_local(true))} column(style:width="300px"){MapSettings(workbench=workbench,pane=workbench.trajectory_builder().pane)}}}}
    }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1000.),
        DevicePx(700.),
    )));
    harness.settle(128);
    let (workbench, pane, service) = captured.borrow().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if super::super::tests::map_fixture(directory.path(), &service.requested())
            || service.failed()
        {
            service.retry();
        }
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        let pixels = harness.app().windows()[0]
            .renderer()
            .as_any()
            .unwrap()
            .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
            .unwrap()
            .read_composed();
        let colored = (100..600)
            .step_by(2)
            .flat_map(|y| (30..970).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pixels.rgba(x, y);
                p[0] > 180 && p[1] < 90 && p[2] > 100
            })
            .count();
        if (colored > 10 || std::time::Instant::now() >= deadline)
            && let Some(path) = std::env::var_os("ENLUMI_MAP_3D_PREVIEW")
        {
            super::super::tests::capture(&pixels, path);
        }
        if colored > 10 && service.complete() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "3D map must render bound altitude trace, not only terrain; {}",
            service.status()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let read = |h: &Harness<zgui::runtime::Runtime>| {
        h.app().windows()[0]
            .renderer()
            .as_any()
            .unwrap()
            .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
            .unwrap()
            .read_composed()
    };
    // Wait for the latest complete tile scene, then import that frame before
    // using exact pixel counts as the baseline for style comparisons.
    workbench.capture_screenshot(Some(pane)).unwrap();
    harness.advance(std::time::Duration::from_millis(17));
    harness.settle(128);
    let legend = harness.app().windows()[0]
        .box_with_class("trajectory-plot-controls")
        .unwrap();
    let surface = harness.app().windows()[0]
        .box_with_class("map-terrain-surface")
        .unwrap();
    let count_magenta = |pixels: &zgui_render_wgpu::Pixels| {
        (0..700)
            .flat_map(|y| (0..700).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (px, py) = (x as f32, y as f32);
                let inside = |rect: zgui::geom::Rect<zgui::geom::DevicePx, zgui::geom::Device>| {
                    px >= rect.origin.x.0
                        && px < rect.origin.x.0 + rect.size.width.0
                        && py >= rect.origin.y.0
                        && py < rect.origin.y.0 + rect.size.height.0
                };
                if !inside(surface) || inside(legend) {
                    return false;
                }
                let p = pixels.rgba(x, y);
                p[0] > 180 && p[1] < 90 && p[2] > 100
            })
            .count()
    };
    let narrow = count_magenta(&read(&harness));
    let mut previous = narrow;
    for pattern in [
        plot::line_draw::LinePattern::Dashed,
        plot::line_draw::LinePattern::Dotted,
        plot::line_draw::LinePattern::DashDot,
        plot::line_draw::LinePattern::Solid,
    ] {
        pane.drawing.update(|drawing| {
            drawing.entry(2).or_default().pattern = pattern;
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            harness.advance(std::time::Duration::from_millis(17));
            harness.settle(64);
            let count = count_magenta(&read(&harness));
            let visible = if pattern == plot::line_draw::LinePattern::Solid {
                count == narrow
            } else {
                count > 10 && count < narrow * 95 / 100 && count.abs_diff(previous) > 10
            };
            if visible {
                previous = count;
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Changing 3D pattern to {pattern:?} must change rasterized stroke: {count}, solid {narrow}, previous {previous}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    pane.plot.styles.update(|styles| {
        styles.get_mut("2").unwrap().width = 10.;
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        if count_magenta(&read(&harness)) > narrow * 2 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Changing 3D width must increase rasterized stroke area"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let wide = count_magenta(&read(&harness));
    pane.plot.styles.update(|styles| {
        let style = styles.get_mut("2").unwrap();
        style.radius = 24.;
        style.marker = plot::plot_data::MarkerShape::Square;
    });
    pane.drawing.update(|drawing| {
        drawing.insert(
            2,
            plot::line_draw::LineDraw {
                markers: true,
                pattern: plot::line_draw::LinePattern::None,
                ..Default::default()
            },
        );
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        if count_magenta(&read(&harness)) > wide * 2 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Sample markers must render with line disabled and selected radius"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    pane.drawing
        .update(|drawing| drawing.get_mut(&2).unwrap().markers = false);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        if count_magenta(&read(&harness)) == 0 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Disabling markers and line must hide sample geometry"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    pane.drawing.set(Default::default());
    workbench.timeline.time.set(Some(4.));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    let timeline_center = loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        let pixels = read(&harness);
        let yellow = |x: i32, y: i32| {
            let p = pixels.rgba(x, y);
            p[0] > 230 && p[1] > 170 && p[2] < 100
        };
        let points = (0..700)
            .flat_map(|y| (0..700).map(move |x| (x, y)))
            .filter(|&(x, y)| yellow(x, y))
            .collect::<Vec<_>>();
        if points.len() > 30 {
            let x = points.iter().map(|p| p.0).sum::<i32>() / points.len() as i32;
            let y = points.iter().map(|p| p.1).sum::<i32>() / points.len() as i32;
            let filled = (-2..=2)
                .flat_map(|dy| (-2..=2).map(move |dx| (x + dx, y + dy)))
                .filter(|&(x, y)| yellow(x, y))
                .count();
            assert!(
                filled >= 20,
                "Timeline marker must have filled center, not thin crossing strokes"
            );
            let white = (-8..=8)
                .flat_map(|dy| (-8..=8).map(move |dx| (x + dx, y + dy)))
                .filter(|&(x, y)| {
                    let p = pixels.rgba(x, y);
                    p[0] > 230 && p[1] > 230 && p[2] > 230
                })
                .count();
            assert!(white > 8, "Timeline dot must have visible white border");
            break (x, y);
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Timeline movement must render circular marker"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    workbench.timeline.time.set(None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        let pixels = read(&harness);
        let filled = (-3..=3)
            .flat_map(|dy| (-3..=3).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx * dx + dy * dy <= 9)
            .filter(|&(dx, dy)| {
                let p = pixels.rgba(timeline_center.0 + dx, timeline_center.1 + dy);
                p[0] > 180 && p[1] < 90 && p[2] > 100
            })
            .count();
        if filled == 29 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Turn join must fill inner stroke disk, got {filled}/29 pixels"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    {
        use zgui::{
            geom::{CssPx, Point},
            vocab::{Modifiers, PointerAction, PointerEvent, Timestamp},
        };
        let cursor = Point::new(
            CssPx(timeline_center.0 as f32 + 3.),
            CssPx(timeline_center.1 as f32 + 2.),
        );
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action: PointerAction::Moved,
            event: PointerEvent::mouse(cursor),
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(64);
        let window = &harness.app().windows()[0];
        let nodes = window.nodes_with_class("map-hover-tooltip");
        assert_eq!(nodes.len(), 1, "3D hover must show snapped sample");
        let label = window.dom().text_content(nodes[0]);
        assert!(
            label.contains("t: 4.000 s")
                && label.contains("Z: 1000.00 m (relative)")
                && label.contains("Elevation:"),
            "{label}"
        );
        assert_eq!(
            workbench.timeline.time.get_untracked(),
            None,
            "Hover leaves pinned timeline unchanged"
        );
        if std::env::var_os("ENLUMI_MAP_HOVER_PREVIEW").is_some() {
            super::super::tests::capture(&read(&harness), "target/map-hover-3d.ppm");
        }
        harness.deliver_to_first(SurfaceEvent::Pointer {
            action: PointerAction::Moved,
            event: PointerEvent::mouse(Point::new(CssPx(2.), CssPx(2.))),
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(64);
        assert!(
            harness.app().windows()[0]
                .box_with_class("map-hover-tooltip")
                .is_none(),
            "Moving away clears snapped location"
        );
    }
    workbench.timeline.time.set(Some(4.));
    harness.settle(64);
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-terrain-surface")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-sidebar-settings")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("flight-map-footer")
            .is_none()
    );
    let loads = service.loads();
    let svg = workbench.capture_screenshot(Some(pane)).unwrap();
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.contains("Mapterhorn"));
    assert!(svg.contains("OpenStreetMap contributors"));
    let png = plot::zgui_svg::ScreenshotFormat::Png.encode(&svg).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert!(
        image
            .pixels()
            .filter(|pixel| pixel[0] > 180 && pixel[1] < 90 && pixel[2] > 100)
            .count()
            > 30,
        "PNG screenshot must contain 3D trace pixels"
    );
    let gold = image
        .pixels()
        .filter(|pixel| pixel[0] > 230 && pixel[1] > 170 && pixel[2] < 100)
        .count();
    if gold <= 30 {
        std::fs::write("target/3d-export-check.png", &png).unwrap();
        std::fs::write("target/3d-export-check.svg", &svg).unwrap();
        let data = svg
            .split("data:image/png;base64,")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let source =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data).unwrap();
        std::fs::write("target/3d-export-source.png", &source).unwrap();
        let source = image::load_from_memory(&source).unwrap().to_rgba8();
        let source_gold = source
            .pixels()
            .filter(|pixel| pixel[0] > 230 && pixel[1] > 170 && pixel[2] < 100)
            .count();
        eprintln!(
            "Export {}x{} gold={gold}; source {}x{} gold={source_gold}",
            image.width(),
            image.height(),
            source.width(),
            source.height()
        );
    }
    assert!(gold > 30, "Screenshot must preserve timeline dot");
    let docks = workbench.capture_screenshot(None).unwrap();
    assert!(docks.contains("data:image/png;base64,"));
    plot::zgui_svg::ScreenshotFormat::Png
        .encode(&docks)
        .unwrap();
    assert_eq!(
        service.loads(),
        loads,
        "Screenshot capture must not demand more tiles"
    );
    let mut appearance = crate::trace_color::TraceColor {
        mode: crate::trace_color::Mode::Height,
        ..Default::default()
    };
    appearance.height.range = [0., 2000.];
    appearance.height.colors = [[0, 255, 0], [0, 0, 255]];
    pane.trace_colors.update(|colors| {
        colors.insert(2, appearance);
    });
    let [red, green, blue] = appearance.height.color(32. / 63.);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        let frame = read(&harness);
        let colored = (80..550)
            .flat_map(|y| (20..680).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = frame.rgba(x, y);
                p[0].abs_diff(red) < 8 && p[1].abs_diff(green) < 8 && p[2].abs_diff(blue) < 8
            })
            .count();
        if colored > 10 && count_magenta(&frame) == 0 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "3D trace must use chosen height gradient, got {colored} colored pixels"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let before_projection = read(&harness);
    super::super::settings::update(workbench, pane, |spec| {
        spec.map.as_mut().unwrap().ground_projection = true
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        harness.advance(std::time::Duration::from_millis(17));
        harness.settle(64);
        if before_projection.max_difference(&read(&harness)) > 10 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Ground projection must change rendered terrain pixels"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    if let Some(path) = std::env::var_os("ENLUMI_MAP_UI_PREVIEW") {
        let frame = read(&harness);
        let size = frame.size();
        image::RgbaImage::from_fn(size.width as u32, size.height as u32, |x, y| {
            image::Rgba(frame.rgba(x as i32, y as i32))
        })
        .save(path)
        .unwrap();
    }
    let colored_svg = workbench.capture_screenshot(Some(pane)).unwrap();
    assert!(
        colored_svg.contains("height"),
        "3D export must include color scale"
    );
    super::super::settings::update(workbench, pane, |s| {
        s.map.as_mut().unwrap().set_terrain(false)
    });
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-terrain-surface")
            .is_none()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("map-tiles")
            .is_some()
    );
    let svg = workbench.capture_screenshot(Some(pane)).unwrap();
    assert!(svg.contains("OpenStreetMap contributors"));
    assert!(
        !svg.contains("Mapterhorn"),
        "Switching to 2D must restore 2D snapshot source"
    );
}
