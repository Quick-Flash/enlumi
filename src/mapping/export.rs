//! Map snapshots use the normal plot export, clipboard and workspace contracts.
use super::{
    camera::Camera,
    tiles::{Key, Payload, Provider, Service},
};
use crate::state::Pane;
use base64::{Engine, engine::general_purpose::STANDARD};
use zgui::{elements::canvas::CanvasHandle, prelude::*};

#[derive(Clone, Copy)]
struct Canvases(StoredValue<Option<(CanvasHandle, CanvasHandle)>, LocalStorage>);

#[derive(Clone, Copy)]
struct Surface(StoredValue<Option<super::gpu::Capture>, LocalStorage>);
pub(crate) fn register_surface(capture: super::gpu::Capture) {
    if let Some(surface) = use_local_context::<Surface>() {
        surface.0.set_value(Some(capture.clone()));
        on_cleanup_local(move || {
            surface.0.try_update_value(|current| {
                if current
                    .as_ref()
                    .is_some_and(|current| current.same(&capture))
                {
                    *current = None;
                }
            });
        });
    }
}
pub(crate) fn register_canvases(path: CanvasHandle, markers: CanvasHandle) {
    if let Some(capture) = use_local_context::<Canvases>() {
        capture.0.set_value(Some((path, markers)));
        on_cleanup_local(move || {
            capture.0.try_update_value(|value| *value = None);
        });
    }
}

