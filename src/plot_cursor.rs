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
    let mut host = workbench.legend_host();
    let spectral = pane.analysis.with_untracked(|result| result.is_some());
    if spectral {
        host.actions.extra = Some(UnsyncCallback::new(move |_| workbench.edit_analysis(pane)));
        host.actions.extra_label = "FFT / PSD settings";
        host.actions.extra_class = "legend-analysis-settings";
        host.actions.extra_help = "FFT / PSD settings changes window function and analyzed time range, then recomputes this plot.";
    }
    view! { SeriesPlot(fixed_connection = spectral.then_some(plot::line_draw::Connection::Pre), custom_legend = true, pane = pane.display(), host_actions = host, visible = visible, linked = linked,
    format = UnsyncCallback::new(move |(axis, value)| if axis == plot::labels::Axis::X { format!("{value:.2}{}", if spectral { " Hz" } else { "s" }) } else { plot::format_number(value) }),
    cursor_format = UnsyncCallback::new(move |(x,y)| format!("X: {x:.6}{} | Y: {y:.6}", if spectral { " Hz" } else { "s" })),
    background_layer = move || AnyView::new((!spectral).then(|| AnyView::new(view! { BandBackground(workbench=workbench,pane=pane,visible=visible) }))),
    overlay = move || AnyView::new((!spectral).then(|| AnyView::new(view! { GeometryOverlay(workbench=workbench,pane=pane,visible=visible) }))),
    legend_extra_visible = Signal::derive_local(move || !pane.geometry.with(|ids| ids.is_empty()))) {GeometryLegend(workbench=workbench,pane=pane)} }
}
