use super::{PlotBoundsSettingsProps, SeriesControlsProps};
use crate::plot_data::PlotData;
use crate::zgui_plot::PlotState;
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn DataPlotSettings(state: PlotState, series: Signal<PlotData, LocalStorage>) -> impl IntoView {
    view! {
        column(class = "data-plot-settings") {
            PlotBoundsSettings(state = state, series = series)
            Toggle(size = ToggleSize::Sm, pressed = state.show_annotations) { "Annotations" }
            if move || matches!(series.get(), PlotData::Histogram(_)) {
                row(class = "plot-toolbar") {
                    for bins in move || [20_usize,40,80,160,320,640], key = |bins| *bins {
                        Button(size = ButtonSize::Xs, class:setting-active = move || state.histogram.get().bins == bins,
                            on:click = move |_| state.histogram.update(|options| options.bins = bins)) { {format!("{bins} bins")} }
                    }
                }
                row(class = "plot-toolbar") {
                    Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || state.histogram.get().cdf), move |value| state.histogram.update(|options| options.cdf = value))) { "CDF" }
                    Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || state.histogram.get().ccdf), move |value| state.histogram.update(|options| options.ccdf = value))) { "CCDF" }
                    Toggle(size = ToggleSize::Sm, pressed = Binding::controlled(Signal::derive_local(move || state.histogram.get().normal_fit), move |value| state.histogram.update(|options| options.normal_fit = value))) { "Normal fit" }
                }
            }
            SeriesControls(series = series, state = state)
        }
    }
}
