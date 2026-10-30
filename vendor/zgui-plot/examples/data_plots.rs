//! Shared scatter/histogram consumer, with styles, annotations, links, and export.
//! cargo run --release -p zgui-plot --example data_plots --features workspace,raster
use zgui::prelude::*;
use zgui_plot::plot_data::{
    AnnotationGeometry, HistogramSeries, HistogramSet, PlotAnnotation, PlotData, ScatterSeries,
    ScatterSet,
};
use zgui_plot::zgui_link::{LinkGroup, LinkOptions, PlotLinks};
use zgui_plot::zgui_plot::{DataPlotProps, PlotState};
use zgui_plot::zgui_svg::{PlotSnapshotSource, WorkspaceSnapshot, capture_workspace};
use zgui_plot::zgui_workspace::{DataPlotSettingsProps, LinkControlsProps};
use zgui_plot::{PlotPoint, Range};
use zgui_ui::prelude::*;
use zgui_ui_tokens::prelude::*;
mod support;

fn sources() -> Result<(PlotData, PlotData), String> {
    let samples = |phase: f64| {
        (0..10_000)
            .map(|index| {
                let t = index as f64 * 0.009;
                PlotPoint {
                    x: t.sin() + phase,
                    y: (t * 1.7).cos() * 0.7 + t.sin() * 0.3,
                }
            })
            .collect::<Vec<_>>()
    };
    let a = samples(0.0);
    let b = samples(0.2);
    let histogram = HistogramSet::new(vec![
        HistogramSeries::new("a", "Source A", a.iter().map(|point| point.x)),
        HistogramSeries::new("b", "Source B", b.iter().map(|point| point.x)),
    ])?;
    let scatter = ScatterSet::new(vec![
        ScatterSeries::new("a", "Source A", a)?,
        ScatterSeries::new("b", "Source B", b)?,
    ])?;
    Ok((PlotData::Scatter(scatter), PlotData::Histogram(histogram)))
}

#[component]
fn Demo(scatter: PlotData, histogram: PlotData) -> impl IntoView {
    let scatter = StoredValue::new_local(scatter);
    let histogram = StoredValue::new_local(histogram);
    let group = LinkGroup::new();
    let status = RwSignal::new_local(String::new());
    let saving = RwSignal::new_local(false);
    let snapshots: [PlotSnapshotSource; 2] = std::array::from_fn(|_| StoredValue::new_local(None));
    let nodes = [NodeRef::new(), NodeRef::new()];
    view! {
        ThemeProvider(scheme = ColorScheme::Dark) {
            column(class = "demo") {
                row(class = "toolbar") {
                    Button(disabled = Signal::derive_local(move || saving.get()), on:click = move |_| {
                        let panes=[WorkspaceSnapshot{id:1,node:nodes[0],snapshot:snapshots[0]},WorkspaceSnapshot{id:2,node:nodes[1],snapshot:snapshots[1]}];
                        let svg=capture_workspace(&panes,None,&zgui_plot::workspace::layout::Layout::flat(&[(1,1.0),(2,1.0)],false),"#12181c");
                        match svg {
                            Err(error)=>status.set(error),
                            Ok(svg)=>{
                                saving.set(true);
                                status.set("Saving plot-exports/data-plots...".into());
                                spawn_local(async move {
                                    let result=blocking(move || -> Result<String,String> {
                                        std::fs::create_dir_all("plot-exports").map_err(|error|error.to_string())?;
                                        std::fs::write("plot-exports/data-plots.svg",&svg).map_err(|error|error.to_string())?;
                                        #[cfg(feature="raster")]
                                        std::fs::write("plot-exports/data-plots.png",zgui_plot::raster::svg_to_png(&svg)?).map_err(|error|error.to_string())?;
                                        Ok("Saved plot-exports/data-plots.svg (and PNG with raster feature)".into())
                                    }).await;
                                    let _ = status.try_set(result.unwrap_or_else(|error|format!("Export failed: {error}")));
                                    let _ = saving.try_set(false);
                                });
                            }
                        }
                    }) { "Export visible plots" }
                    text { "Shared X initially; Y units differ. Settings control series and annotations." }
                    text {{move || status.get()}}
                }
                row(class = "panels") {
                    Panel(id = 1, group = group, series = Signal::derive_local(move || scatter.get_value()), snapshot = snapshots[0], node = nodes[0])
                    Panel(id = 2, group = group, series = Signal::derive_local(move || histogram.get_value()), snapshot = snapshots[1], node = nodes[1])
                }
            }
        }
    }
}

