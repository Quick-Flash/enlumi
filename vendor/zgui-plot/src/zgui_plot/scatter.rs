use crate::labels::Axis;
use crate::plot_data::{PlotData, ScatterSet};
use crate::zgui_plot::{DataPlotProps, PlotState};
use crate::zgui_svg::PlotSnapshotSource;
use zgui::prelude::*;
use zgui::reactive::UnsyncCallback;

#[component]
pub fn ScatterPlot(
    series: Signal<ScatterSet, LocalStorage>,
    state: PlotState,
    #[prop(default = Signal::derive_local(String::new))] title: Signal<String, LocalStorage>,
    #[prop(default = Signal::derive_local(|| true))] visible: Signal<bool, LocalStorage>,
    #[prop(optional)] format: Option<UnsyncCallback<(Axis, f64), String>>,
    #[prop(optional)] snapshot: Option<PlotSnapshotSource>,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    let data = Signal::derive_local(move || PlotData::Scatter(series.get()));
    view! { DataPlot(series = data,
    state = state,
    {..attrs},
    options = { crate::zgui_plot::PlotOptions { title, visible, format, snapshot, ..Default::default() } }) }
}
