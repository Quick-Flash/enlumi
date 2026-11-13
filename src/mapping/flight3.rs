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
    /// Direction chevrons along the trace; restyle only, set from the plot spec.
    pub arrows: bool,
    pub arrow_color: [u8; 4],
    /// State changes (time, color) marked along the trace; restyle only.
    pub modes: Arc<[(f64, [u8; 3])]>,
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
#[cfg(test)]
pub(crate) fn vertices(
    scene: &Scene,
    camera: Camera,
    orbit: super::terrain::Orbit,
    size: [f64; 2],
) -> Vec<[f32; 11]> {
    let mut ground = super::ground_path::Cache::default();
    ground.prepare(&scene.paths, &scene.tiles);
    let mut detail = super::trace_detail::Cache::default();
    let view = super::trace_detail::View {
        camera,
        orbit,
        size,
        focus: scene.center_height(camera),
    };
    for path in scene.paths.iter() {
        if let Some(origin) = scene.path_base(path) {
            ground.get(path, &detail.trace(path, &scene.colors, view, origin));
        }
    }
    [Layer::Ground, Layer::Trace, Layer::Timeline]
        .into_iter()
        .flat_map(|layer| {
            vertices_layer(scene, camera, orbit, size, layer, &mut ground, &mut detail)
        })
        .collect()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// Draped ground strokes, stored once as halos (see flight3.wgsl).
    Ground,
    Trace,
    Timeline,
}
fn vertices_layer(
    scene: &Scene,
    camera: Camera,
    orbit: super::terrain::Orbit,
    size: [f64; 2],
    layer: Layer,
    ground: &mut super::ground_path::Cache,
    detail: &mut super::trace_detail::Cache,
) -> Vec<[f32; 11]> {
    let scale = super::terrain::ground_scale(camera);
    let basis = Basis::of(scene, camera);
    let projection = super::hover::Projection::new(scene, camera, orbit, size);
    let mut vertices = Vec::new();
    let view = super::trace_detail::View {
        camera,
        orbit,
        size,
        focus: scene.center_height(camera),
    };
    for path in scene.paths.iter() {
        let Some(origin) = scene.path_base(path) else {
            continue;
        };
        let vertex = |point: [f64; 3], color: [u8; 4]| {
            let [x, y, z] = basis.place(point, origin, orbit.exaggeration);
            [
                x,
                y,
                z,
                f32::from(color[0]) / 255.,
                f32::from(color[1]) / 255.,
                f32::from(color[2]) / 255.,
                f32::from(color[3]) / 255.,
                path.width,
                0.,
                0.,
                0.,
            ]
        };
        if layer == Layer::Ground && scene.ground_projection {
            // Renderer::prepare drapes every path before building this layer.
            let source = ground.cached(path.id).unwrap_or_default();
            for segment in &detail.ground(path.id, source, view) {
                for &[x, y, z] in segment {
                    let mut v = vertex([x, y, z - f64::from(origin)], [255, 255, 255, 170]);
                    v[7] = 3.5;
                    v[8] = -1.;
                    vertices.push(v);
                }
            }
        }
        if layer == Layer::Trace {
            if path.drawing.pattern != plot::line_draw::LinePattern::None {
                let pattern = match path.drawing.pattern {
                    plot::line_draw::LinePattern::Dashed => 1.,
                    plot::line_draw::LinePattern::Dotted => 2.,
                    plot::line_draw::LinePattern::DashDot => 3.,
                    _ => 0.,
                };
                let mut phase = 0.;
                // Keep dash phase continuous across samples, measured in CSS pixels.
                let indices = if path.drawing.pattern == plot::line_draw::LinePattern::Solid {
                    detail.trace(path, &scene.colors, view, origin)
                } else {
                    (0..path.points.len()).collect()
                };
                for indices in indices.windows(2) {
                    let index = indices[0];
                    let next = indices[1];
                    let pair = [path.points[index], path.points[next]];
                    if pair.iter().flatten().all(|v| v.is_finite()) {
                        let (ta, tb) = (path.times[index], path.times[next]);
                        let scalar = |offset: usize| {
                            scene
                                .colors
                                .samples
                                .get(&path.id)
                                .and_then(|values| {
                                    values.get(if offset == 0 { index } else { next })
                                })
                                .copied()
                                .unwrap_or(f64::NAN)
                        };
                        let (a, b) = (scalar(0), scalar(1));
                        let mut cuts = if scene.colors.tints.is_empty() {
                            Vec::new()
                        } else {
                            crate::state_tint::cuts(&scene.colors.tints, ta, tb)
                        };
                        if a.is_finite()
                            && b.is_finite()
                            && a != b
                            && let Some(range) = scene
                                .colors
                                .scales
                                .get(&path.id)
                                .map(|scale| scale.gradient.range)
                                .or_else(|| {
                                    scene
                                        .colors
                                        .scales
                                        .is_empty()
                                        .then_some(scene.colors.range)
                                        .flatten()
                                })
                        {
                            for limit in range {
                                let at = (limit - a) / (b - a);
                                if at > 0. && at < 1. {
                                    cuts.push(at);
                                }
                            }
                        }
                        let defaults = [0., 1.];
                        let cuts = if cuts.is_empty() {
                            &defaults[..]
                        } else {
                            cuts.extend(defaults);
                            cuts.sort_unstable_by(f64::total_cmp);
                            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
                            &cuts[..]
                        };
                        for cut in cuts.windows(2) {
                            let middle = ta + (tb - ta) * (cut[0] + cut[1]) / 2.;
                            let point = |t: f64| {
                                std::array::from_fn(|axis| {
                                    pair[0][axis] + (pair[1][axis] - pair[0][axis]) * t
                                })
                            };
                            for &at in cut {
                                let value = if a.is_finite() && b.is_finite() {
                                    a + (b - a) * at
                                } else {
                                    f64::NAN
                                };
                                let mut v = vertex(
                                    point(at),
                                    scene.colors.shade(path.id, value, middle, path.color),
                                );
                                v[9] = phase as f32;
                                v[10] = pattern;
                                vertices.push(v);
                            }
                            if pattern != 0. {
                                if let (Some((a, _)), Some((b, _))) = (
                                    projection.point(point(cut[0]), origin),
                                    projection.point(point(cut[1]), origin),
                                ) {
                                    phase += (b[0] - a[0]).hypot(b[1] - a[1]);
                                } else {
                                    phase = 0.;
                                }
                            }
                        }
                    } else {
                        phase = 0.;
                    }
                }
            }
            if path.drawing.markers {
                for (index, &point) in path
                    .points
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.iter().all(|v| v.is_finite()))
                {
                    let mut marker = vertex(point, scene.colors.color(path.id, index, path.color));
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
            if path.arrows && path.drawing.pattern != plot::line_draw::LinePattern::None {
                let width = path.width.max(1.5);
                for (_, segment) in arrow_segments(&path.points, scale, scale / camera.scale()) {
                    let color = path.arrow_color;
                    for point in segment {
                        let mut v = vertex(point, color);
                        v[7] = width;
                        vertices.push(v);
                    }
                }
            }
            for &(time, [r, g, b]) in path.modes.iter() {
                if let Some(point) = path.at(time) {
                    let mut marker = vertex(point, [r, g, b, 255]);
                    marker[7] = 10.;
                    marker[8] = 6.;
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
        }
        if layer == Layer::Timeline
            && let Some(point) = scene.time.and_then(|time| path.at(time))
        {
            if scene.ground_projection
                && let Some(height) = scene
                    .tiles
                    .iter()
                    .find_map(|tile| tile.mesh_height_at([point[0], point[1]]))
            {
                let mut a = vertex(
                    [point[0], point[1], f64::from(height - origin)],
                    [255, 255, 255, 115],
                );
                let mut b = vertex(point, [255, 255, 255, 115]);
                a[7] = 1.;
                b[7] = 1.;
                vertices.extend([a, b]);
            }
            let mut marker = vertex(point, [255, 215, 60, 255]);
            marker[7] = 13.;
            marker[8] = 6.;
            vertices.extend([marker, marker]);
            if let Some(time) = scene.time
                && let Some(chevron) =
                    heading_chevron(path, time, point, scale, scale / camera.scale())
            {
                for point in chevron {
                    let mut v = vertex(point, [255, 255, 255, 235]);
                    v[7] = 3.;
                    vertices.push(v);
                }
            }
        }
    }
    vertices
}
const ARROW_SPACING: f64 = 90.;
const ARROW_LENGTH: f64 = 7.;
const ARROW_HALF_WIDTH: f64 = 5.;

/// Chevron wings (two line segments) for a tip moving along `direction`.
/// `direction` is in true metres; `ground` converts mercator units to metres.
fn chevron(
    tip: [f64; 3],
    direction: [f64; 3],
    ground: f64,
    length: f64,
    half: f64,
) -> Option<[[f64; 3]; 4]> {
    let flat = direction[0].hypot(direction[1]);
    let norm = (flat * flat + direction[2] * direction[2]).sqrt();
    if norm <= 0. || !norm.is_finite() || ground <= 0. || !ground.is_finite() {
        return None;
    }
    let u = direction.map(|v| v / norm);
    let back = [
        tip[0] - u[0] * length / ground,
        tip[1] - u[1] * length / ground,
        tip[2] - u[2] * length,
    ];
    let side = if flat == 0. {
        [half / ground, 0.]
    } else {
        [
            -direction[1] / flat * half / ground,
            direction[0] / flat * half / ground,
        ]
    };
    let wing = |sign: f64| [back[0] + side[0] * sign, back[1] + side[1] * sign, back[2]];
    Some([wing(1.), tip, wing(-1.), tip])
}

/// Chevrons every ARROW_SPACING screen pixels of path, with the sample index for color.
/// `pixel` is true metres per CSS pixel at the focus.
fn arrow_segments(points: &[[f64; 3]], ground: f64, pixel: f64) -> Vec<(usize, [[f64; 3]; 4])> {
    let spacing = ARROW_SPACING * pixel;
    let mut out = Vec::new();
    if spacing.is_nan() || ground.is_nan() || spacing <= 0. || ground <= 0. {
        return out;
    }
    let mut since = spacing / 2.;
    let mut previous = None::<[f64; 3]>;
    for (index, &point) in points.iter().enumerate() {
        if !point.iter().all(|v| v.is_finite()) {
            previous = None;
            continue;
        }
        if let Some(a) = previous {
            let d = [
                (point[0] - a[0]) * ground,
                (point[1] - a[1]) * ground,
                point[2] - a[2],
            ];
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if length > 0. {
                let mut at = spacing - since;
                while at <= length && out.len() < 4096 {
                    let t = at / length;
                    let tip = std::array::from_fn(|axis| a[axis] + (point[axis] - a[axis]) * t);
                    if let Some(c) = chevron(
                        tip,
                        d,
                        ground,
                        ARROW_LENGTH * pixel,
                        ARROW_HALF_WIDTH * pixel,
                    ) {
                        out.push((index, c));
                    }
                    at += spacing;
                }
                since = length - (at - spacing);
            }
        }
        previous = Some(point);
    }
    out
}

/// Larger chevron ahead of the timeline marker, following climb and descent.
fn heading_chevron(
    path: &Path3,
    time: f64,
    point: [f64; 3],
    ground: f64,
    pixel: f64,
) -> Option<[[f64; 3]; 4]> {
    let (before, after) = (
        path.at(time - 0.5).unwrap_or(point),
        path.at(time + 0.5).unwrap_or(point),
    );
    let d = [
        (after[0] - before[0]) * ground,
        (after[1] - before[1]) * ground,
        after[2] - before[2],
    ];
    let norm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !norm.is_finite() || norm <= 0. {
        return None;
    }
    let tip = [
        point[0] + d[0] / norm * 14. * pixel / ground,
        point[1] + d[1] / norm * 14. * pixel / ground,
        point[2] + d[2] / norm * 14. * pixel,
    ];
    chevron(tip, d, ground, 8. * pixel, 7. * pixel)
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
            arrows: false,
            arrow_color: crate::trajectory::arrow_color(),
            modes: Arc::from([]),
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
    let prepared = RwSignal::new_local(None);
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
    // Lazy request identity makes readiness false immediately when inputs change,
    // before either alignment or Fit effects have had a chance to run.
    let request = Memo::new(move |_| {
        (
            enabled.get(),
            pane.trajectory_data
                .with(|data| data.as_ref().map(|data| Arc::as_ptr(data) as usize)),
            pane.lines.with(|lines| {
                lines
                    .iter()
                    .map(|line| {
                        (
                            line.id,
                            line.revision,
                            line.data.samples().as_ptr() as usize,
                            line.data.samples().len(),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
            altitudes.get(),
            pane.plot.styles.get(),
            pane.drawing.get(),
        )
    });
    let effect = RenderEffect::new(move |_| {
        let key = request.get();
        prepared.set(None);
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
                // Hidden traces stay prepared so legend hover can reveal them.
                prepare(&data, &lines, &altitudes, &BTreeSet::new(), &colors)
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
                    prepared.set(Some(key));
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
    // Visibility, legend order and hover emphasis only restyle prepared paths; no re-alignment.
    let styled = Memo::new_owning(move |_| {
        let order = pane.legend_order.get();
        let emphasized = pane.plot.emphasized.get();
        let hidden = pane.plot.hidden.get();
        let arrows = pane
            .trajectory
            .with(|spec| spec.as_ref().is_some_and(|spec| spec.arrows));
        let mut paths = emphasize(
            &output.get(),
            &order,
            &hidden,
            emphasized.as_deref(),
            arrows,
        );
        decorate(workbench, pane, &mut paths);
        let paths = match super::flight::highlight_window(workbench, pane) {
            Some(window) => highlight(&paths, window),
            None => paths,
        };
        (paths, true)
    });
    (
        Signal::derive_local(move || styled.get()),
        Signal::derive_local(move || prepared.get().is_some_and(|key| key == request.get())),
    )
}
/// Apply per-line marker appearance without re-aligning samples.
fn decorate(workbench: Workbench, pane: Pane, paths: &mut Arc<[Path3]>) {
    let fallback = pane.trajectory.with(|s| {
        s.as_ref()
            .map_or_else(crate::trajectory::arrow_color, |s| s.arrow_color)
    });
    let modes = mode_changes(workbench, pane);
    for path in Arc::make_mut(paths) {
        let style = crate::trace_color::current(pane, Some(path.id));
        let color = style.arrow_color.unwrap_or(fallback);
        path.arrow_color = color;
        path.arrow_color[3] = (u16::from(color[3]) * u16::from(path.color[3]) / 255) as u8;
        path.modes = if style.state_markers {
            modes.clone()
        } else {
            Arc::from([])
        };
    }
}
/// Paint in legend order with the emphasized trace last, wider, and others dimmed,
/// matching 2D plots. Hidden traces show only while emphasized.
fn emphasize(
    paths: &[Path3],
    order: &[u64],
    hidden: &BTreeSet<String>,
    emphasized: Option<&str>,
    arrows: bool,
) -> Arc<[Path3]> {
    let emphasized = emphasized.and_then(|id| id.parse::<u64>().ok());
    let mut paths = paths
        .iter()
        .filter(|path| emphasized == Some(path.id) || !hidden.contains(&path.id.to_string()))
        .cloned()
        .map(|mut path| {
            path.arrows = arrows;
            path
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| {
        (
            emphasized == Some(path.id),
            order
                .iter()
                .position(|id| *id == path.id)
                .unwrap_or(usize::MAX),
        )
    });
    if let Some(emphasized) = emphasized {
        for path in &mut paths {
            if path.id == emphasized {
                path.width += 1.5;
            } else {
                path.color[3] = (f32::from(path.color[3]) * 0.3).round() as u8;
            }
        }
    }
    paths.into()
}

/// Value changes of the plot's first state band, when state markers are enabled.
fn mode_changes(workbench: Workbench, pane: Pane) -> Arc<[(f64, [u8; 3])]> {
    let enabled = pane
        .trajectory
        .with(|s| s.as_ref().and_then(|s| s.map).is_some_and(|m| m.modes));
    if !enabled {
        return Arc::from([]);
    }
    let hidden = pane.hidden_geometry.get();
    let Some(states) = workbench
        .ordered_geometry(pane)
        .into_iter()
        .filter(|item| !hidden.contains(&item.id))
        .find_map(|item| item.states.filter(|s| s.error.is_none()))
    else {
        return Arc::from([]);
    };
    let mut out = Vec::new();
    let mut previous = None;
    for interval in states.intervals.iter() {
        if previous == Some(interval.value) {
            continue;
        }
        previous = Some(interval.value);
        if let Some(rule) = states.rules.iter().find(|r| r.value == interval.value) {
            out.push((interval.start, rule.color));
        }
        if out.len() == 60 {
            break;
        }
    }
    out.into()
}

/// Bit marking the faded full-length copy of a path drawn under its highlight.
pub(crate) const FADED: u64 = 1 << 63;

/// Each path that leaves `window` becomes a faded full copy (solid, no arrows)
/// plus the original with samples outside the window hidden, keeping indices
/// aligned with its colors.
fn highlight(paths: &[Path3], window: [f64; 2]) -> Arc<[Path3]> {
    let mut out = Vec::with_capacity(paths.len() * 2);
    for path in paths {
        let inside = |t: f64| t >= window[0] && t <= window[1];
        if path.times.iter().all(|t| inside(*t)) {
            out.push(path.clone());
            continue;
        }
        let mut faded = path.clone();
        faded.id |= FADED;
        faded.arrows = false;
        faded.color[3] = (f32::from(faded.color[3]) * 0.2).round() as u8;
        out.push(faded);
        let mut shown = path.clone();
        shown.points = path
            .points
            .iter()
            .zip(path.times.iter())
            .map(|(p, t)| if inside(*t) { *p } else { [f64::NAN; 3] })
            .collect();
        out.push(shown);
    }
    out.into()
}

/// Changes that make built geometry wrong: rebuild before drawing.
type SyncKey = (
    usize,
    u32,
    Vec<(u64, u32)>,
    Option<([u32; 16], [u64; 2])>,
    usize,
    bool,
);
/// Build camera and reduction detail. Geometry built for an older detail
/// stays exactly placed under `Basis::offset`, so it may update in the
/// background.
type DetailKey = ([u64; 2], u32, u64, u64);
type RenderKey = (SyncKey, DetailKey);
fn geometry_key(
    scene: &Scene,
    camera: Camera,
    orbit: super::terrain::Orbit,
    css: [f64; 2],
) -> RenderKey {
    let bases = scene
        .paths
        .iter()
        .map(|path| (path.id, scene.path_base(path).unwrap_or(f32::NAN).to_bits()))
        .collect();
    let patterned = scene.paths.iter().any(|path| {
        matches!(
            path.drawing.pattern,
            plot::line_draw::LinePattern::Dashed
                | plot::line_draw::LinePattern::Dotted
                | plot::line_draw::LinePattern::DashDot
        )
    });
    (
        (
            scene.paths.as_ptr() as usize,
            orbit.exaggeration.to_bits(),
            bases,
            patterned.then(|| {
                (
                    orbit.matrix(camera, css).map(f32::to_bits),
                    css.map(f64::to_bits),
                )
            }),
            Arc::as_ptr(&scene.colors) as usize,
            scene.ground_projection,
        ),
        (
            camera.center.map(f64::to_bits),
            scene.center_height(camera).to_bits(),
            camera.zoom.to_bits(),
            css[1].to_bits(),
        ),
    )
}
/// Camera frame that path vertices were built in (see `vertices_layer`).
#[derive(Clone, Copy)]
struct Basis {
    center: [f64; 2],
    scale: f64,
    base: f64,
}
impl Basis {
    fn of(scene: &Scene, camera: Camera) -> Self {
        Self {
            center: camera.center,
            scale: super::terrain::ground_scale(camera),
            base: f64::from(scene.center_height(camera)),
        }
    }
    /// Camera-relative vertex position, as uploaded.
    fn place(self, point: [f64; 3], origin: f32, exaggeration: f32) -> [f32; 3] {
        [
            ((point[0] - self.center[0]) * self.scale) as f32,
            ((point[2] + f64::from(origin) - self.base) * f64::from(exaggeration) + 0.5) as f32,
            ((self.center[1] - point[1]) * self.scale) as f32,
        ]
    }
    /// Translation and horizontal scale that move vertices built in `self`
    /// to where they belong in `now`; exact for pans and focus-height moves.
    fn offset(self, now: Self, exaggeration: f32) -> [f32; 4] {
        [
            ((self.center[0] - now.center[0]) * now.scale) as f32,
            ((self.base - now.base) * f64::from(exaggeration)) as f32,
            ((now.center[1] - self.center[1]) * now.scale) as f32,
            (now.scale / self.scale) as f32,
        ]
    }
}
/// Wakes the producer when background work finishes.
pub type Wake = Arc<dyn Fn() + Send + Sync>;
/// Builds trace and draped ground-stroke vertices, caching between builds.
#[derive(Default)]
struct PathBuilder {
    ground: super::ground_path::Cache,
    detail: super::trace_detail::Cache,
    /// Per-path reductions; they depend only on the render key.
    indices: BTreeMap<u64, Vec<usize>>,
    trace: Option<RenderKey>,
    /// Render key and draped segments behind the last ground vertices.
    built: Option<(RenderKey, Vec<usize>)>,
}
/// New vertices for each layer; `None` keeps that layer's current buffer.
struct Built {
    basis: Basis,
    trace: Option<Vec<[f32; 11]>>,
    ground: Option<Vec<[f32; 11]>>,
}
impl PathBuilder {
    fn build(
        &mut self,
        scene: &Scene,
        camera: Camera,
        orbit: super::terrain::Orbit,
        css: [f64; 2],
        key: &RenderKey,
    ) -> Built {
        let basis = Basis::of(scene, camera);
        let trace = (self.trace.as_ref() != Some(key)).then(|| {
            self.indices.clear();
            self.detail.retain(&scene.paths);
            self.trace = Some(key.clone());
            vertices_layer(
                scene,
                camera,
                orbit,
                css,
                Layer::Trace,
                &mut self.ground,
                &mut self.detail,
            )
        });
        if !scene.ground_projection {
            self.built = None;
            return Built {
                basis,
                trace,
                ground: Some(Vec::new()),
            };
        }
        self.ground.prepare(&scene.paths, &scene.tiles);
        let view = super::trace_detail::View {
            camera,
            orbit,
            size: css,
            focus: scene.center_height(camera),
        };
        let mut grounds = Vec::new();
        for path in scene.paths.iter() {
            let Some(origin) = scene.path_base(path) else {
                continue;
            };
            let indices = self
                .indices
                .entry(path.id)
                .or_insert_with(|| self.detail.trace(path, &scene.colors, view, origin));
            grounds.push(Arc::as_ptr(&self.ground.get(path, indices)) as *const u8 as usize);
        }
        let built = Some((key.clone(), grounds));
        let ground = (self.built != built).then(|| {
            self.built = built;
            vertices_layer(
                scene,
                camera,
                orbit,
                css,
                Layer::Ground,
                &mut self.ground,
                &mut self.detail,
            )
        });
        Built {
            basis,
            trace,
            ground,
        }
    }
}
struct PathJob {
    scene: Scene,
    camera: Camera,
    orbit: super::terrain::Orbit,
    css: [f64; 2],
    key: RenderKey,
}
struct PathDone {
    key: RenderKey,
    built: Built,
}
#[derive(Default)]
struct PathQueue {
    job: Option<PathJob>,
    done: Option<PathDone>,
    builds: u64,
    closed: bool,
}
/// Rebuilds path geometry off the render thread when only the camera,
/// detail or terrain changed: orbiting moves tiles under long paths in and
/// out of view nearly every frame, and pans and zooms change the reduction.
/// Current buffers stay correctly placed meanwhile, merely a frame behind.
struct PathWorker {
    queue: std::sync::Mutex<PathQueue>,
    changed: std::sync::Condvar,
}
impl PathWorker {
    fn spawn(wake: Wake) -> Arc<Self> {
        let worker = Arc::new(Self {
            queue: Default::default(),
            changed: Default::default(),
        });
        let shared = worker.clone();
        std::thread::spawn(move || {
            let mut builder = PathBuilder::default();
            loop {
                let job = {
                    let mut queue = shared.queue.lock().unwrap();
                    loop {
                        if queue.closed {
                            return;
                        }
                        if let Some(job) = queue.job.take() {
                            break job;
                        }
                        queue = shared.changed.wait(queue).unwrap();
                    }
                };
                let before = builder.ground.builds;
                let built = builder.build(&job.scene, job.camera, job.orbit, job.css, &job.key);
                let mut queue = shared.queue.lock().unwrap();
                queue.builds += builder.ground.builds - before;
                queue.done = Some(PathDone {
                    key: job.key,
                    built,
                });
                drop(queue);
                wake();
            }
        });
        worker
    }
}
pub struct Renderer {
    pipeline: zgui::surface::wgpu::RenderPipeline,
    viewport: zgui::surface::wgpu::Buffer,
    viewport_bind: zgui::surface::wgpu::BindGroup,
    /// Markers are rebuilt for the current camera: no offset.
    marker_viewport: zgui::surface::wgpu::Buffer,
    marker_bind: zgui::surface::wgpu::BindGroup,
    geometry: super::flight3_buffer::Buffer,
    ground_geometry: super::flight3_buffer::Buffer,
    marker: super::flight3_buffer::Buffer,
    detail: super::trace_detail::Cache,
    held_paths: Option<Arc<[Path3]>>,
    held_colors: Option<Arc<super::telemetry::Coloring>>,
    /// Key and camera frame of the uploaded trace and ground buffers.
    key: Option<RenderKey>,
    basis: Option<Basis>,
    /// Builds immediately when the sync key changes.
    builder: PathBuilder,
    worker: Arc<PathWorker>,
    sent: Option<(RenderKey, super::ground_path::Key)>,
    marker_key: Option<(RenderKey, Option<u64>, super::ground_path::Key)>,
}
impl Drop for Renderer {
    fn drop(&mut self) {
        self.worker.queue.lock().unwrap().closed = true;
        self.worker.changed.notify_one();
    }
}
impl Renderer {
    pub fn new(
        device: &zgui::surface::wgpu::Device,
        view: &zgui::surface::wgpu::BindGroupLayout,
        wake: Wake,
    ) -> Self {
        use zgui::surface::wgpu;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map.trace3"),
            source: wgpu::ShaderSource::Wgsl(include_str!("flight3.wgsl").into()),
        });
        let viewport = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.trace3.viewport"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let marker_viewport = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.trace3.marker_viewport"),
            size: 32,
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
                    min_binding_size: std::num::NonZeroU64::new(32),
                },
                count: None,
            }],
        });
        let bind = |buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &viewport_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let viewport_bind = bind(&viewport);
        let marker_bind = bind(&marker_viewport);
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
                    array_stride: 72,
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
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 48,
                            shader_location: 5,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 56,
                            shader_location: 6,
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
            marker_viewport,
            marker_bind,
            geometry: Default::default(),
            ground_geometry: Default::default(),
            marker: Default::default(),
            detail: Default::default(),
            held_paths: None,
            held_colors: None,
            key: None,
            basis: None,
            builder: PathBuilder::default(),
            worker: PathWorker::spawn(wake),
            sent: None,
            marker_key: None,
        }
    }
    pub fn prepare(
        &mut self,
        gpu: &zgui::surface::GpuShare,
        scene: &Scene,
        camera: Camera,
        orbit: super::terrain::Orbit,
        css: [f64; 2],
    ) {
        let key = geometry_key(scene, camera, orbit, css);
        let terrain = if scene.ground_projection {
            super::ground_path::key(&scene.tiles)
        } else {
            Vec::new()
        };
        let wanted = (key.clone(), terrain.clone());
        if self.key.as_ref().is_none_or(|(sync, _)| *sync != key.0) {
            // Paths, colors, scale or patterns changed: old buffers are wrong.
            self.held_paths = Some(scene.paths.clone());
            self.held_colors = Some(scene.colors.clone());
            let built = self.builder.build(scene, camera, orbit, css, &key);
            self.apply(gpu, key.clone(), built);
            self.sent = Some(wanted);
        } else if self.sent.as_ref() != Some(&wanted) {
            self.worker.queue.lock().unwrap().job = Some(PathJob {
                scene: scene.clone(),
                camera,
                orbit,
                css,
                key: key.clone(),
            });
            self.worker.changed.notify_one();
            self.sent = Some(wanted);
        }
        let done = self.worker.queue.lock().unwrap().done.take();
        // Results for older paths or styles would draw wrong geometry.
        if let Some(done) = done.filter(|done| done.key.0 == key.0) {
            self.apply(gpu, done.key, done.built);
        }
        let now = Basis::of(scene, camera);
        let offset = self.basis.map_or([0., 0., 0., 1.], |basis| {
            basis.offset(now, orbit.exaggeration)
        });
        let size = [css[0].max(1.) as f32, css[1].max(1.) as f32, 0., 0.];
        for (buffer, offset) in [
            (&self.viewport, offset),
            (&self.marker_viewport, [0., 0., 0., 1.]),
        ] {
            let values = size.iter().chain(&offset);
            let bytes = values.flat_map(|v| v.to_ne_bytes()).collect::<Vec<_>>();
            gpu.queue().write_buffer(buffer, 0, &bytes);
        }
        // Markers follow the camera, time and the terrain under the path.
        let marker_key = Some((key, scene.time.map(f64::to_bits), terrain));
        if self.marker_key != marker_key {
            self.marker_key = marker_key;
            self.marker.upload(
                gpu,
                &vertices_layer(
                    scene,
                    camera,
                    orbit,
                    css,
                    Layer::Timeline,
                    &mut self.builder.ground,
                    &mut self.detail,
                ),
            );
        }
    }
    fn apply(&mut self, gpu: &zgui::surface::GpuShare, key: RenderKey, built: Built) {
        if let Some(trace) = built.trace {
            self.geometry.upload(gpu, &trace);
        }
        if let Some(ground) = built.ground {
            self.ground_geometry.upload(gpu, &ground);
        }
        self.key = Some(key);
        self.basis = Some(built.basis);
    }
    pub fn draw(&self, pass: &mut zgui::surface::wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &self.viewport_bind, &[]);
        self.ground_geometry.draw_ground(pass);
        self.geometry.draw(pass);
        pass.set_bind_group(1, &self.marker_bind, &[]);
        self.marker.draw(pass);
    }
    pub fn work(&self) -> Work {
        Work {
            ground_builds: self.builder.ground.builds + self.worker.queue.lock().unwrap().builds,
            trace_uploads: self.geometry.uploads,
            trace_bytes: self.geometry.uploaded_bytes,
            marker_uploads: self.marker.uploads,
            marker_bytes: self.marker.uploaded_bytes,
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Work {
    pub ground_builds: u64,
    pub trace_uploads: u64,
    pub trace_bytes: u64,
    pub marker_uploads: u64,
    pub marker_bytes: u64,
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
    fn offset_moves_old_vertices_onto_the_current_camera() {
        let point = [1200., -800., 75.];
        let built = Basis {
            center: [0., 0.],
            scale: 0.76,
            base: 1300.,
        };
        let now = Basis {
            center: [350., -420.],
            scale: 0.7605,
            base: 1342.,
        };
        let [x, y, z] = built.place(point, 20., 2.);
        let [dx, dy, dz, w] = built.offset(now, 2.);
        let moved = [x * w + dx, y + dy, z * w + dz];
        let expected = now.place(point, 20., 2.);
        for axis in 0..3 {
            assert!(
                (moved[axis] - expected[axis]).abs() < 1e-3,
                "{moved:?} vs {expected:?}"
            );
        }
    }
    #[test]
    fn highlight_fades_a_full_copy_and_hides_samples_outside_the_window() {
        let path = Path3 {
            id: 7,
            origin: [0., 0.],
            points: (0..5).map(|i| [f64::from(i), 0., 10.]).collect(),
            times: (0..5).map(f64::from).collect(),
            color: [200, 100, 50, 255],
            width: 2.,
            drawing: Default::default(),
            radius: 3.,
            marker: plot::plot_data::MarkerShape::Circle,
            local: false,
            arrows: true,
            arrow_color: crate::trajectory::arrow_color(),
            modes: Arc::from([]),
        };
        let split = highlight(std::slice::from_ref(&path), [1., 3.]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].id, 7 | FADED);
        assert_eq!(split[0].color[3], 51);
        assert!(!split[0].arrows);
        assert_eq!(split[1].id, 7);
        let shown: Vec<bool> = split[1].points.iter().map(|p| p[0].is_finite()).collect();
        assert_eq!(shown, [false, true, true, true, false]);
        // A window covering the whole flight leaves the path untouched.
        let whole = highlight(std::slice::from_ref(&path), [-1., 10.]);
        assert_eq!(whole.len(), 1);
        assert_eq!(whole[0].id, 7);
    }

    #[test]
    fn emphasis_follows_legend_order_and_raises_hovered_trace() {
        let path = |id| Path3 {
            id,
            origin: [0., 0.],
            points: Arc::from([]),
            times: Arc::from([]),
            color: [255; 4],
            width: 2.,
            drawing: Default::default(),
            radius: 3.,
            marker: plot::plot_data::MarkerShape::Circle,
            local: false,
            arrows: false,
            arrow_color: crate::trajectory::arrow_color(),
            modes: Arc::from([]),
        };
        let paths = [path(1), path(2), path(3)];
        let ids = |paths: &[Path3]| paths.iter().map(|p| p.id).collect::<Vec<_>>();
        let none = BTreeSet::new();
        assert_eq!(
            ids(&emphasize(&paths, &[3, 1, 2], &none, None, false)),
            [3, 1, 2]
        );
        let hidden = BTreeSet::from(["1".to_string()]);
        assert_eq!(
            ids(&emphasize(&paths, &[3, 1, 2], &hidden, None, false)),
            [3, 2]
        );
        let styled = emphasize(&paths, &[3, 1, 2], &hidden, Some("1"), false);
        assert_eq!(ids(&styled), [3, 2, 1]);
        assert_eq!((styled[2].width, styled[2].color[3]), (3.5, 255));
        assert_eq!((styled[0].width, styled[0].color[3]), (2., 77));
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
            arrows: false,
            arrow_color: crate::trajectory::arrow_color(),
            modes: Arc::from([]),
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
            super::super::terrain::Orbit::default(),
            [800., 600.],
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
#[path = "flight3_style_tests.rs"]
mod style_tests;

#[cfg(test)]
#[path = "flight3_tests.rs"]
mod integration_tests;
