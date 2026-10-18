use crate::{
    geometry_legend_row::GeometryLegendRowProps,
    state::{Pane, Workbench},
};
use plot::zgui_series::LegendEntryProps;
use zgui::prelude::*;
#[component]
pub fn GeometryLegend(workbench: Workbench, pane: Pane) -> impl IntoView {
    let host = workbench.legend_host();
    view! { column(class = "geometry-legend") {
        for entry in move || {
            let lines = pane.view_lines.get();
            workbench.geometry.items.track();
            workbench.legend_keys(pane).into_iter().filter_map(|key| {
                let name = if key >= (1_u64 << 63) { workbench.geometry_item(!key).map(|item| item.name) }
                    else { lines.iter().find(|line| line.id == key).map(|line| line.name.clone()) };
                name.map(|name| (key, name))
            }).collect::<Vec<_>>()
        }, key = |entry| entry.clone() {
            {if entry.0 >= (1_u64 << 63) {
                workbench.geometry_item(!entry.0).map(|item| AnyView::new(view! {
                    GeometryLegendRow(workbench = workbench, pane = pane, item = item)
                }))
            } else {
                pane.view_lines.get_untracked().into_iter().find(|line| line.id == entry.0).map(|line| AnyView::new(view! {
                    LegendEntry(host = host, pane = pane.display(), line = line)
                }))
            }}
        }
    }}
}
