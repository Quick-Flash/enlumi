use crate::state::{Pane, Workbench};
use crate::state_band_key::StateBandKeyProps;
use zgui::prelude::*;
pub fn legend_entries(workbench: Workbench, pane: Pane) -> Vec<plot::SvgLegendEntry> {
    let hidden = pane.hidden_geometry.get_untracked();
    workbench
        .ordered_geometry(pane)
        .into_iter()
        .filter(|item| !hidden.contains(&item.id))
        .flat_map(|item| {
            item.states
                .map(|states| {
                    states
                        .rules
                        .iter()
                        .map(|rule| {
                            plot::zgui_plot::theme::LEGEND.entry(
                                plot::LegendMarker::Line,
                                &format!("{}: {}", item.name, rule.name),
                                crate::plot_appearance::css(rule.color),
                                true,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
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
