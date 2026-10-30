use super::SeriesControlProps;
use crate::plot_data::PlotData;
use crate::zgui_plot::PlotState;
use zgui::prelude::*;

/// Visibility, colors, line widths, and markers for stable series IDs.
#[component]
pub fn SeriesControls(series: Signal<PlotData, LocalStorage>, state: PlotState) -> impl IntoView {
    view! {
        column(class = "series-controls") {
            for label in move || series.get().labels(), key = |label| (label.id.clone(), label.label.clone()) {
                SeriesControl(id = label.id, label = label.label, state = state,
                    line = matches!(series.get_untracked(), PlotData::Time(_)),
                    markers = !matches!(series.get_untracked(), PlotData::Histogram(_)))
            }
        }
    }
}
