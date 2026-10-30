//! Standalone reusable plot consumer.
//! cargo run --release -p zgui-plot --example time_series --features workspace
use zgui::prelude::*;
use zgui_plot::PlotPoint;
use zgui_plot::time_series::{SeriesData, TimeSeries, TimeSeriesSet};
use zgui_plot::zgui_time_series::{TimeSeriesPlotProps, TimeSeriesState};
use zgui_plot::zgui_workspace::TimeSeriesSettingsProps;
use zgui_ui::prelude::*;
use zgui_ui_tokens::prelude::*;
mod support;

fn sources() -> Result<TimeSeriesSet, String> {
    let epoch = 1_800_000_000.0;
    let data = |count: usize, step: f64, cosine: bool| {
        SeriesData::new(
            (0..count)
                .map(|index| {
                    let t = index as f64 * step;
                    PlotPoint {
                        x: epoch + t,
                        y: if (20.0..21.0).contains(&t) {
                            f64::NAN
                        } else if cosine {
                            (t * 0.7).cos()
                        } else {
                            t.sin()
                        },
                    }
                })
                .collect::<Vec<_>>(),
        )
    };
    TimeSeriesSet::new(vec![
        TimeSeries::new(
            "sensor-a/value",
            "Sensor value",
            data(100_000, 0.001, false)?,
        ),
        // Same label is valid: stable IDs distinguish series.
        TimeSeries::new("sensor-b/value", "Sensor value", data(25_000, 0.004, true)?),
    ])
}

#[component]
fn Demo(series: TimeSeriesSet) -> impl IntoView {
    let series = StoredValue::new_local(series);
    let input = Signal::derive_local(move || series.get_value());
    let first = TimeSeriesState::default();
    let second = TimeSeriesState::default();
    let group = zgui_plot::zgui_link::LinkGroup::new();
    let options = {
        zgui_plot::zgui_link::LinkOptions {
            x: true,
            y: false,
            pointer: true,
        }
    };
    view! {
        ThemeProvider(scheme = ColorScheme::Dark) {
            column(class = "demo") {
                row(class = "toolbar") {
                    Button(on:click = move |_| { first.fit_to(&input.get_untracked()); second.fit_to(&input.get_untracked()); }) { "Fit both" }
                    Button(on:click = move |_| {
                        let lod = if first.lod.get_untracked() == zgui::canvas::Lod::Columns { zgui::canvas::Lod::Exact } else { zgui::canvas::Lod::Columns };
                        first.lod.set(lod); second.lod.set(lod);
                    }) {{move || format!("LOD: {:?}", first.lod.get())}}
                    text { "Shared X + pointer; independent Y. Legend toggles series. Double-click fits." }
                }
                TimeSeriesSettings(state = first, series = input)
                row(class = "plots") {
                    Panel(id = 1, group = group, options = options, series = input, state = first)
                    Panel(id = 2, group = group, options = options, series = input, state = second)
                }
            }
        }
    }
}

#[component]
fn Panel(
    id: u64,
    group: zgui_plot::zgui_link::LinkGroup,
    options: zgui_plot::zgui_link::LinkOptions,
    series: Signal<TimeSeriesSet, LocalStorage>,
    state: TimeSeriesState,
) -> impl IntoView {
    provide_local_context(zgui_plot::zgui_link::PlotLinks::with_options(
        id,
        group,
        RwSignal::new_local(options),
    ));
    let plot_options = zgui_plot::zgui_plot::PlotOptions {
        title: Signal::derive_local(move || format!("Plot {id}")),
        format: (zgui::reactive::UnsyncCallback::new(move |(axis, value)| {
            if axis == zgui_plot::labels::Axis::X {
                format!("{:.2}s", value - 1_800_000_000.0)
            } else {
                zgui_plot::format_number(value)
            }
        }))
        .into(),
        ..Default::default()
    };
    view! { TimeSeriesPlot(series = series,
    state = state,
    options = plot_options) }
}

fn main() -> Result<(), zgui::Error> {
    let series = sources().expect("valid demo series");
    let stylesheet = format!(
        "{}\n{}\n{}\n.demo {{ flex: 1; gap: 8px; padding: 8px; background: #12181c; }} .toolbar {{ gap: 8px; align-items: center; }} .plots {{ flex: 1; gap: 8px; min-height: 0; }}",
        zgui_plot::zgui_surface::stylesheet(),
        zgui_plot::zgui_time_series::stylesheet(),
        zgui_plot::zgui_workspace::stylesheet()
    );
    app()
        .with_title("Time series")
        .with_size(1200.0, 640.0)
        .with_stylesheet(stylesheet)
        .run(move || {
            let view = view! { Demo(series = series.clone()) };
            support::force_first_paint("Time series");
            view
        })
}
