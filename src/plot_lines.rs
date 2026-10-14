use crate::{
    line_draw::{Connection, prepare},
    state::{Pane, Workbench},
};
use plot::{PlotGeometry, plot_data::MarkerShape, time_series::SeriesData, zgui_renderer::*};
use zgui::{
    canvas::{Lod, Marker, zgui_color::Color},
    prelude::*,
    reactive::RenderEffect,
};

type Prepared = Vec<(u64, SeriesData, SeriesData, Connection)>;
type LineKey = (u64, u64, usize, Connection);
type Plan = (Vec<LineKey>, u64, u64, usize, Lod);

#[component]
pub fn PlotLines(
    workbench: Workbench,
    pane: Pane,
    geometry: Memo<Option<PlotGeometry>>,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let prepared = RwSignal::new_local(Prepared::new());
    let generation = RwSignal::new_local(0_u64);
    let running = RwSignal::new_local(false);
    let cached_plan = StoredValue::new_local(None::<Plan>);
    let plan = Memo::new(move |_| {
        if !visible.get() {
            return None;
        }
        let geometry = geometry.get()?;
        let range = pane.plot.ranges.get().x;
        let hidden = pane.plot.hidden.get();
        let emphasized = pane.plot.emphasized.get();
        let drawing = pane.drawing.get();
        let keys = pane.lines.with(|lines| {
            lines
                .iter()
                .filter(|line| {
                    let key = line.id.to_string();
                    !hidden.contains(&key) || emphasized.as_deref() == Some(&key)
                })
                .map(|line| {
                    (
                        line.id,
                        line.revision,
                        line.data.samples().as_ptr() as usize,
                        drawing
                            .get(&line.id)
                            .copied()
                            .unwrap_or_default()
                            .connection,
                    )
                })
                .collect::<Vec<_>>()
        });
        Some((
            keys,
            range.min.to_bits(),
            range.max.to_bits(),
            geometry.data_width().ceil().clamp(1.0, 8192.0) as usize,
            pane.plot.lod.get(),
        ))
    });
    let prepare_effect = RenderEffect::new(move |_| {
        let request = plan.get();
        generation.update(|value| *value += 1);
        if request.is_none() {
            prepared.set(vec![]);
        }
        if running.get_untracked() {
            return;
        }
        running.set(true);
        workbench.task(async move {
            // Coalesce pans/zoom/style changes: one worker per pane, latest request wins.
            loop {
                let Some(version) = generation.try_get_untracked() else {
                    return;
                };
                let Some((keys, min, max, columns, lod)) = plan.get_untracked() else {
                    prepared.set(vec![]);
                    running.set(false);
                    return;
                };
                let request = (keys.clone(), min, max, columns, lod);
                let old = cached_plan.get_value();
                let cached = prepared.get_untracked();
                let jobs: Vec<_> = pane.lines.with_untracked(|lines| {
                    keys.iter()
                        .filter_map(|&(id, revision, pointer, mode)| {
                            let reuse = old
                                .as_ref()
                                .is_some_and(|old| {
                                    old.1 == min
                                        && old.2 == max
                                        && old.3 == columns
                                        && old.4 == lod
                                        && old.0.contains(&(id, revision, pointer, mode))
                                })
                                .then(|| cached.iter().find(|line| line.0 == id).cloned())
                                .flatten();
                            lines
                                .iter()
                                .find(|line| line.id == id)
                                .map(|line| (id, line.data.clone(), mode, reuse))
                        })
                        .collect()
                });
                let x = plot::Range::new(f64::from_bits(min), f64::from_bits(max));
                let data = blocking(move || {
                    jobs.into_iter()
                        .map(|(id, data, mode, reuse)| {
                            if let Some(reuse) = reuse {
                                return reuse;
                            }
                            let samples = crate::plot_samples::viewport(&data, x, columns, lod);
                            let steps = prepare(&samples, mode);
                            (id, samples, steps, mode)
                        })
                        .collect()
                })
                .await;
                match generation.try_get_untracked() {
                    None => return,
                    Some(current) if current != version => continue,
                    _ => {
                        cached_plan.set_value(Some(request));
                        prepared.set(data);
                        running.set(false);
                        return;
                    }
                }
            }
        });
    });
    on_cleanup_local(move || drop(prepare_effect));
    let handle = zgui::elements::canvas::CanvasHandle::new();
    let draw = handle.clone();
    redraw_on_change(move || {
        let mut data = if visible.get() {
            prepared.get()
        } else {
            vec![]
        };
        let hidden = pane.plot.hidden.get();
        let present = pane
            .lines
            .with(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>());
        let settings = pane.drawing.get();
        let emphasized = pane.plot.emphasized.get();
        // Paint hovered series last, including temporary previews of hidden lines.
        data.sort_by_key(|(id, ..)| emphasized.as_deref() == Some(&id.to_string()));
        let lod = pane.plot.lod.get();
        draw.draw(|scene| {
            scene.clear();
            for (id, original, line, _) in &data {
                let key = id.to_string();
                if (hidden.contains(&key) && emphasized.as_deref() != Some(&key))
                    || !present.contains(id)
                {
                    continue;
                }
                let mut style = pane.plot.style(&key);
                let alpha = if emphasized.as_ref().is_some_and(|hover| hover != &key) {
                    0.3
                } else {
                    1.0
                };
                if emphasized.as_deref() == Some(&key) {
                    style.width += 1.5;
                }
                let [r, g, b, a] = style.color;
                let color = Color::srgb_u8(r, g, b, (f64::from(a) * alpha) as u8);
                push_prepared_line(
                    scene,
                    line,
                    color,
                    style.width.max(0.5),
                    lod,
                    kurbo::Affine::IDENTITY,
                );
                if settings.get(id).copied().unwrap_or_default().markers {
                    let radius = style.radius.clamp(0.5, 20.0);
                    let marker = match style.marker {
                        MarkerShape::Circle => Marker::Circle { radius },
                        MarkerShape::Square => Marker::Square { half: radius },
                        MarkerShape::Triangle => {
                            Marker::Path(std::sync::Arc::new(plot::triangle_marker(radius)))
                        }
                        MarkerShape::Diamond => {
                            Marker::Path(std::sync::Arc::new(plot::diamond_marker(radius)))
                        }
                        MarkerShape::Cross => Marker::Path(std::sync::Arc::new(
                            plot::cross_marker(radius, radius / 3.0),
                        )),
                    };
                    let origin = original.origin();
                    push_points_transformed(
                        scene,
                        original.vertices(),
                        marker,
                        color,
                        kurbo::Affine::translate((origin.x, origin.y)),
                    );
                }
            }
        });
    });
    let view = handle.clone();
    redraw_on_change(move || {
        if visible.get()
            && let Some(geometry) = geometry.get()
        {
            show_ranges(&view, geometry, pane.plot.ranges.get());
        }
    });
    zgui::elements::canvas()
        .class("enlumi-lines")
        .scene(&handle)
}
