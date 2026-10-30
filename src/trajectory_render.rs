//! Retained, bounded GPU dash segments in data coordinates. Pan never clears the frame.
use crate::{
    state::{Pane, Workbench},
    trajectory::{PreparedTrace, Tracks},
};
use plot::{
    PlotGeometry, PlotPoint, Ranges,
    line_draw::{LineDraw, LinePattern},
    plot_data::{MarkerShape, SeriesStyle},
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use zgui::{
    canvas::{CanvasScene, Marker, zgui_color::Color},
    prelude::*,
};
#[derive(Clone)]
pub struct Prepared {
    source: Arc<[[f32; 2]]>,
    vertices: Arc<[[f32; 2]]>,
    markers: Arc<[[f32; 2]]>,
}
pub type Paths = BTreeMap<u64, Prepared>;
#[derive(Clone)]
struct Key {
    data: Arc<Tracks>,
    styles: Vec<StyleKey>,
}
type StyleKey = (u64, LinePattern, u64, u64, bool);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.styles == other.styles
    }
}
#[derive(Clone)]
struct Frame {
    key: Key,
    ranges: Ranges,
    pixels: (f64, f64),
}
impl Frame {
    fn current_for(&self, latest: Option<&Self>) -> bool {
        latest.is_some_and(|next| self.covers(next))
    }

    fn covers(&self, next: &Self) -> bool {
        self.key == next.key
            && [
                (self.ranges.x, next.ranges.x, self.pixels.0, next.pixels.0),
                (self.ranges.y, next.ranges.y, self.pixels.1, next.pixels.1),
            ]
            .into_iter()
            .all(|(old, new, pixels, next_pixels)| {
                let scale = pixels / old.span();
                let next_scale = next_pixels / new.span();
                (scale - next_scale).abs() < scale.abs() * 1e-6
                    && new.min >= old.min - old.span() * 0.75
                    && new.max <= old.max + old.span() * 0.75
            })
    }
}
struct Job {
    frame: Frame,
    data: Arc<Tracks>,
}
pub fn patterns(
    workbench: Workbench,
    pane: Pane,
    geometry: Signal<Option<PlotGeometry>, LocalStorage>,
    visible: Signal<bool, LocalStorage>,
) -> RwSignal<Paths, LocalStorage> {
    let paths = RwSignal::new_local(Paths::new());
    let frame = StoredValue::new_local(None::<Frame>);
    let pending = StoredValue::new_local(None::<Job>);
    let wanted = StoredValue::new_local(None::<Frame>);
    let running = RwSignal::new_local(false);
    let effect = zgui::reactive::RenderEffect::new(move |_| {
        if !visible.get() {
            pending.set_value(None);
            wanted.set_value(None);
            return;
        }
        let data = pane.trajectory_data.get();
        let ranges = pane.plot.ranges.get();
        let geometry = geometry.get();
        let drawing = pane.drawing.get();
        let _ = pane.plot.styles.get();
        let Some(data) = data else {
            pending.set_value(None);
            wanted.set_value(None);
            frame.set_value(None);
            paths.set(Paths::new());
            return;
        };
        let Some(g) = geometry else {
            pending.set_value(None);
            wanted.set_value(None);
            return;
        };
        let styles = data
            .traces
            .iter()
            .filter_map(|trace| {
                let draw = drawing.get(&trace.id).copied().unwrap_or_default();
                let pattern = draw.pattern;
                (pattern != LinePattern::None || draw.markers).then(|| {
                    let style = pane.plot.style(&trace.id.to_string());
                    (
                        trace.id,
                        pattern,
                        if pattern == LinePattern::Solid {
                            0
                        } else {
                            style.width.to_bits()
                        },
                        if draw.markers {
                            style.radius.to_bits()
                        } else {
                            0
                        },
                        draw.markers,
                    )
                })
            })
            .collect::<Vec<_>>();
        let next = Frame {
            key: Key {
                data: data.clone(),
                styles,
            },
            ranges,
            pixels: (g.width, g.height),
        };
        wanted.set_value(Some(next.clone()));
        if frame.with_value(|frame| frame.as_ref().is_some_and(|frame| frame.covers(&next))) {
            pending.set_value(None);
            return;
        }
        pending.set_value(Some(Job { frame: next, data }));
        if running.get_untracked() {
            return;
        }
        running.set(true);
        workbench.task(async move {
            loop {
                let mut job = None;
                pending.update_value(|pending| job = pending.take());
                let Some(job) = job else {
                    running.set(false);
                    return;
                };
                let result = plot::preparation::prepare("xy-paths", move || {
                    let paths = job
                        .frame
                        .key
                        .styles
                        .iter()
                        .filter_map(|(id, pattern, width, radius, markers)| {
                            let trace = job.data.traces.iter().find(|trace| trace.id == *id)?;
                            Some((
                                *id,
                                Prepared {
                                    source: trace.track.vertices.clone(),
                                    vertices: pattern_vertices(
                                        &trace.track.points,
                                        trace.track.origin,
                                        job.frame.ranges,
                                        job.frame.pixels,
                                        *pattern,
                                        f64::from_bits(*width),
                                    )
                                    .into(),
                                    markers: if *markers {
                                        marker_vertices(
                                            &trace.track.points,
                                            trace.track.origin,
                                            job.frame.ranges,
                                            job.frame.pixels,
                                            f64::from_bits(*radius),
                                        )
                                        .into()
                                    } else {
                                        Arc::from([])
                                    },
                                },
                            ))
                        })
                        .collect();
                    (job.frame, paths)
                })
                .await;
                if running.try_get_untracked().is_none() {
                    return;
                }
                if visible.try_get_untracked() == Some(true)
                    && wanted.with_value(|latest| result.0.current_for(latest.as_ref()))
                    && pane
                        .trajectory_data
                        .try_get_untracked()
                        .flatten()
                        .is_some_and(|data| Arc::ptr_eq(&data, &result.0.key.data))
                {
                    // A nearby pan can queue another request while this job runs.
                    // If its geometry already covers that request, consume it rather
                    // than rebuilding the same retained buffers a second time.
                    pending.set_value(None);
                    frame.set_value(Some(result.0));
                    paths.set(result.1);
                }
            }
        });
    });
    on_cleanup_local(move || drop(effect));
    paths
}

