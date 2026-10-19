use crate::{
    measurement_state::{Measurement, snap},
    state::{Pane, Workbench},
};
use plot::{PlotGeometry, PlotPoint, zgui_renderer::local_position};
use zgui::prelude::*;

#[component]
pub fn MeasurementOverlay(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    install_stylesheet("enlumi-measurement", include_str!("measurement.css"));
    let ui = workbench.measurement;
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let dragging = RwSignal::new_local(false);
    let cancel = zgui::reactive::RenderEffect::new(move |_| {
        if !ui.active.get() {
            dragging.set(false);
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
    let point_at = move |position, line| {
        let (x, y) = local_position(&host, position)?;
        let geometry = geometry.get_untracked()?;
        let ranges = pane.plot.ranges.get_untracked();
        let point = PlotPoint {
            x: geometry.x_value(x.clamp(0., geometry.width), ranges.x),
            y: geometry.y_value(y.clamp(0., geometry.height), ranges.y),
        };
        snap(pane, point, geometry, line)
    };
    let selection = Memo::new(move |_| {
        visible
            .get()
            .then(|| {
                ui.selection
                    .get()
                    .filter(|measurement| measurement.pane == pane.id)
            })
            .flatten()
    });
    let pixels = Memo::new(move |_| {
        let selection = selection.get()?;
        let geometry = geometry.get()?;
        let ranges = pane.plot.ranges.get();
        Some((
            geometry.x(selection.start.x, ranges.x),
            geometry.y(selection.start.y, ranges.y),
            geometry.x(selection.end.x, ranges.x),
            geometry.y(selection.end.y, ranges.y),
        ))
    });
    let move_end = move |position| {
        if !dragging.get_untracked() {
            return;
        }
        if let Some(selection) = selection.get_untracked()
            && let Some((_, point)) = point_at(position, Some(selection.line))
        {
            ui.selection.set(Some(Measurement {
                end: point,
                ..selection
            }));
        }
    };
    let frequency_axis = pane.analysis.with_untracked(Option::is_some);
    let mut readout_style = plot::zgui_plot::theme::tick_text();
    readout_style.color = "var(--app-foreground)".into();
    readout_style.outline = Some("#000000".into());
    view! {
        column(class="measurement-overlay",node_ref=host) {
            if move || pixels.get().is_some() {
                for end in move ||[false,true],key=|end|*end {
                    box(class="measurement-point",style:left=move ||pixels.get().map(|(a,_,b,_)|format!("{}px",if end {b-4.}else{a-4.})),style:top=move ||pixels.get().map(|(_,a,_,b)|format!("{}px",if end {b-4.}else{a-4.})))
                }
                text(class="measurement-readout",style=Some(readout_style.css()),a11y:role=Role::Status,style:left=move ||pixels.get().map(|(a,_,b,_)|format!("{}px",a.min(b))),style:width=move ||pixels.get().map(|(a,_,b,_)|format!("{}px",(b-a).abs().max(1.)))) {{move ||selection.get().map_or_else(String::new,|measurement|measurement.readout(frequency_axis))}}
            }
            if move || visible.get() && ui.active.get() {
                text(class="measurement-hint") {"Measure: drag between samples · Release clears"}
                control(class="measurement-picker",tabindex=Focus::Sequential,a11y:label="Drag between data points to measure interval",
                    on:pointer_down=move |event| {
                        if event.button!=Some(zgui::vocab::PointerButton::Primary){return;}
                        if let Some((line,point))=point_at(event.position,None){
                            workbench.activate(pane.id); ui.selection.set(Some(Measurement {pane:pane.id,line,start:point,end:point})); dragging.set(true); event.capture_pointer();
                        }
                        event.prevent_default();event.stop_propagation();
                    },
                    on:pointer_move=move |event| {move_end(event.position);event.stop_propagation();},
                    on:pointer_up=move |event| {
                        if dragging.get_untracked(){move_end(event.position);dragging.set(false);ui.clear();}
                        event.release_pointer();event.prevent_default();
                    },
                    on:pointer_cancel=move |event|{dragging.set(false);ui.clear();event.release_pointer();})
            }
        }
    }
}
