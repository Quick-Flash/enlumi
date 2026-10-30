//! Retained cell geometry: prepare tiles on workers, pan/zoom with one matrix.
mod geometry;
use crate::spectral::Heatmap;
#[cfg(test)]
use geometry::Prepared;
use geometry::Tile;
use plot::{PlotGeometry, Ranges, zgui_plot::PlotState};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zgui::{elements::canvas::CanvasHandle, prelude::*};

#[derive(Clone, Copy, PartialEq)]
struct View {
    transform: kurbo::Affine,
    tile: Option<Tile>,
}
fn project(data: &Heatmap, ranges: Ranges, width: f64, height: f64) -> Option<View> {
    let g = PlotGeometry::with_bounds(0., 0., width, height, width, height)?;
    let dx = (data.x_high - data.x_low) / data.columns as f64;
    let dy = (data.y_high - data.y_low) / data.rows as f64;
    let matrix = kurbo::Affine::new([
        dx * g.width / ranges.x.span(),
        0.,
        0.,
        -dy * g.height / ranges.y.span(),
        g.x(data.x_low, ranges.x),
        g.y(data.y_low, ranges.y),
    ]);
    matrix
        .as_coeffs()
        .into_iter()
        .all(f64::is_finite)
        .then(|| View {
            transform: matrix,
            tile: Tile::visible(data, ranges),
        })
}

#[cfg(test)]
#[derive(Clone)]
struct PreparationHook(Arc<PreparationFn>);
#[cfg(test)]
type PreparationFn = dyn Fn(&Heatmap, Tile, &AtomicBool) -> Option<Prepared> + Send + Sync;

fn install(
    handle: CanvasHandle,
    view: Memo<Option<View>>,
    data: Arc<Heatmap>,
) -> RwSignal<bool, LocalStorage> {
    let displayed = StoredValue::new_local(None::<Tile>);
    let wanted = StoredValue::new_local(None::<Tile>);
    let pending = StoredValue::new_local(None::<(Tile, Arc<AtomicBool>)>);
    let active = StoredValue::new_local(None::<(Tile, Arc<AtomicBool>)>);
    let running = RwSignal::new_local(false);
    #[cfg(test)]
    let hook = use_local_context::<PreparationHook>();
    let cancel_pending = move || {
        pending.update_value(|job| {
            if let Some((_, cancel)) = job.take() {
                cancel.store(true, Ordering::Relaxed);
            }
        })
    };
    let cancel_active = move || {
        active.with_value(|job| {
            if let Some((_, cancel)) = job {
                cancel.store(true, Ordering::Relaxed);
            }
        })
    };
    let effect = zgui::reactive::RenderEffect::new(move |_| {
        let view = view.get();
        if let Some(view) = view {
            handle.set_transform(view.transform);
        }
        let tile = view.and_then(|view| view.tile);
        wanted.set_value(tile);
        let Some(tile) = tile else {
            cancel_pending();
            cancel_active();
            return;
        };
        if displayed.with_value(|held| held.is_some_and(|held| held.contains(tile))) {
            cancel_pending();
            cancel_active();
            return;
        }
        if active.with_value(|job| {
            job.as_ref().is_some_and(|(held, cancel)| {
                !cancel.load(Ordering::Relaxed) && held.contains(tile)
            })
        }) {
            cancel_pending();
            return;
        }
        if pending.with_value(|job| job.as_ref().is_some_and(|(held, _)| held.contains(tile))) {
            return;
        }
        cancel_pending();
        cancel_active();
        pending.set_value(Some((tile.padded(&data), Arc::new(AtomicBool::new(false)))));
        if running.get_untracked() {
            return;
        }
        running.set(true);
        let data = data.clone();
        let handle = handle.clone();
        #[cfg(test)]
        let hook = hook.clone();
        spawn_local(async move {
            loop {
                let mut job = None;
                pending.update_value(|pending| job = pending.take());
                let Some((tile, cancel)) = job else {
                    running.set(false);
                    return;
                };
                active.set_value(Some((tile, cancel.clone())));
                let input = data.clone();
                let cancelled = cancel.clone();
                #[cfg(test)]
                let hook = hook.clone();
                let prepared = plot::preparation::prepare("heatmap", move || {
                    #[cfg(test)]
                    if let Some(hook) = hook {
                        return (hook.0)(&input, tile, &cancelled);
                    }
                    geometry::prepare(&input, tile, &cancelled)
                })
                .await;
                if running.try_get_untracked().is_none() {
                    return;
                }
                active.set_value(None);
                if !cancel.load(Ordering::Relaxed)
                    && let Some(prepared) = prepared
                    && wanted.with_value(|wanted| {
                        wanted.is_some_and(|tile| prepared.tile.contains(tile))
                    })
                {
                    handle.draw(|scene| {
                        scene.clear();
                        for shape in prepared.shapes.iter() {
                            scene.push(shape.clone());
                        }
                    });
                    displayed.set_value(Some(prepared.tile));
                }
            }
        });
    });
    on_cleanup_local(move || {
        cancel_pending();
        cancel_active();
        drop(effect);
    });
    running
}

#[component]
pub fn AnalysisHeatmap(data: Heatmap, state: PlotState) -> impl IntoView {
    let host = NodeRef::new();
    let size = host.observe_border_box();
    let data = Arc::new(data);
    let input = data.clone();
    let view = Memo::new(move |_| {
        let size = size.get()?;
        project(
            &input,
            state.ranges.get(),
            f64::from(size.size.width.0 / host.scale()),
            f64::from(size.size.height.0 / host.scale()),
        )
    });
    let handle = CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    let _ = install(handle.clone(), view, data);
    let canvas = zgui::elements::canvas()
        .class("analysis-heatmap-canvas")
        .scene(&handle);
    view! { column(class = "analysis-heatmap", node_ref = host) { {canvas.into_view()} } }
}

#[cfg(test)]
#[path = "analysis_heatmap/tests.rs"]
mod tests;