// Sampling remains XY-specific; dash generation is shared with normal plots.
fn pattern_vertices(
    points: &[PlotPoint],
    origin: PlotPoint,
    ranges: Ranges,
    pixels: (f64, f64),
    pattern: LinePattern,
    width: f64,
) -> Vec<[f32; 2]> {
    let clipped = clip_runs(points, ranges);
    let sampled = sample_xy(&clipped, ranges, pixels);
    plot::line_pattern::prepare_path(&sampled, ranges, pixels, pattern, width)
        .into_iter()
        .map(|point| {
            if point.x.is_finite() && point.y.is_finite() {
                [(point.x - origin.x) as f32, (point.y - origin.y) as f32]
            } else {
                [f32::NAN, f32::NAN]
            }
        })
        .collect()
}

// Keep crossing segments, and a pan margin matching Frame::covers. Never join separated runs.
fn clip_runs(points: &[PlotPoint], ranges: Ranges) -> Vec<PlotPoint> {
    let (left, right) = (
        ranges.x.min - ranges.x.span() * 0.75,
        ranges.x.max + ranges.x.span() * 0.75,
    );
    let (bottom, top) = (
        ranges.y.min - ranges.y.span() * 0.75,
        ranges.y.max + ranges.y.span() * 0.75,
    );
    let mut result = Vec::new();
    let mut previous = None::<PlotPoint>;
    let mut connected = false;
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            previous = None;
            connected = false;
            continue;
        }
        if let Some(start) = previous {
            let rejected = (start.x < left && point.x < left)
                || (start.x > right && point.x > right)
                || (start.y < bottom && point.y < bottom)
                || (start.y > top && point.y > top);
            if !rejected {
                if !connected {
                    if !result.is_empty() {
                        result.push(PlotPoint {
                            x: f64::NAN,
                            y: f64::NAN,
                        });
                    }
                    result.push(start);
                }
                result.push(point);
                connected = true;
            } else {
                connected = false;
            }
        }
        previous = Some(point);
    }
    result
}