pub(crate) fn install(
    workbench: crate::state::Workbench,
    pane: Pane,
    host: NodeRef,
    camera: RwSignal<Camera, LocalStorage>,
    provider: RwSignal<Provider, LocalStorage>,
    service: Service,
    colors: Signal<std::sync::Arc<super::telemetry::Coloring>, LocalStorage>,
) {
    let canvases = Canvases(StoredValue::new_local(None));
    provide_local_context(canvases);
    let surface = Surface(StoredValue::new_local(None));
    provide_local_context(surface);
    plot::zgui_svg::register_snapshot(pane.snapshot, move || {
        let _zone = zgui::reactive::enter_non_reactive_zone();
        let rect = plot::zgui_svg::node_rect(host).ok_or("Map is not ready to export")?;
        let [w, h] = [rect.width, rect.height];
        if pane
            .trajectory
            .with_untracked(|s| s.as_ref().is_some_and(|s| s.map.is_some_and(|m| m.terrain)))
        {
            let frame = surface
                .0
                .get_value()
                .ok_or("3D map has not rendered a frame yet")?
                .png()?;
            let background = pane.background.get_untracked().map_or_else(
                || "#12181e".into(),
                |[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"),
            );
            let mut description = plot::PlotDescription::new(w, h, &background);
            crate::trajectory_plot::describe_legend(workbench, pane, &mut description);
            let mut svg = plot::zgui_svg::scenes_to_svg(&[], &description)?;
            let tag = format!(
                "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"{background}\"/>"
            );
            let index = svg
                .find(&tag)
                .ok_or("Map export background was not generated")?
                + tag.len();
            svg.insert_str(index,&format!("<image x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{}\"/>",STANDARD.encode(&frame.png)));
            if let Some(scale) = frame.metres_per_pixel {
                let index = svg
                    .rfind("</svg>")
                    .ok_or("Map export SVG was not generated")?;
                svg.insert_str(index, &super::scale::svg_rulers(scale, h));
            }
            if let Some(index) = svg.rfind("</svg>") {
                svg.insert_str(
                    index,
                    &super::color_legend::svg(&colors.get_untracked(), w, h),
                );
            }
            let mut sources = frame
                .providers
                .iter()
                .map(|provider| provider.attribution())
                .collect::<Vec<_>>();
            if sources.is_empty() {
                sources.push(provider.get_untracked().attribution());
            }
            sources.push("Terrain: (c) Mapterhorn / source datasets");
            let attribution = sources.join(" | ");
            let total = h + 26.;
            let svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{total}\" viewBox=\"0 0 {w} {total}\">{svg}<rect x=\"0\" y=\"{h}\" width=\"{w}\" height=\"26\" fill=\"{background}\"/><text x=\"8\" y=\"{}\" font-family=\"sans-serif\" font-size=\"11\" fill=\"white\">{attribution}</text></svg>",
                h + 18.
            );
            return Ok(plot::zgui_svg::PlotSnapshot {
                svg: plot::zgui_svg::SvgPair {
                    clean: svg.clone(),
                    with_overlay: svg,
                },
                node: host,
                width: w,
                height: total,
            });
        }
        let (path, markers) = canvases
            .0
            .get_value()
            .ok_or("Map paths are not ready to export")?;
        let scenes = plot::zgui_svg::snapshot_canvas_handles(&[&path, &markers])?;
        let camera = camera.get_untracked();
        let background = pane.background.get_untracked().map_or_else(
            || "#12181e".into(),
            |[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"),
        );
        let mut description = plot::PlotDescription::new(w, h, &background);
        crate::trajectory_plot::describe_legend(workbench, pane, &mut description);
        let selected_provider = provider.get_untracked();
        let images = tile_images(camera, [w, h], service, pane.id, selected_provider)?;
        let attribution = selected_provider.attribution();
        let render = |scenes: &[&zgui::canvas::CanvasScene]| -> Result<String, String> {
            let mut svg = plot::zgui_svg::scenes_to_svg(scenes, &description)?;
            let background_tag = format!(
                "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"{background}\"/>"
            );
            let index = svg
                .find(&background_tag)
                .ok_or("Map export background was not generated")?
                + background_tag.len();
            svg.insert_str(index, &images);
            let ruler =
                super::scale::svg_rulers(super::terrain::ground_scale(camera) / camera.scale(), h);
            let index = svg
                .rfind("</svg>")
                .ok_or("Map export SVG was not generated")?;
            svg.insert_str(index, &ruler);
            if let Some(index) = svg.rfind("</svg>") {
                svg.insert_str(
                    index,
                    &super::color_legend::svg(&colors.get_untracked(), w, h),
                );
            }
            let total = h + 26.;
            Ok(format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{total}\" viewBox=\"0 0 {w} {total}\">{svg}<rect x=\"0\" y=\"{h}\" width=\"{w}\" height=\"26\" fill=\"{background}\"/><text x=\"8\" y=\"{footer}\" font-family=\"sans-serif\" font-size=\"11\" fill=\"white\">{attribution}</text></svg>",
                footer = h + 18.
            ))
        };
        let clean = render(&[&scenes[0]])?;
        let with_overlay = render(&[&scenes[0], &scenes[1]])?;
        Ok(plot::zgui_svg::PlotSnapshot {
            svg: plot::zgui_svg::SvgPair {
                clean,
                with_overlay,
            },
            node: host,
            width: w,
            height: h + 26.,
        })
    });
}

