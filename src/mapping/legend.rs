//! Demo-style map legend: visibility, short name, color mode and inline editing.
use super::{
    gradient_legend_row::GradientLegendRowProps, telemetry::Coloring,
    trace_controls::TraceColorControlsProps, trace_markers::TraceMarkersProps,
};
use crate::{
    action_button::ActionButtonProps,
    plot_menu::PlotMenuEditorProps,
    state::{Pane, Workbench},
    state_key::StateLegendProps,
    trace_color::{self, Mode},
};
use std::sync::Arc;
use zgui::prelude::*;

pub(crate) fn trace_name(name: &str) -> String {
    let parts = name.split('/').collect::<Vec<_>>();
    let topic = if name.starts_with("mavlink/") {
        parts
            .last()
            .unwrap_or(&name)
            .split('.')
            .next()
            .unwrap_or(name)
    } else {
        parts.first().copied().unwrap_or(name)
    };
    match topic {
        "vehicle_global_position" | "GLOBAL_POSITION_INT" => "Global position".into(),
        "vehicle_gps_position" | "sensor_gps" | "GPS_RAW_INT" => "GPS raw".into(),
        "vehicle_local_position" | "LOCAL_POSITION_NED" => "Local position".into(),
        _ if parts.len() > 1 && !topic.contains(' ') && !topic.starts_with('@') => {
            let title = topic.trim_start_matches("vehicle_").replace('_', " ");
            let mut letters = title.chars();
            letters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + letters.as_str())
                .unwrap_or_default()
        }
        _ => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::trace_name;
    #[test]
    fn map_names_hide_wire_fields_and_keep_custom_labels() {
        assert_eq!(trace_name("sensor_gps/0/latitude_deg"), "GPS raw");
        assert_eq!(
            trace_name("vehicle_global_position/0/lat"),
            "Global position"
        );
        assert_eq!(
            trace_name("mavlink/1.1/LOCAL_POSITION_NED.y"),
            "Local position"
        );
        assert_eq!(trace_name("my_position/0/east"), "My position");
        assert_eq!(trace_name("Filtered GPS"), "Filtered GPS");
    }
}

#[component]
pub fn MapLegend(
    workbench: Workbench,
    pane: Pane,
    colors: Signal<Arc<Coloring>, LocalStorage>,
) -> impl IntoView {
    let open = RwSignal::new_local(None::<u64>);
    let collapsed = RwSignal::new_local(false);
    let settings = RwSignal::new_local(false);
    plot::zgui_svg::register_description(
        plot::zgui_svg::SnapshotLayer::Foreground,
        move |description| crate::trajectory_plot::describe_legend(workbench, pane, description),
    );
    view! {column(class="map-legend",class:map-legend-collapsed=move ||collapsed.get(),a11y:label="Map traces",on:pointer_down=move |e|e.stop_propagation(),on:pointer_up=move |e|e.stop_propagation()) {
        row(class="legend-heading map-legend-heading") {
            ActionButton(class="legend-collapse",a11y:label="Collapse or expand legend",on:click=move |_|collapsed.update(|value|*value= !*value)) {
                vector(class="legend-chevron",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if collapsed.get(){"M4 2 L8 6 L4 10"}else{"M2 4 L6 8 L10 4"}),a11y:hidden=true)
                text {"Legend"}
            }
            PlotMenuEditor(workbench=workbench,pane=pane,open=settings,legend_trigger=true)
        }
        if move ||!collapsed.get() {
        for line in move || {let order=pane.legend_order.get();let mut lines=pane.view_lines.get();lines.sort_by_key(|line|order.iter().position(|id|*id==line.id).unwrap_or(usize::MAX));lines},key=|line|(line.id,line.name.clone()) {
            MapLegendRow(workbench=workbench,pane=pane,id=line.id,name=trace_name(&line.name),colors=colors,open=open)
        }
        StateLegend(workbench=workbench,pane=pane)
        }
    }}
}

