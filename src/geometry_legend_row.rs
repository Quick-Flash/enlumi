use crate::{
    action_button::ActionButtonProps,
    geometry::Geometry,
    state::{Pane, Workbench},
};
use plot::zgui_legend::{LegendItemProps, LegendMarker};
use zgui::prelude::*;
#[component]
pub fn GeometryLegendRow(workbench: Workbench, pane: Pane, item: Geometry) -> impl IntoView {
    let id = item.id;
    view! {
            row(class="legend-entry geometry-legend-row",class:geometry-legend-hidden=move || pane.hidden_geometry.with(|ids|ids.contains(&id)),class:geometry-legend-editing=move || workbench.geometry.editing.get() && workbench.geometry.selected.get()==Some(id) && workbench.active.get()==pane.id) {
                row(class="legend-drag-source geometry-legend-toggle",on:pointer_down=move |event|event.stop_propagation()) {
                    LegendItem(label=item.name,color=format!("#{:02x}{:02x}{:02x}",item.color[0],item.color[1],item.color[2]),
                        color_signal=Signal::derive_local(move ||{workbench.geometry.items.track();workbench.geometry_item(id).map_or_else(String::new,|item|format!("#{:02x}{:02x}{:02x}",item.color[0],item.color[1],item.color[2]))}),marker=LegendMarker::Line,
                        visible=Signal::derive_local(move ||!pane.hidden_geometry.with(|ids|ids.contains(&id))),on_toggle=UnsyncCallback::new(move |()|workbench.toggle_geometry(pane,id)),key=id)
                }
                ActionButton(icon=true,class="geometry-legend-edit",a11y:label="Edit geometry",on:pointer_down=move |event|event.stop_propagation(),on:click=move |event|{workbench.edit_geometry_on(pane,id);event.stop_propagation();}) {
                    vector(prop:viewBox="0 0 20 20",prop:d="M4 14 L3 18 L7 17 L17 7 L13 3 Z M11 5 L15 9",a11y:hidden=true)
                }
                ActionButton(icon=true,class="geometry-legend-remove",a11y:label="Remove geometry from plot",on:pointer_down=move |event|event.stop_propagation(),on:click=move |event|{workbench.remove_geometry_from(pane,id);event.stop_propagation();}) {"×"}
            }
    }
}
