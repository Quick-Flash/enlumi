use crate::state::{Pane, Workbench};
use plot::{PlotGeometry, zgui_renderer::local_position};
use zgui::prelude::*;

#[component]
pub fn AnalysisRange(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    install_stylesheet("enlumi-analysis", include_str!("analysis.css"));
    let ui = workbench.analysis;
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let drag = RwSignal::new_local(None::<(f64, f64)>);
    let cancel = zgui::reactive::RenderEffect::new(move |_| {
        if !ui.picking_range.get() {
            drag.set(None);
        }
    });
    on_cleanup_local(move || drop(cancel));
    let geometry = Memo::new(move |_| {
        let size = bounds.get()?.size;
        let w = f64::from(size.width.0 / host.scale());
        let h = f64::from(size.height.0 / host.scale());
        (w > 0. && h > 0.)
            .then(|| PlotGeometry::with_bounds(0., 0., w, h, w, h))
            .flatten()
    });
    let time_at = move |position| {
        let (x, _) = local_position(&host, position)?;
        let geometry = geometry.get_untracked()?;
        Some(geometry.x_value(
            x.clamp(geometry.left, geometry.right),
            pane.plot.ranges.get_untracked().x,
        ))
    };
    let selection = Memo::new(move |_| {
        if !visible.get() || !ui.open.get() {
            return None;
        }
        let (a, b) = if ui.picking_range.get()
            && let Some(range) = drag.get()
        {
            range
        } else {
            (
                ui.start.get().parse::<f64>().ok()?,
                ui.end.get().parse::<f64>().ok()?,
            )
        };
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let geometry = geometry.get()?;
        let range = pane.plot.ranges.get().x;
        let left = geometry
            .x(a.min(b), range)
            .clamp(geometry.left, geometry.right);
        let right = geometry
            .x(a.max(b), range)
            .clamp(geometry.left, geometry.right);
        (right > left).then_some((left, right - left))
    });
    view! {
        column(class="analysis-range-overlay",node_ref=host) {
            if move || selection.get().is_some() {
                box(class="analysis-range-selection",style:left=move ||selection.get().map(|(left,_)|format!("{left}px")),style:width=move ||selection.get().map(|(_,width)|format!("{width}px")))
            }
            if move ||visible.get() && ui.open.get() && ui.picking_range.get() {
                control(class="analysis-range-picker",tabindex=Focus::Sequential,a11y:label="Drag to select FFT time range; Escape cancels",
                    on:pointer_down=move |event| {
                        if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                        if let Some(time) = time_at(event.position) { drag.set(Some((time,time))); workbench.activate(pane.id); event.capture_pointer(); }
                        event.prevent_default(); event.stop_propagation();
                    },
                    on:pointer_move=move |event| {
                        if let Some((start,_))=drag.get_untracked() && let Some(end)=time_at(event.position) {drag.set(Some((start,end)));}
                        event.stop_propagation();
                    },
                    on:pointer_up=move |event| {
                        if let Some((start,_))=drag.get_untracked() && let Some(end)=time_at(event.position) && start!=end {
                            ui.start.set(start.min(end).to_string()); ui.end.set(start.max(end).to_string()); ui.picking_range.set(false);
                        }
                        drag.set(None); event.release_pointer(); event.prevent_default(); event.stop_propagation();
                    },
                    on:pointer_cancel=move |event| {drag.set(None);event.release_pointer();},
                    on:key_down=move |event| {if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape){drag.set(None);ui.picking_range.set(false);event.prevent_default();event.stop_propagation();}})
            }
        }
    }
}