fn tile_images(
    camera: Camera,
    size: [f64; 2],
    service: Service,
    _id: u64,
    provider: Provider,
) -> Result<String, String> {
    let scale = (4_194_304. / (size[0] * size[1])).sqrt().min(2.);
    let width = (size[0] * scale).ceil().max(1.) as u32;
    let height = (size[1] * scale).ceil().max(1.) as u32;
    let mut output = image::RgbaImage::new(width, height);
    for tile in provider.expand_tiles(camera.map_tiles(size)) {
        let mut layers = Vec::new();
        let mut source = tile;
        loop {
            if let Some(payload) = service.payload(Key::raster_from(source, provider))
                && matches!(payload.as_ref(), Payload::Raster(_))
            {
                layers.push((camera.place(source, size), payload));
            }
            if source.z == 0 {
                break;
            }
            source = super::camera::TileId {
                z: source.z - 1,
                x: source.x.div_euclid(2),
                y: source.y / 2,
            };
        }
        if layers.is_empty() {
            continue;
        }
        let [left, top, tile_scale] = camera.place(tile, size);
        let x0 = (left * scale).floor().max(0.).min(f64::from(width)) as u32;
        let y0 = (top * scale).floor().max(0.).min(f64::from(height)) as u32;
        let x1 = ((left + 256. * tile_scale) * scale)
            .ceil()
            .max(0.)
            .min(f64::from(width)) as u32;
        let y1 = ((top + 256. * tile_scale) * scale)
            .ceil()
            .max(0.)
            .min(f64::from(height)) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let mut color = [0.; 4];
                let mut remaining = 1.;
                for ([sx, sy, zoom], payload) in &layers {
                    let Payload::Raster(raster) = payload.as_ref() else {
                        continue;
                    };
                    let px = (((f64::from(x) + 0.5) / scale - sx) / (256. * zoom)
                        * f64::from(raster.width)
                        - 0.5)
                        .clamp(0., f64::from(raster.width - 1));
                    let py = (((f64::from(y) + 0.5) / scale - sy) / (256. * zoom)
                        * f64::from(raster.height)
                        - 0.5)
                        .clamp(0., f64::from(raster.height - 1));
                    let ax = px.floor() as u32;
                    let ay = py.floor() as u32;
                    let bx = (ax + 1).min(raster.width - 1);
                    let by = (ay + 1).min(raster.height - 1);
                    let sample = |channel: usize| {
                        let get = |x: u32, y: u32| {
                            f64::from(raster.rgba[((y * raster.width + x) * 4) as usize + channel])
                        };
                        let top = get(ax, ay) * (1. - px.fract()) + get(bx, ay) * px.fract();
                        let bottom = get(ax, by) * (1. - px.fract()) + get(bx, by) * px.fract();
                        top * (1. - py.fract()) + bottom * py.fract()
                    };
                    let alpha = sample(3) / 255.;
                    let weight = remaining * alpha;
                    for (channel, value) in color[..3].iter_mut().enumerate() {
                        *value += sample(channel) * weight;
                    }
                    color[3] += weight;
                    remaining *= 1. - alpha;
                    if remaining < 0.0001 {
                        break;
                    }
                }
                if color[3] > 0. {
                    output.put_pixel(
                        x,
                        y,
                        image::Rgba([
                            (color[0] / color[3]).round() as u8,
                            (color[1] / color[3]).round() as u8,
                            (color[2] / color[3]).round() as u8,
                            (color[3] * 255.).round() as u8,
                        ]),
                    );
                }
            }
        }
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(output)
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "<image x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{}\"/>",
        size[0],
        size[1],
        STANDARD.encode(png.into_inner())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jpeg_provider_fallback_tiles_export_without_network_requests() {
        let _zone = zgui::reactive::enter_non_reactive_zone();
        let window = zgui_testkit_view::Window::open();
        let directory = tempfile::tempdir().unwrap();
        let provider = Provider::BingSatellite;
        let roots = provider.expand_tiles(vec![super::super::camera::TileId { z: 0, x: 0, y: 0 }]);
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            256,
            256,
            image::Rgb([35, 80, 160]),
        ))
        .write_to(&mut encoded, image::ImageFormat::Jpeg)
        .unwrap();
        for tile in &roots {
            let path = directory
                .path()
                .join("bing-satellite/1")
                .join(tile.x.to_string())
                .join(format!("{}.png", tile.y));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, encoded.get_ref()).unwrap();
        }
        let service = window.scope.with(|| {
            Service::new(super::super::tiles::Config {
                cache: directory.path().join("cache"),
                local: Some(directory.path().to_path_buf()),
                network: false,
            })
        });
        service.demand(1, &roots, false, provider, false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !service.complete() {
            window.frame();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let loads = service.loads();
        let tiles = tile_images(
            Camera {
                center: [0., 0.],
                zoom: 3.,
            },
            [256., 256.],
            service,
            1,
            provider,
        )
        .unwrap();
        assert_eq!(tiles.matches("data:image/png;base64,").count(), 1);
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\">{tiles}</svg>"
        );
        let png = plot::zgui_svg::ScreenshotFormat::Png.encode(&svg).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        let pixel = image.get_pixel(128, 128);
        assert!(
            pixel[0].abs_diff(35) < 4 && pixel[1].abs_diff(80) < 4 && pixel[2].abs_diff(160) < 4
        );
        assert_eq!(
            service.loads(),
            loads,
            "Export must not request missing detailed tiles"
        );
    }
}
