use super::PlotBoundsSettingsProps;
use crate::plot_data::PlotData;
use crate::time_series::TimeSeriesSet;
use crate::zgui_plot::PlotState;
use zgui::prelude::*;
#[component]
pub fn TimeSeriesSettings(
    state: PlotState,
    series: Signal<TimeSeriesSet, LocalStorage>,
) -> impl IntoView {
    view! { PlotBoundsSettings(state = state, series = Signal::derive_local(move || PlotData::Time(series.get()))) }
}
