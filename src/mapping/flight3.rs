//! Arbitrary coordinate traces with independently aligned altitude samples.
use super::{
    camera::{Camera, TileId, WORLD},
    terrain::Scene,
};
use crate::{
    state::{Pane, Workbench},
    trajectory::{Altitude, Tracks},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zgui::{prelude::*, reactive::RenderEffect};
#[derive(Clone)]
pub struct Path3 {
    pub id: u64,
    pub origin: [f64; 2],
    pub points: Arc<[[f64; 3]]>,
    pub times: Arc<[f64]>,
    pub color: [u8; 4],
    pub width: f32,
    pub drawing: plot::line_draw::LineDraw,
    pub radius: f32,
    pub marker: plot::plot_data::MarkerShape,
    pub local: bool,
}
pub type Pick = zgui::reactive::UnsyncCallback<
    (
        Arc<Scene>,
        Camera,
        super::terrain::Orbit,
        [f64; 2],
        [f64; 2],
    ),
    (),
>;
impl Path3 {
    pub fn at(&self, time: f64) -> Option<[f64; 3]> {
        let right = self.times.partition_point(|value| *value <= time);
        let index = right.checked_sub(1)?;
        let a = *self.points.get(index)?;
        if !a.iter().all(|v| v.is_finite()) {
            return None;
        }
        if self.times[index] == time {
            return Some(a);
        }
        let b = *self.points.get(right)?;
        let dt = self.times[right] - self.times[index];
        if dt <= 0. || !b.iter().all(|v| v.is_finite()) {
            return None;
        }
        let t = (time - self.times[index]) / dt;
        Some(std::array::from_fn(|axis| {
            a[axis] + (b[axis] - a[axis]) * t
        }))
    }
}
pub fn pick_time(
    scene: &Scene,
    camera: Camera,
    orbit: super::terrain::Orbit,
    size: [f64; 2],
    cursor: [f64; 2],
) -> Option<f64> {
    let projection = super::hover::Projection::new(scene, camera, orbit, size);
    let mut best = (f64::INFINITY, None);
    for path in scene.paths.iter() {
        let Some(origin) = scene.path_base(path) else {
            continue;
        };
        let project = |point| projection.point(point, origin);
        for (index, pair) in path.points.windows(2).enumerate() {
            let (Some((a, wa)), Some((b, wb))) = (project(pair[0]), project(pair[1])) else {
                continue;
            };
            let d = [b[0] - a[0], b[1] - a[1]];
            let length = d[0] * d[0] + d[1] * d[1];
            let t = if length > 0. {
                ((cursor[0] - a[0]) * d[0] + (cursor[1] - a[1]) * d[1]) / length
            } else {
                0.
            }
            .clamp(0., 1.);
            let distance =
                (cursor[0] - a[0] - t * d[0]).powi(2) + (cursor[1] - a[1] - t * d[1]).powi(2);
            if distance < best.0 && distance < (f64::from(path.width) * 0.5 + 4.).max(10.).powi(2) {
                let fraction = (t / wb) / ((1. - t) / wa + t / wb);
                best = (
                    distance,
                    Some(
                        path.times[index] + (path.times[index + 1] - path.times[index]) * fraction,
                    ),
                );
            }
        }
    }
    best.1
}
pub(crate) fn vertices(scene: &Scene, camera: Camera, exaggeration: f32) -> Vec<[f32; 9]> {
    let scale = super::terrain::ground_scale(camera);
    let base = f64::from(scene.center_height(camera));
    let mut vertices = Vec::new();
    for path in scene.paths.iter() {
        let Some(origin) = scene.path_base(path) else {
            continue;
        };
        let vertex = |point: [f64; 3], color: [u8; 4]| {
            [
                ((point[0] - camera.center[0]) * scale) as f32,
                ((point[2] + f64::from(origin) - base) * f64::from(exaggeration) + 0.5) as f32,
                ((camera.center[1] - point[1]) * scale) as f32,
                f32::from(color[0]) / 255.,
                f32::from(color[1]) / 255.,
                f32::from(color[2]) / 255.,
                f32::from(color[3]) / 255.,
                path.width,
                0.,
            ]
        };
        if path.drawing.pattern != plot::line_draw::LinePattern::None {
            for pair in path.points.windows(2) {
                if pair.iter().flatten().all(|v| v.is_finite()) {
                    vertices.extend(pair.iter().map(|&point| vertex(point, path.color)));
                }
            }
        }
        if path.drawing.markers {
            for &point in path
                .points
                .iter()
                .filter(|p| p.iter().all(|v| v.is_finite()))
            {
                let mut marker = vertex(point, path.color);
                marker[7] = path.radius * 2.;
                marker[8] = match path.marker {
                    plot::plot_data::MarkerShape::Circle => 1.,
                    plot::plot_data::MarkerShape::Square => 2.,
                    plot::plot_data::MarkerShape::Triangle => 3.,
                    plot::plot_data::MarkerShape::Diamond => 4.,
                    plot::plot_data::MarkerShape::Cross => 5.,
                };
                vertices.extend([marker, marker]);
            }
        }
        let first = path
            .points
            .iter()
            .find(|p| p.iter().all(|v| v.is_finite()))
            .copied();
        let last = path
            .points
            .iter()
            .rev()
            .find(|p| p.iter().all(|v| v.is_finite()))
            .copied();
        let current = scene.time.and_then(|time| path.at(time));
        let size = 5. * scale / camera.scale();
        for (point, color) in [(first, [40, 230, 110, 255]), (last, [240, 75, 65, 255])] {
            if let Some(point) = point {
                for axis in 0..3 {
                    let mut a = point;
                    let mut b = point;
                    a[axis] -= size;
                    b[axis] += size;
                    let mut a = vertex(a, color);
                    let mut b = vertex(b, color);
                    a[7] = 2.;
                    b[7] = 2.;
                    vertices.extend([a, b]);
                }
            }
        }
        if let Some(point) = current {
            let mut marker = vertex(point, [255, 215, 60, 255]);
            marker[7] = 10.;
            marker[8] = 1.;
            vertices.extend([marker, marker]);
        }
    }
    vertices
}
pub fn anchor_tile(origin: [f64; 2]) -> TileId {
    let side = 1_i64 << 14;
    TileId {
        z: 14,
        x: (((origin[0] + WORLD / 2.) / WORLD * side as f64).floor() as i64),
        y: (((WORLD / 2. - origin[1]) / WORLD * side as f64).floor() as i64).clamp(0, side - 1)
            as u32,
    }
}
pub(crate) fn ground_height(service: super::tiles::Service, origin: [f64; 2]) -> Option<f32> {
    let id = anchor_tile(origin);
    super::terrain::Tile {
        id,
        dem_id: id,
        raster_id: id,
        provider: super::tiles::Provider::OpenStreetMap,
        raster: None,
        elevation: service.payload(super::tiles::Key::elevation(id)),
    }
    .height_at(Camera {
        center: origin,
        zoom: 14.,
    })
}
pub fn prepare(
    data: &Tracks,
    lines: &[crate::state::Line],
    altitudes: &BTreeMap<String, Altitude>,
    hidden: &BTreeSet<String>,
    colors: &BTreeMap<u64, (plot::plot_data::SeriesStyle, plot::line_draw::LineDraw)>,
) -> Result<Arc<[Path3]>, String> {
    let mut paths = Vec::new();
    for trace in &data.traces {
        if hidden.contains(&trace.id.to_string()) {
            continue;
        }
        let Some(y) = lines.iter().find(|line| line.id == trace.id) else {
            continue;
        };
        let altitude = altitudes.get(&y.source).cloned().unwrap_or_default();
        let z = if altitude.source.is_empty() {
            None
        } else {
            Some(
                &lines
                    .iter()
                    .find(|line| line.source == altitude.source)
                    .ok_or_else(|| format!("Missing Z input {}", altitude.source))?
                    .data,
            )
        };
        let reference = trace
            .track
            .times
            .iter()
            .map(|&time| plot::PlotPoint { x: time, y: 0. })
            .collect::<Vec<_>>();
        let mut aligned = z.map(|z| crate::filter_multi::Aligned::new(z, &reference));
        let mut points = trace
            .track
            .points
            .iter()
            .zip(trace.track.times.iter())
            .enumerate()
            .map(|(index, (point, &time))| {
                [
                    point.x,
                    point.y,
                    aligned
                        .as_mut()
                        .map_or(0., |z| z.at(index, time) * altitude.scale),
                ]
            })
            .collect::<Vec<_>>();
        let mut origin = [trace.track.endpoints.0.x, trace.track.endpoints.0.y];
        if altitude.local
            && altitude.at_first
            && let Some(first) = points
                .iter()
                .find(|p| p.iter().all(|v| v.is_finite()))
                .copied()
        {
            origin = [first[0], first[1]];
            for point in &mut points {
                point[2] -= first[2];
            }
        }
        paths.push(Path3 {
            id: trace.id,
            origin,
            points: points.into(),
            times: trace.track.times.clone(),
            color: colors
                .get(&trace.id)
                .map_or([255, 180, 50, 255], |style| style.0.color),
            width: colors.get(&trace.id).map_or(2., |style| {
                if style.0.width.is_finite() {
                    style.0.width.max(0.) as f32
                } else {
                    2.
                }
            }),
            drawing: colors
                .get(&trace.id)
                .map_or_else(Default::default, |style| style.1),
            radius: colors
                .get(&trace.id)
                .map_or(3., |style| style.0.radius.max(0.) as f32),
            marker: colors
                .get(&trace.id)
                .map_or(plot::plot_data::MarkerShape::Circle, |style| style.0.marker),
            local: altitude.local,
        });
    }
    Ok(paths.into())
}
type PreparedPaths = (
    Signal<Arc<[Path3]>, LocalStorage>,
    Signal<bool, LocalStorage>,
);
pub fn install(workbench: Workbench, pane: Pane) -> PreparedPaths {
    let output = RwSignal::new_local(Arc::<[Path3]>::from([]));
    let generation = RwSignal::new_local(0_u64);
    let ready = RwSignal::new_local(false);
    let failure = StoredValue::new_local(String::new());
    let altitudes = Memo::new(move |_| {
        pane.trajectory
            .with(|s| {
                s.as_ref().map(|s| {
                    s.pairs()
                        .iter()
                        .enumerate()
                        .filter_map(|(index, pair)| {
                            s.altitudes
                                .get(&index)
                                .map(|altitude| (pair.y.clone(), altitude.clone()))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
            })
            .unwrap_or_default()
    });
    let enabled = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.map.is_some_and(|m| m.terrain)))
    });
    let effect = RenderEffect::new(move |_| {
        ready.set(false);
        if !enabled.get() {
            generation.update(|g| *g = g.wrapping_add(1));
            output.set(Arc::from([]));
            return;
        }
        let Some(data) = pane.trajectory_data.get() else {
            output.set(Arc::from([]));
            return;
        };
        let lines = pane.lines.get();
        let altitudes = altitudes.get();
        let hidden = pane.plot.hidden.get();
        pane.plot.styles.get();
        let drawing = pane.drawing.get();
        let colors = data
            .traces
            .iter()
            .map(|trace| {
                let style = pane.plot.style(&trace.id.to_string());
                (
                    trace.id,
                    (style, drawing.get(&trace.id).copied().unwrap_or_default()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let next = generation.get_untracked().wrapping_add(1);
        generation.set(next);
        workbench.task(async move {
            let result = plot::preparation::prepare("3d-alignment", move || {
                prepare(&data, &lines, &altitudes, &hidden, &colors)
            })
            .await;
            if generation.try_get_untracked() != Some(next) {
                return;
            }
            match result {
                Ok(paths) => {
                    let old = failure.get_value();
                    if !old.is_empty() && workbench.status.get_untracked() == old {
                        workbench.status.set(String::new());
                    }
                    failure.set_value(String::new());
                    output.set(paths);
                    ready.set(true);
                }
                Err(error) => {
                    output.set(Arc::from([]));
                    failure.set_value(error.clone());
                    workbench.status.set(error);
                }
            }
        });
    });
    on_cleanup_local(move || drop(effect));
    (
        Signal::derive_local(move || output.get()),
        Signal::derive_local(move || ready.get()),
    )
}

type RenderKey = (usize, [u64; 2], u32, u32, Vec<(u64, u32)>, Option<u64>, u64);
pub struct Renderer {
    pipeline: zgui::surface::wgpu::RenderPipeline,
    viewport: zgui::surface::wgpu::Buffer,
    viewport_bind: zgui::surface::wgpu::BindGroup,
    buffer: Option<zgui::surface::wgpu::Buffer>,
    count: u32,
    held_paths: Option<Arc<[Path3]>>,
    key: Option<RenderKey>,
}
impl Renderer {
    pub fn new(
        device: &zgui::surface::wgpu::Device,
        view: &zgui::surface::wgpu::BindGroupLayout,
    ) -> Self {
        use zgui::surface::wgpu;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map.trace3"),
            source: wgpu::ShaderSource::Wgsl(include_str!("flight3.wgsl").into()),
        });
        let viewport = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.trace3.viewport"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(16),
                },
                count: None,
            }],
        });
        let viewport_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(view), Some(&viewport_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map.trace3"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 48,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 40,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 44,
                            shader_location: 4,
                        },
                    ],
                }],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            viewport,
            viewport_bind,
            buffer: None,
            count: 0,
            held_paths: None,
            key: None,
        }
    }
    pub fn prepare(
        &mut self,
        gpu: &zgui::surface::GpuShare,
        scene: &Scene,
        camera: Camera,
        exaggeration: f32,
        css: [f64; 2],
    ) {
        use zgui::surface::wgpu;
        let viewport = [css[0].max(1.) as f32, css[1].max(1.) as f32, 0., 0.];
        gpu.queue().write_buffer(
            &self.viewport,
            0,
            &viewport
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<_>>(),
        );
        let bases = scene
            .paths
            .iter()
            .map(|path| {
                (
                    path.id,
                    scene
                        .path_base(path)
                        .map_or(f32::NAN, |base| base)
                        .to_bits(),
                )
            })
            .collect::<Vec<_>>();
        let key = (
            scene.paths.as_ptr() as usize,
            camera.center.map(f64::to_bits),
            scene.center_height(camera).to_bits(),
            exaggeration.to_bits(),
            bases,
            scene.time.map(f64::to_bits),
            camera.zoom.to_bits(),
        );
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.key = Some(key);
        self.held_paths = Some(scene.paths.clone());
        self.count = 0;
        let vertices = vertices(scene, camera, exaggeration);
        self.count = (vertices.len() / 2) as u32;
        let mut bytes = Vec::with_capacity(self.count as usize * 48);
        for pair in vertices.chunks_exact(2) {
            for value in pair[0][..3]
                .iter()
                .chain(pair[1][..3].iter())
                .chain(pair[0][3..8].iter())
            {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
            let point = pair[0][8];
            bytes.extend_from_slice(&point.to_ne_bytes());
        }
        if bytes.is_empty() {
            self.buffer = None;
            return;
        }
        let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.trace3.vertices"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue().write_buffer(&buffer, 0, &bytes);
        self.buffer = Some(buffer);
    }
    pub fn draw(&self, pass: &mut zgui::surface::wgpu::RenderPass<'_>) {
        if let Some(buffer) = &self.buffer {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.set_bind_group(1, &self.viewport_bind, &[]);
            pass.draw(0..6, 0..self.count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::Line,
        trajectory::{Spec, Tracks},
    };
    fn line(id: u64, source: &str, values: &[(f64, f64)]) -> Line {
        Line {
            id,
            source: source.into(),
            name: source.into(),
            revision: 0,
            data: plot::time_series::SeriesData::new(
                values
                    .iter()
                    .map(|&(x, y)| plot::PlotPoint { x, y })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        }
    }
    #[test]
    fn projected_3d_segment_pins_interpolated_timeline_time() {
        let camera = Camera {
            center: [0., 0.],
            zoom: 14.,
        };
        let orbit = super::super::terrain::Orbit::default();
        let size = [800., 600.];
        let path = Path3 {
            id: 1,
            origin: [-500., 0.],
            points: Arc::from([[-500., 0., 50.], [500., 0., 50.]]),
            times: Arc::from([0., 10.]),
            color: [255; 4],
            width: 2.,
            drawing: Default::default(),
            radius: 3.,
            marker: plot::plot_data::MarkerShape::Circle,
            local: false,
        };
        let mut scene = Scene::default();
        scene.paths = Arc::from([path]);
        let matrix = orbit.matrix(camera, size);
        let v = [0., 50.5, 0., 1.];
        let clip = std::array::from_fn::<_, 4, _>(|row| {
            (0..4)
                .map(|col| matrix[col * 4 + row] * v[col])
                .sum::<f32>()
        });
        let cursor = [
            (f64::from(clip[0] / clip[3]) + 1.) * 400.,
            (1. - f64::from(clip[1] / clip[3])) * 300.,
        ];
        assert!((pick_time(&scene, camera, orbit, size, cursor).unwrap() - 5.).abs() < 0.001);
        assert!(pick_time(&scene, camera, orbit, size, [0., 0.]).is_none());
    }
    #[test]
    fn first_altitude_on_ground_uses_first_valid_sample_and_preserves_height_changes() {
        let lines = vec![
            line(
                1,
                "x",
                &[(0., 0.), (1., 10.), (2., 20.), (3., 30.), (4., 40.)],
            ),
            line(2, "y", &[(0., 0.), (4., 40.)]),
            line(
                3,
                "z",
                &[
                    (0., f64::NAN),
                    (1., -1200000.),
                    (2., -1210000.),
                    (3., f64::NAN),
                    (4., -1220000.),
                ],
            ),
        ];
        let tracks = Tracks::prepare(
            &lines,
            &Spec {
                x: "x".into(),
                y: "y".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut altitudes = BTreeMap::from([(
            "y".into(),
            Altitude {
                source: "z".into(),
                scale: -0.001,
                local: true,
                at_first: true,
            },
        )]);
        let prepare_paths = |altitudes: &BTreeMap<String, Altitude>| {
            prepare(
                &tracks,
                &lines,
                altitudes,
                &Default::default(),
                &Default::default(),
            )
            .unwrap()
        };
        let paths = prepare_paths(&altitudes);
        let path = &paths[0];
        assert!(path.points[0][2].is_nan());
        assert_eq!(path.origin, [10., 10.]);
        assert_eq!(path.points[1][2], 0.);
        assert_eq!(path.points[2][2], 10.);
        assert!(path.points[3][2].is_nan());
        assert_eq!(path.points[4][2], 20.);
        assert_eq!(path.at(1.5).unwrap()[2], 5.);
        assert!(path.at(2.5).is_none());

        altitudes.get_mut("y").unwrap().at_first = false;
        assert_eq!(prepare_paths(&altitudes)[0].points[1][2], 1200.);
        altitudes.get_mut("y").unwrap().local = false;
        altitudes.get_mut("y").unwrap().at_first = true;
        let absolute = prepare_paths(&altitudes);
        assert!(!absolute[0].local);
        assert_eq!(absolute[0].points[1][2], 1200.);
    }
    #[test]
    fn altitude_aligns_to_xy_times_and_preserves_invalid_segments() {
        let lines = vec![
            line(1, "x", &[(0., 0.), (1., 1.), (2., 2.), (3., 3.), (4., 4.)]),
            line(2, "y", &[(0., 0.), (4., 4.)]),
            line(3, "z", &[(0., 1000.), (2., 3000.), (4., 5000.)]),
        ];
        let spec = Spec {
            x: "x".into(),
            y: "y".into(),
            ..Default::default()
        };
        let tracks = Tracks::prepare(&lines, &spec).unwrap();
        let altitudes = BTreeMap::from([(
            "y".into(),
            Altitude {
                source: "z".into(),
                scale: 0.001,
                local: true,
                at_first: false,
            },
        )]);
        let paths = prepare(
            &tracks,
            &lines,
            &altitudes,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            paths[0].points.iter().map(|p| p[2]).collect::<Vec<_>>(),
            vec![1., 2., 3., 4., 5.]
        );
        assert_eq!(paths[0].at(1.5).unwrap(), [1.5, 1.5, 2.5]);
        assert!(paths[0].at(5.).is_none());
        let mut style = plot::plot_data::SeriesStyle::for_id("2");
        style.radius = 7.;
        style.marker = plot::plot_data::MarkerShape::Diamond;
        let styled = prepare(
            &tracks,
            &lines,
            &altitudes,
            &Default::default(),
            &BTreeMap::from([(
                2,
                (
                    style,
                    plot::line_draw::LineDraw {
                        markers: true,
                        pattern: plot::line_draw::LinePattern::None,
                        ..Default::default()
                    },
                ),
            )]),
        )
        .unwrap();
        let mut scene = Scene::default();
        scene.paths = styled;
        Arc::make_mut(&mut scene.paths)[0].local = false;
        let geometry = vertices(
            &scene,
            Camera {
                center: [0., 0.],
                zoom: 14.,
            },
            1.,
        );
        let samples = geometry
            .chunks_exact(2)
            .filter(|pair| pair[0][8] == 4.)
            .collect::<Vec<_>>();
        assert_eq!(samples.len(), 5, "Every valid source sample gets marker");
        assert!(
            samples
                .iter()
                .all(|pair| pair[0] == pair[1] && pair[0][7] == 14.)
        );
        assert!(
            geometry
                .chunks_exact(2)
                .filter(|pair| pair[0][8] == 0.)
                .all(|pair| pair[0][7] == 2.),
            "Disabled line leaves endpoint crosses only"
        );
        let mut invalid = lines.clone();
        invalid[2] = line(3, "z", &[(0., 1000.), (2., f64::NAN), (4., 5000.)]);
        let paths = prepare(
            &tracks,
            &invalid,
            &altitudes,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        assert!(paths[0].points[1][2].is_nan());
        assert!(paths[0].at(1.5).is_none());
        assert!(
            prepare(
                &tracks,
                &lines,
                &altitudes,
                &BTreeSet::from(["2".into()]),
                &Default::default()
            )
            .unwrap()
            .is_empty()
        );
    }
}

#[cfg(test)]
#[path = "flight3_tests.rs"]
mod integration_tests;
