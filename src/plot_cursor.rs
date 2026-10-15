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
    cursor_format = UnsyncCallback::new(|(x,y)| format!("X: {x:.6}s | Y: {y:.6}"))) }
}
