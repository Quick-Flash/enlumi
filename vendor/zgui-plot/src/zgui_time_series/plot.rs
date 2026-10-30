use crate::plot_data::PlotData;
use crate::time_series::TimeSeriesSet;
use crate::zgui_plot::{DataPlotProps, PlotState};
use zgui::prelude::*;

#[component]
pub fn TimeSeriesPlot(
    series: Signal<TimeSeriesSet, LocalStorage>,
    state: PlotState,
    #[prop(default = crate::zgui_plot::PlotOptions::default())]
    options: crate::zgui_plot::PlotOptions,
    #[prop(optional)] background_layer: Option<ChildrenFn>,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    let data = Signal::derive_local(move || PlotData::Time(series.get()));
    view! { DataPlot(series = data,
    state = state,
    options = options,
    background_layer = move || AnyView::new(background_layer.as_ref().map(|layer| layer.view())),
    {..attrs}) }
}