#[component]
fn MapLegendRow(
    workbench: Workbench,
    pane: Pane,
    id: u64,
    name: String,
    colors: Signal<Arc<Coloring>, LocalStorage>,
    open: RwSignal<Option<u64>, LocalStorage>,
) -> impl IntoView {
    let hidden = Memo::new(move |_| {
        pane.plot
            .hidden
            .with(|hidden| hidden.contains(&id.to_string()))
    });
    let mode = Memo::new(move |_| trace_color::current(pane, Some(id)).mode);
    let title = name.clone();
    view! {column(class="map-legend-trace",class:map-trace-hidden=move ||hidden.get()) {
        row(class="map-legend-row",on:pointer_enter=move |_|pane.plot.emphasized.set(Some(id.to_string())),on:pointer_leave=move |_|pane.plot.emphasized.set(None)) {
            ActionButton(icon=true,class="map-trace-visibility",a11y:label=format!("Show or hide {title}"),on:click=move |_|pane.plot.hidden.update(|hidden|{let key=id.to_string();if !hidden.remove(&key){hidden.insert(key);}})) {
                vector(class="map-eye",prop:viewBox="0 0 16 16",prop:d=move ||PropValue::from(if hidden.get(){"M2 2 L14 14 M6.5 3.5 A7 7 0 0 1 15 8 A8 8 0 0 1 12 11 M10 12 A8 8 0 0 1 1 8 L4 5"}else{"M1 8 Q8 -1 15 8 Q8 17 1 8 Z M5.5 8 A2.5 2.5 0 1 0 10.5 8 A2.5 2.5 0 1 0 5.5 8"}),a11y:hidden=true)
            }
            ActionButton(class="map-trace-summary",class:setting-active=move ||open.get()==Some(id),a11y:label=format!("Trace settings: {name}"),on:click=move |_|{pane.selected.set(Some(id));open.update(|open|*open=if *open==Some(id){None}else{Some(id)});}) {
                row(class="map-trace-swatch") {for bucket in move ||0..4,key=|bucket|*bucket {
                    box(style:background=move ||{
                        let [r,g,b,_]=if mode.get()==Mode::Solid {pane.plot.style(&id.to_string()).color}else{colors.with(|colors|colors.palette_for(id,(bucket*21) as u8))};
                        Some(format!("#{r:02x}{g:02x}{b:02x}"))
                    }) {}
                }}
                text(class="map-trace-name") {{name}}
                text(class="map-trace-meta") {{move ||match mode.get(){Mode::Solid=>"solid".to_owned(),Mode::Speed=>"speed".into(),Mode::Height=>"altitude".into(),Mode::Field=>pane.trajectory.with(|s|s.as_ref().map(|s|s.color_source.rsplit('/').next().unwrap_or("field").to_owned()).unwrap_or_default())}}}
            }
            ActionButton(icon=true,class="map-trace-remove",a11y:label=format!("Remove {title}"),on:click=move |_|{
                pane.remove_line(id);
                let builder=workbench.trajectory_builder();
                if builder.open.get_untracked() && builder.target.get_untracked()==Some(pane.id) {workbench.edit_trajectory(pane);}
            }) {vector(class="map-eye",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)}

        }
        if move ||!hidden.get() && colors.with(|colors|colors.scales.contains_key(&id)) {GradientLegendRow(colors=colors,id=id,compact=true)}
        if move ||open.get()==Some(id) {
            column(class="map-legend-editor") {
                text(class="map-layers-heading") {"Color by · or drop a field in trace setup"}
                TraceColorControls(workbench=workbench,pane=pane,selected=Signal::stored_local(Some(id)))
                TraceMarkers(workbench=workbench,pane=pane,selected=Signal::stored_local(Some(id)))
                row(class="map-trace-actions") {
                    ActionButton(class="trace-fit",on:click=move |_|{pane.fit_trace.set(Some(id));super::settings::update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.fit();});}) {"Fit to trace"}
                    ActionButton(class="trace-edit",on:click=move |_|workbench.edit_trajectory(pane)) {"Edit trace…"}
                }
            }
        }
    }}
}
