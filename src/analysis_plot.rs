use crate::{analysis_heatmap::AnalysisHeatmapProps, analysis_state::AnalysisResult};
use plot::{
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    zgui_plot::PlotState,
    zgui_time_series::TimeSeriesPlotProps,
};
use zgui::prelude::*;

#[component]
pub fn AnalysisPlot(result: AnalysisResult) -> impl IntoView {
    let state = PlotState::default();
    let heatmap = StoredValue::new_local(result.heatmap);
    let is_heatmap = heatmap.with_value(Option::is_some);
    let chart = NodeRef::new();
    let readout = RwSignal::new_local("Hover heatmap to inspect bin".to_owned());
    let series = if let Some(map) = heatmap.get_value() {
        TimeSeriesSet::new(vec![TimeSeries::new(
            "bounds",
            "Heatmap",
            SeriesData::new(vec![
                plot::PlotPoint { x: 0., y: map.low },
                plot::PlotPoint {
                    x: (map.columns - 1) as f64 * map.frequency_step,
                    y: map.high,
                },
            ])
            .expect("ordered heatmap bounds"),
        )])
        .expect("one bounds series")
    } else {
        result.series
    };
    view! {
        column(class = "analysis-result") {
            text(class = "heading") {{result.title}}
            text(class = "muted") {{format!("X: Hz · {}",result.units)}}
            text(class = "analysis-summary muted") {{result.summary}}
            if move || is_heatmap {
                row(class = "analysis-color-key") {
                    text(class = "muted") {{heatmap.with_value(|map| format!("{:.1} dB",map.as_ref().unwrap().min_db))}}
                    box(class = "analysis-color-gradient")
                    text(class = "muted") {{heatmap.with_value(|map|format!("{:.1} dB · blank = no windows/power",map.as_ref().unwrap().max_db))}}
                }
            }
            if move || is_heatmap { text(class="muted") {{move || readout.get()}} }
            column(class="analysis-chart",node_ref=chart,on:pointer_move=move |event| {
                if !is_heatmap {return;}
                let Some((x,y))=plot::zgui_renderer::local_position(&chart,event.position) else {return;};
                let Some(bounds)=chart.window_bounds() else {return;};
                let w=f64::from(bounds.size.width.0/chart.scale());let h=f64::from(bounds.size.height.0/chart.scale());
                let Some(g)=plot::PlotGeometry::with_bounds(0.,0.,w,h,w,h) else {return;};
                let ranges=state.ranges.get_untracked();let frequency=g.x_value(x,ranges.x);let value=g.y_value(y,ranges.y);
                heatmap.with_value(|map| {
                    let Some(map)=map else {return;};
                    if frequency<0. || frequency>(map.columns-1) as f64*map.frequency_step || value<map.low || value>map.high {return;}
                    let column=(frequency/map.frequency_step).round() as usize;
                    let row=(((value-map.low)/(map.high-map.low)*map.rows as f64) as usize).min(map.rows-1);
                    readout.set(format!("{:.2} Hz · Y {:.3} · {}",column as f64*map.frequency_step,value,map.values[row*map.columns+column].map_or_else(||"No windows/power in bin".into(),|db|format!("{db:.2} dB"))));
                });
            }) {
            TimeSeriesPlot(series = Signal::stored_local(series), state = state,
                draw_series = Signal::stored_local(!is_heatmap), show_legend = !is_heatmap,
                show_pointer = !is_heatmap, show_cursor_values = !is_heatmap,
                format = UnsyncCallback::new(|(axis,value)|if axis==plot::labels::Axis::X {format!("{} Hz",plot::format_number(value))}else{plot::format_number(value)}),
                background_layer = move || AnyView::new(heatmap.get_value().map(|data|AnyView::new(view! {AnalysisHeatmap(data=data,state=state)}))))
            }
        }
    }
}
