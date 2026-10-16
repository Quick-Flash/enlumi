use crate::{
    geometry_legend_row::GeometryLegendRowProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;
#[component]
pub fn GeometryLegend(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {column(class="geometry-legend") {
        for item in move || {let ids=pane.geometry.get();workbench.geometry.items.with(|items|items.iter().filter(|item|ids.contains(&item.id)).cloned().collect::<Vec<_>>())},key=|item|(item.id,item.name.clone()) {
            GeometryLegendRow(workbench=workbench,pane=pane,item=item)
        }
    }}
}
