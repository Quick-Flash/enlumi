use crate::{
    geometry_legend_row::GeometryLegendRowProps,
    state::{Pane, Workbench},
};
use plot::zgui_series::LegendEntryProps;
use zgui::prelude::*;

pub fn register_snapshot_legend(workbench: Workbench, pane: Pane) {
    use plot::{
        zgui_plot::theme,
        zgui_svg::{SnapshotLayer, register_description},
    };
    register_description(SnapshotLayer::Foreground, move |description| {
        let lines = pane.view_lines.get_untracked();
        // SeriesPlot exports its series legend after the ticks. This is the
        // first description layer; replace that box and its trailing labels.
        if !lines.is_empty() {
            description.rects.pop();
            description
                .text
                .truncate(description.text.len().saturating_sub(lines.len()));
        }
        let entries = workbench
            .legend_keys(pane)
            .into_iter()
            .filter_map(|key| {
                let (name, color, visible) = if key >= (1_u64 << 63) {
                    let item = workbench.geometry_item(!key)?;
                    (
                        item.name,
                        format!(
                            "#{:02x}{:02x}{:02x}",
                            item.color[0], item.color[1], item.color[2]
                        ),
                        !pane
                            .hidden_geometry
                            .with_untracked(|hidden| hidden.contains(&item.id)),
                    )
                } else {
                    let line = lines.iter().find(|line| line.id == key)?;
                    (
                        line.name.clone(),
                        pane.plot.style(&key.to_string()).css_color(),
                        !pane
                            .plot
                            .hidden
                            .with_untracked(|hidden| hidden.contains(&key.to_string())),
                    )
                };
                Some(theme::LEGEND.entry(plot::LegendMarker::Line, &name, color, visible))
            })
            .collect();
        description.push_legend(
            entries,
            (description.width - theme::LEGEND_RIGHT, theme::LEGEND_TOP),
            &theme::legend_box(),
        );
    });
}

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