#[component]
fn Panel(
    id: u64,
    group: LinkGroup,
    series: Signal<PlotData, LocalStorage>,
    snapshot: PlotSnapshotSource,
    node: NodeRef,
) -> impl IntoView {
    let state = PlotState::default();
    let links = RwSignal::new_local(LinkOptions {
        x: true,
        y: false,
        pointer: false,
    });
    provide_local_context(PlotLinks::with_options(id, group, links));
    state.annotations.set(vec![
        PlotAnnotation {
            label: "Reference".into(),
            geometry: AnnotationGeometry::XThreshold(0.0),
            color: [244, 247, 245, 255],
            width: 1.5,
            dashed: true,
        },
        PlotAnnotation {
            label: "Band".into(),
            geometry: AnnotationGeometry::XBand(Range::new(-0.4, -0.2)),
            color: [237, 145, 134, 45],
            width: 1.0,
            dashed: false,
        },
    ]);
    if id == 1 {
        state.annotations.update(|annotations| {
            annotations.push(PlotAnnotation {
                label: "Example fit".into(),
                geometry: AnnotationGeometry::FitLine {
                    slope: 0.3,
                    intercept: 0.0,
                },
                color: [238, 190, 96, 255],
                width: 2.0,
                dashed: false,
            })
        });
    }
    let plot_options = zgui_plot::zgui_plot::PlotOptions {
        snapshot: Some(snapshot),
        title: Signal::derive_local(move || {
            if id == 1 {
                "Scatter".into()
            } else {
                "Histogram".into()
            }
        }),
        ..Default::default()
    };
    view! {
            column(class = "panel", node_ref = node) {
                row(class = "toolbar") {
                    text { {if id==1{"Scatter"}else{"Histogram"}} }
                    Popover {
                        PopoverTrigger { "Settings" }
                        PopoverContent(class = "plot-settings-surface") {
                            LinkControls(options = links)
                            DataPlotSettings(state = state, series = series)
                        }
                    }
                }
                DataPlot(series = series,
    state = state,
    options = plot_options)
                text(class = "readout") {{move || state.hover.get().map(|hit|format!("{} sample {}: {}, {}",hit.series_id,hit.sample_index,zgui_plot::format_number(hit.point.x),zgui_plot::format_number(hit.point.y)))
                    .or_else(||state.bin_hover.get().map(|hit|format!("{} .. {}: {} ({:.2}%)",zgui_plot::format_number(hit.bin.start),zgui_plot::format_number(hit.bin.end),hit.bin.count,hit.percent))).unwrap_or_default()}}
            }
        }
}
fn main() -> Result<(), zgui::Error> {
    let (scatter, histogram) = sources().expect("valid demo sources");
    let stylesheet = format!(
        "{}\n{}\n{}\n.demo {{flex:1;padding:8px;gap:8px;background:#12181c;}} .toolbar {{gap:8px;align-items:center;}} .panels {{flex:1;gap:8px;min-height:0;}} .panel {{flex:1;min-width:0;min-height:0;gap:4px;}} .readout {{height:20px;color:#d7e1d9;font-size:12px;}}",
        zgui_plot::zgui_surface::stylesheet(),
        zgui_plot::zgui_plot::stylesheet(),
        zgui_plot::zgui_workspace::stylesheet()
    );
    app()
        .with_title("Data plots")
        .with_size(1280.0, 720.0)
        .with_stylesheet(stylesheet)
        .run(move || {
            let view = view! {Demo(scatter = scatter.clone(),histogram = histogram.clone())};
            support::force_first_paint("Data plots");
            view
        })
}