fn marker_vertices(
    points: &[PlotPoint],
    origin: PlotPoint,
    ranges: Ranges,
    pixels: (f64, f64),
    radius: f64,
) -> Vec<[f32; 2]> {
    let spacing = (radius * 2.).max(4.);
    let scale = (pixels.0 / ranges.x.span(), pixels.1 / ranges.y.span());
    let mut cells = HashSet::new();
    points
        .iter()
        .filter(|point| {
            point.x.is_finite()
                && point.y.is_finite()
                && point.x >= ranges.x.min - ranges.x.span() * 0.75
                && point.x <= ranges.x.max + ranges.x.span() * 0.75
                && point.y >= ranges.y.min - ranges.y.span() * 0.75
                && point.y <= ranges.y.max + ranges.y.span() * 0.75
        })
        .filter(|point| {
            cells.insert((
                ((point.x - ranges.x.min) * scale.0 / spacing).floor() as i64,
                ((point.y - ranges.y.min) * scale.1 / spacing).floor() as i64,
            ))
        })
        .map(|point| [(point.x - origin.x) as f32, (point.y - origin.y) as f32])
        .collect()
}

// A time-series column reducer cannot be used on loops or vertical XY runs.
// Keep subpixel reduction separate from styling, including the final point of
// each run so short endpoints and gaps survive the sampling adapter.
fn sample_xy(points: &[PlotPoint], ranges: Ranges, pixels: (f64, f64)) -> Vec<PlotPoint> {
    let scale = (pixels.0 / ranges.x.span(), pixels.1 / ranges.y.span());
    let mut out = Vec::<PlotPoint>::new();
    let mut previous = None;
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            if let Some(last) = previous.take() {
                if out.last() != Some(&last) {
                    out.push(last);
                }
                out.push(PlotPoint {
                    x: last.x,
                    y: f64::NAN,
                });
            }
            continue;
        }
        if previous.is_none()
            || out.last().is_none_or(|last| {
                ((point.x - last.x) * scale.0).hypot((point.y - last.y) * scale.1) >= 0.75
            })
        {
            out.push(point);
        }
        previous = Some(point);
    }
    if let Some(last) = previous
        && out.last() != Some(&last)
    {
        out.push(last);
    }
    out
}

