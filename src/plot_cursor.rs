use crate::band_background::BandBackgroundProps;
use crate::geometry_legend::GeometryLegendProps;
use crate::geometry_overlay::GeometryOverlayProps;
use crate::state::{Pane, Workbench};
use plot::{PlotPoint, zgui_series::SeriesPlotProps};
use zgui::prelude::*;
#[component]
pub fn CursorPlot(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
    linked: RwSignal<Option<(u64, PlotPoint)>, LocalStorage>,
) -> impl IntoView {
    let host = workbench.legend_host();
    view! { SeriesPlot(pane = pane.display(), host_actions = host, visible = visible, linked = linked,
    format = UnsyncCallback::new(|(axis, value)| if axis == plot::labels::Axis::X { format!("{value:.2}s") } else { plot::format_number(value) }),
    cursor_format = UnsyncCallback::new(|(x,y)| format!("X: {x:.6}s | Y: {y:.6}")),
    background_layer = move || AnyView::new(view! { BandBackground(workbench=workbench,pane=pane,visible=visible) }),
    overlay = move || AnyView::new(view! { GeometryOverlay(workbench=workbench,pane=pane,visible=visible) }),
    legend_extra_visible = Signal::derive_local(move || !pane.geometry.with(|ids| ids.is_empty()))) {GeometryLegend(workbench=workbench,pane=pane)} }
}
