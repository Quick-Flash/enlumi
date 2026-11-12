use crate::state::{Pane, Workbench};
use crate::state_band_key::StateBandKeyProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;
pub fn legend_entries(workbench: Workbench, pane: Pane) -> Vec<plot::SvgLegendEntry> {
    let hidden = pane.hidden_geometry.get_untracked();
    workbench
        .ordered_geometry(pane)
        .into_iter()
        .filter(|item| item.states.is_some() && !hidden.contains(&item.id))
        .map(|item| {
            plot::zgui_plot::theme::LEGEND.entry(
                plot::LegendMarker::Line,
                &item.name,
                crate::plot_appearance::css(item.color),
                true,
            )
        })
        .collect()
}
#[component]
pub fn StateKey(workbench: Workbench, pane: Pane) -> impl IntoView {
    let items = Memo::new(move |_| {
        let hidden = pane.hidden_geometry.get();
        workbench
            .ordered_geometry(pane)
            .into_iter()
            .filter(|item| item.states.is_some() && !hidden.contains(&item.id))
            .collect::<Vec<_>>()
    });
    view! {column(class="state-color-key") {
        for item in move ||items.get(),key=|item|item.id {StateBandKey(workbench=workbench,id=item.id)}
    }}
}
/// Legend form: band names only; rules show on hover.
#[component]
pub fn StateLegend(workbench: Workbench, pane: Pane) -> impl IntoView {
    let items = Memo::new(move |_| {
        let hidden = pane.hidden_geometry.get();
        workbench
            .ordered_geometry(pane)
            .into_iter()
            .filter(|item| item.states.is_some() && !hidden.contains(&item.id))
            .collect::<Vec<_>>()
    });
    view! {column(class="state-color-key") {
        for item in move ||items.get(),key=|item|(item.id,item.name.clone(),item.color) {
            Tooltip {
                TooltipTrigger(class="state-legend-trigger") {
                    row(class="legend-entry state-legend-band") {
                        box(class="state-color-key-swatch",style:background=crate::plot_appearance::css(item.color)){}
                        text{{item.name.clone()}}
                    }
                }
                TooltipContent(class="geometry-state-popup") {
                    column(class="geometry-state-colors") {
                        text(class="heading"){"State modes"}
                        for rule in move ||{workbench.geometry.items.track();workbench.geometry_item(item.id).and_then(|item|item.states).map_or_else(Vec::new,|s|s.rules.clone())},key=|rule|(rule.value,rule.name.clone(),rule.color) {
                            row(class="state-color-key-rule") {
                                box(class="state-color-key-swatch",style:background=crate::plot_appearance::css(rule.color)){}
                                text{{rule.name}}
                            }
                        }
                    }
                }
            }
        }
    }}
}