pub fn paint(
    scene: &mut CanvasScene,
    trace: &PreparedTrace,
    draw: LineDraw,
    style: SeriesStyle,
    color: Color,
    transform: kurbo::Affine,
    patterns: &Paths,
) {
    let width = style.width;
    let prepared = patterns
        .get(&trace.id)
        .filter(|prepared| Arc::ptr_eq(&prepared.source, &trace.track.vertices));
    match draw.pattern {
        LinePattern::Solid => plot::zgui_renderer::push_line_transformed(
            scene,
            prepared.map_or_else(
                || trace.track.vertices.clone(),
                |prepared| prepared.vertices.clone(),
            ),
            color,
            width,
            zgui::canvas::Lod::Exact,
            transform,
        ),
        LinePattern::None => {}
        _ => {
            if let Some(prepared) = prepared {
                plot::zgui_renderer::push_line_transformed(
                    scene,
                    prepared.vertices.clone(),
                    color,
                    width,
                    zgui::canvas::Lod::Exact,
                    transform,
                );
            }
        }
    }

    if draw.markers {
        let radius = style.radius;
        let marker = match style.marker {
            MarkerShape::Circle => Marker::Circle { radius },
            MarkerShape::Square => Marker::Square { half: radius },
            MarkerShape::Triangle => {
                Marker::Path(std::sync::Arc::new(plot::triangle_marker(radius)))
            }
            MarkerShape::Diamond => Marker::Path(std::sync::Arc::new(plot::diamond_marker(radius))),
            MarkerShape::Cross => {
                Marker::Path(std::sync::Arc::new(plot::cross_marker(radius, radius / 3.)))
            }
        };
        plot::zgui_renderer::push_points_transformed(
            scene,
            prepared.map_or_else(
                || trace.track.vertices.clone(),
                |prepared| prepared.markers.clone(),
            ),
            marker,
            color,
            transform,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_are_bounded_by_screen_cells_and_offscreen_paths_keep_crossings() {
        let points = (0..250_000)
            .map(|index| PlotPoint {
                x: (index % 100) as f64 / 100.,
                y: ((index / 100) % 100) as f64 / 100.,
            })
            .collect::<Vec<_>>();
        let markers = marker_vertices(
            &points,
            PlotPoint { x: 0., y: 0. },
            Ranges::new((0., 1.), (0., 1.)),
            (100., 100.),
            3.,
        );
        assert!(
            markers.len() <= 17 * 17,
            "Marker count must follow screen density, not sample count: {}",
            markers.len()
        );
        let points = [
            PlotPoint { x: -10., y: 0.5 },
            PlotPoint { x: 10., y: 0.5 },
            PlotPoint { x: 20., y: 0.5 },
            PlotPoint {
                x: f64::NAN,
                y: f64::NAN,
            },
            PlotPoint { x: 0.5, y: -10. },
            PlotPoint { x: 0.5, y: 10. },
        ];
        let clipped = clip_runs(&points, Ranges::new((0., 1.), (0., 1.)));
        assert_eq!(clipped.len(), 5);
        assert_eq!(clipped[0], points[0]);
        assert_eq!(clipped[1], points[1]);
        assert!(clipped[2].x.is_nan());
        assert_eq!(clipped[3], points[4]);
        assert_eq!(clipped[4], points[5]);
    }

    #[test]
    fn xy_sampling_keeps_short_endpoints_and_separate_runs() {
        let points = [
            PlotPoint { x: 0., y: 0. },
            PlotPoint { x: 0.01, y: 0. },
            PlotPoint {
                x: f64::NAN,
                y: f64::NAN,
            },
            PlotPoint { x: 0.02, y: 1. },
            PlotPoint { x: 0., y: 1. },
        ];
        let samples = sample_xy(&points, Ranges::new((0., 1.), (0., 1.)), (10., 10.));
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[1], points[1]);
        assert!(samples[2].y.is_nan());
        assert_eq!(samples[3], points[3]);
        assert_eq!(samples[4], points[4]);
    }

    #[test]
    fn dense_loops_reduce_to_a_small_retained_gpu_payload() {
        let points = (0..200_000)
            .map(|index| {
                let angle = index as f64 * std::f64::consts::TAU / 199_999.;
                PlotPoint {
                    x: angle.cos(),
                    y: angle.sin(),
                }
            })
            .collect::<Vec<_>>();
        for pattern in [
            LinePattern::Solid,
            LinePattern::Dashed,
            LinePattern::Dotted,
            LinePattern::DashDot,
        ] {
            let vertices = pattern_vertices(
                &points,
                PlotPoint { x: 0., y: 0. },
                Ranges::new((-1., 1.), (-1., 1.)),
                (1000., 1000.),
                pattern,
                2.,
            );
            assert!(
                vertices.len() < 20_000,
                "Dense XY patterns must be reduced before GPU submission: {}",
                vertices.len()
            );
            assert!(vertices.len() > 100);
        }
    }

    #[test]
    fn nearby_pans_reuse_the_pattern_scale_but_zoom_and_style_changes_rebuild() {
        let data = Arc::new(Tracks::default());
        let original = Frame {
            key: Key {
                data,
                styles: vec![(1, LinePattern::Dashed, 2_f64.to_bits(), 0, false)],
            },
            ranges: Ranges::new((0., 10.), (0., 10.)),
            pixels: (1000., 1000.),
        };
        let mut next = original.clone();
        next.ranges = Ranges::new((5., 15.), (-5., 5.));
        assert!(original.covers(&next));
        assert!(original.current_for(Some(&next)));
        assert!(!original.current_for(None));
        next.ranges = Ranges::new((0., 5.), (0., 5.));
        assert!(!original.covers(&next));
        assert!(!original.current_for(Some(&next)));
        next = original.clone();
        next.key.styles[0].2 = 8_f64.to_bits();
        assert!(!original.covers(&next));
        assert!(!original.current_for(Some(&next)));
        next = original.clone();
        next.key.data = Arc::new(Tracks::default());
        assert!(!original.current_for(Some(&next)));
        let mut distant = original.clone();
        distant.ranges = Ranges::new((100., 110.), (100., 110.));
        assert!(
            !distant.current_for(Some(&original)),
            "An in-flight distant pan must be rejected after returning to the displayed viewport"
        );
    }
}
