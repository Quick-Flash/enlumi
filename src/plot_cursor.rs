use crate::analysis_range::AnalysisRangeProps;
use crate::band_background::BandBackgroundProps;
use crate::geometry_legend::GeometryLegendProps;
use crate::geometry_overlay::GeometryOverlayProps;
use crate::measurement_background::MeasurementBackgroundProps;
use crate::measurement_overlay::MeasurementOverlayProps;
use crate::plot_appearance::PlotBackgroundProps;
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
    // Register before child layers so the default series legend can be replaced
    // before band labels and measurement readouts are added to the description.
    plot::zgui_svg::SnapshotLayers::current_or_new();
    crate::geometry_legend::register_snapshot_legend(workbench, pane);
    let mut host = workbench.legend_host();
    let spectral = pane.analysis.with_untracked(|result| result.is_some());
    if spectral {
        host.actions.extra = Some(UnsyncCallback::new(move |_| workbench.edit_analysis(pane)));
        host.actions.extra_label = "FFT / PSD settings";
        host.actions.extra_class = "legend-analysis-settings";
        host.actions.extra_help = "FFT / PSD settings changes window function and analyzed time range, then recomputes this plot.";
    }
    let plot_options = plot::zgui_series::SeriesPlotOptions {
        snapshot: (pane.snapshot).into(),
        custom_legend: true,
        format: (UnsyncCallback::new(move |(axis, value)| {
            if axis == plot::labels::Axis::X {
                format!("{value:.2}{}", if spectral { " Hz" } else { "s" })
            } else {
                plot::format_number(value)
            }
        }))
        .into(),
        cursor_format: (UnsyncCallback::new(move |(x, y)| {
            format!(
                "X: {x:.6}{} | Y: {y:.6}",
                if spectral { " Hz" } else { "s" }
            )
        }))
        .into(),
        background_layer: Some(ChildrenFn::new(move || {
            AnyView::new((
                view! {PlotBackground(background=pane.background,visible=visible)}.into_view(),
                view! {MeasurementBackground(workbench=workbench,pane=pane,visible=visible)}
                    .into_view(),
                (!spectral)
                    .then(|| {
                        AnyView::new(
                            view! { BandBackground(workbench=workbench,pane=pane,visible=visible) },
                        )
                    })
                    .into_view(),
            ))
        })),
        overlay: Some(ChildrenFn::new(move || {
            AnyView::new((
            (!spectral).then(|| AnyView::new((view! { GeometryOverlay(workbench=workbench,pane=pane,visible=visible) }.into_view(), view! { AnalysisRange(workbench=workbench,pane=pane,visible=visible) }.into_view()))).into_view(),
            view! {MeasurementOverlay(workbench=workbench,pane=pane,visible=visible)}.into_view()
        ))
        })),
        legend_extra_visible: Signal::derive_local(move || {
            !pane.geometry.with(|ids| ids.is_empty())
        }),
        ..Default::default()
    };
    view! { SeriesPlot(pane = pane.display(),
    host_actions = host,
    visible = visible,
    linked = linked,
    options = plot_options) {GeometryLegend(workbench=workbench,pane=pane)} }
}
