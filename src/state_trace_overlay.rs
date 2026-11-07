//! Translucent state strokes over time-series data; background bands and labels stay intact.
use crate::{
    state::{Pane, Workbench},
    state_tint::Tint,
};
use plot::{PlotPoint, Ranges, line_draw::LineDraw, plot_data::SeriesStyle};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use zgui::{
    canvas::{Marker, zgui_color::Color},
    elements::canvas::CanvasHandle,
    prelude::*,
};
struct Paint {
    points: Arc<[[f32; 2]]>,
    origin: PlotPoint,
    color: [u8; 4],
    width: f64,
    marker: Option<Marker>,
}
fn marker(style: SeriesStyle) -> Marker {
    match style.marker {
        plot::plot_data::MarkerShape::Circle => Marker::Circle {
            radius: style.radius,
        },
        plot::plot_data::MarkerShape::Square => Marker::Square { half: style.radius },
        plot::plot_data::MarkerShape::Triangle => {
            Marker::Path(Arc::new(plot::triangle_marker(style.radius)))
        }
        plot::plot_data::MarkerShape::Diamond => {
            Marker::Path(Arc::new(plot::diamond_marker(style.radius)))
        }
        plot::plot_data::MarkerShape::Cross => Marker::Path(Arc::new(plot::cross_marker(
            style.radius,
            style.radius / 3.,
        ))),
    }
}
fn prepare(
    lines: Vec<(crate::state::Line, SeriesStyle, LineDraw, f64)>,
    tints: Vec<Tint>,
    ranges: Ranges,
    pixels: (f64, f64),
    accuracy: plot::series_samples::Accuracy,
) -> Vec<Paint> {
    let mut output = Vec::new();
    for (line, style, draw, emphasis) in lines {
        let reduced = plot::series_samples::pan_viewport(
            &line.data,
            ranges.x,
            pixels.0.ceil() as usize,
            accuracy,
        );
        let connected = plot::line_draw::prepare(&reduced, draw.connection);
        let path = plot::line_pattern::prepare_path(
            connected.samples(),
            ranges,
            pixels,
            draw.pattern,
            style.width,
        );
        let origin = line.data.origin();
        let local = |p: PlotPoint| [(p.x - origin.x) as f32, (p.y - origin.y) as f32];
        for tint in &tints {
            let mut groups = std::collections::BTreeMap::<[u8; 3], Vec<[f32; 2]>>::new();
            for pair in path
                .windows(2)
                .filter(|p| p.iter().all(|p| p.x.is_finite() && p.y.is_finite()))
            {
                let (a, b) = (pair[0], pair[1]);
                let start = tint.states.intervals.partition_point(|i| i.end < a.x);
                for interval in tint
                    .states
                    .intervals
                    .iter()
                    .skip(start)
                    .take_while(|i| i.start <= b.x)
                {
                    if interval.end <= a.x || interval.start > b.x {
                        continue;
                    }
                    let Some(rule) = tint.states.rules.iter().find(|r| r.value == interval.value)
                    else {
                        continue;
                    };
                    let lo = a.x.max(interval.start);
                    let hi = b.x.min(interval.end);
                    if hi < lo {
                        continue;
                    }
                    let point = |x: f64| PlotPoint {
                        x,
                        y: if b.x == a.x {
                            a.y
                        } else {
                            a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x)
                        },
                    };
                    let (left, right) = if b.x == a.x {
                        (a, b)
                    } else {
                        (point(lo), point(hi))
                    };
                    groups.entry(rule.color).or_default().extend([
                        local(left),
                        local(right),
                        [f32::NAN, f32::NAN],
                    ]);
                }
            }
            for (color, points) in groups {
                output.push(Paint {
                    points: points.into(),
                    origin,
                    color: [
                        color[0],
                        color[1],
                        color[2],
                        (tint.opacity * f64::from(style.color[3]) * emphasis).round() as u8,
                    ],
                    width: style.width,
                    marker: None,
                });
            }
            if draw.markers {
                let mut groups = std::collections::BTreeMap::<[u8; 3], Vec<[f32; 2]>>::new();
                for &p in reduced.samples().iter().filter(|p| p.y.is_finite()) {
                    let index = tint.states.intervals.partition_point(|i| i.end <= p.x);
                    if let Some(interval) = tint
                        .states
                        .intervals
                        .get(index)
                        .filter(|i| i.start <= p.x && p.x < i.end)
                        && let Some(rule) =
                            tint.states.rules.iter().find(|r| r.value == interval.value)
                    {
                        groups.entry(rule.color).or_default().push(local(p));
                    }
                }
                for (color, points) in groups {
                    output.push(Paint {
                        points: points.into(),
                        origin,
                        color: [
                            color[0],
                            color[1],
                            color[2],
                            (tint.opacity * f64::from(style.color[3]) * emphasis).round() as u8,
                        ],
                        width: style.width,
                        marker: Some(marker(style)),
                    });
                }
            }
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_clips_states_keeps_gaps_and_uses_band_opacity() {
        let line = crate::state::Line {
            id: 1,
            source: "x".into(),
            name: "x".into(),
            revision: 0,
            data: plot::time_series::SeriesData::new(vec![
                PlotPoint { x: 0., y: 0. },
                PlotPoint { x: 4., y: 4. },
            ])
            .unwrap(),
        };
        let mut style = SeriesStyle::for_id("1");
        style.color = [0, 0, 200, 128];
        let ranges = Ranges::new((0., 4.), (0., 4.));
        let draw = LineDraw::default();
        let tint = crate::state_tint::tests::fixture();
        assert!(
            prepare(
                vec![(line.clone(), style, draw, 1.)],
                vec![],
                ranges,
                (400., 400.),
                plot::series_samples::Accuracy::Exact
            )
            .is_empty()
        );
        let paints = prepare(
            vec![(line, style, draw, 1.)],
            vec![tint],
            ranges,
            (400., 400.),
            plot::series_samples::Accuracy::Exact,
        );
        assert_eq!(paints.len(), 2);
        for paint in paints {
            assert_eq!(paint.color[3], 32);
            for pair in paint
                .points
                .windows(2)
                .filter(|p| p.iter().flatten().all(|v| v.is_finite()))
            {
                let (a, b) = (pair[0][0], pair[1][0]);
                assert!(b <= 2. || a >= 3., "State gap must stay clear: {a}..{b}");
                if paint.color[0] == 255 {
                    assert!(b <= 1. || a >= 3.);
                } else {
                    assert!(a >= 1. && b <= 2.);
                }
            }
        }
    }
}
#[component]
pub fn StateTraceOverlay(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let handle = CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Foreground);
    let paints = RwSignal::new_local(Arc::new(Vec::<Paint>::new()));
    let version = Arc::new(AtomicU64::new(0));
    let jobs = version.clone();
    let tints = Memo::new(move |_| crate::state_tint::bands(workbench, pane));
    let effect = zgui::reactive::RenderEffect::new(move |_| {
        let token = jobs.fetch_add(1, Ordering::Relaxed) + 1;
        let tints = tints.get();
        if !visible.get() || tints.is_empty() {
            paints.set(Arc::new(Vec::new()));
            return;
        }
        let Some(bounds) = bounds.get() else {
            return;
        };
        let pixels = (
            f64::from(bounds.size.width.0 / host.scale()),
            f64::from(bounds.size.height.0 / host.scale()),
        );
        if pixels.0 <= 0. || pixels.1 <= 0. {
            return;
        }
        let ranges = pane.plot.ranges.get();
        let hidden = pane.plot.hidden.get();
        let emphasized = pane.plot.emphasized.get();
        pane.plot.styles.get();
        let drawing = pane.drawing.get();
        let accuracy = if pane.plot.lod.get() == zgui::canvas::Lod::Exact {
            plot::series_samples::Accuracy::Exact
        } else {
            plot::series_samples::Accuracy::Columns
        };
        let lines = pane
            .lines
            .get()
            .into_iter()
            .filter(|line| {
                !hidden.contains(&line.id.to_string())
                    || emphasized.as_deref() == Some(line.id.to_string().as_str())
            })
            .map(|line| {
                let id = line.id;
                let style = pane.plot.style(&id.to_string());
                let draw = drawing.get(&id).copied().unwrap_or_default();
                let alpha = if emphasized
                    .as_ref()
                    .is_some_and(|key| *key != id.to_string())
                {
                    0.3
                } else {
                    1.
                };
                (line, style, draw, alpha)
            })
            .collect();
        let current = jobs.clone();
        workbench.task(async move {
            let job = current.clone();
            let result = plot::preparation::prepare("state-trace-tint", move || {
                if job.load(Ordering::Relaxed) == token {
                    prepare(lines, tints, ranges, pixels, accuracy)
                } else {
                    Vec::new()
                }
            })
            .await;
            if current.load(Ordering::Relaxed) == token && paints.try_get_untracked().is_some() {
                paints.set(Arc::new(result));
            }
        });
    });
    on_cleanup_local(move || {
        version.fetch_add(1, Ordering::Relaxed);
        drop(effect);
    });
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        let bounds = bounds.get();
        let shown = visible.get();
        let paints = paints.get();
        let ranges = pane.plot.ranges.get();
        if shown && let Some(bounds) = bounds {
            let (w, h) = (
                f64::from(bounds.size.width.0 / host.scale()),
                f64::from(bounds.size.height.0 / host.scale()),
            );
            if let Some(g) = plot::PlotGeometry::with_bounds(0., 0., w, h, w, h) {
                plot::zgui_renderer::show_ranges(&drawing, g, ranges);
            }
        }
        drawing.draw(|scene| {
            scene.clear();
            if !shown {
                return;
            }
            let Some(bounds) = bounds else {
                return;
            };
            let (w, h) = (
                f64::from(bounds.size.width.0 / host.scale()),
                f64::from(bounds.size.height.0 / host.scale()),
            );
            let Some(_g) = plot::PlotGeometry::with_bounds(0., 0., w, h, w, h) else {
                return;
            };
            for paint in paints.iter() {
                let [r, gc, b, a] = paint.color;
                let color = Color::srgb_u8(r, gc, b, a);
                let transform = kurbo::Affine::translate((paint.origin.x, paint.origin.y));
                if let Some(marker) = &paint.marker {
                    plot::zgui_renderer::push_points_transformed(
                        scene,
                        paint.points.clone(),
                        marker.clone(),
                        color,
                        transform,
                    );
                } else {
                    plot::zgui_renderer::push_line_transformed(
                        scene,
                        paint.points.clone(),
                        color,
                        paint.width,
                        zgui::canvas::Lod::Exact,
                        transform,
                    );
                }
            }
        });
    });
    let canvas = zgui::elements::canvas()
        .class("state-trace-tint-canvas")
        .scene(&handle);
    view! {column(class="state-trace-tint",node_ref=host){{canvas.into_view()}}}
}
