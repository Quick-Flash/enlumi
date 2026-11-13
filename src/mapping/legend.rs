//! Map legend: visibility, colors, ordering and full line settings.
use super::{gradient_legend_row::GradientLegendRowProps, telemetry::Coloring};
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
            MapLegendRow(workbench=workbench,pane=pane,id=line.id,name=pane.trajectory.with_untracked(|s|{let s=s.as_ref()?;let index=s.pairs().iter().position(|p|pane.lines.with_untracked(|lines|lines.iter().any(|l|l.id==line.id&&l.source==p.y)))?;s.trace_names.get(&index).cloned()}).unwrap_or_else(||trace_name(&line.name)),colors=colors)
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
) -> impl IntoView {
    use plot::zgui_series::DragLine;
    let mut host = workbench.legend_host();
    host.order = Some(UnsyncCallback::new(move |id| {
        workbench.find(id).map_or_else(Vec::new, |pane| {
            workbench
                .legend_keys(pane)
                .into_iter()
                .filter(|key| *key < (1_u64 << 63))
                .collect()
        })
    }));
    let drag = host.drag;
    let row = NodeRef::new();
    let control = NodeRef::new();
    drag.rows.update_value(|rows| {
        rows.insert((pane.id, id), row);
    });
    on_cleanup_local(move || {
        drag.rows.update_value(|rows| {
            rows.remove(&(pane.id, id));
        });
        if pane.plot.emphasized.get_untracked().as_deref() == Some(&id.to_string()) {
            pane.plot.emphasized.set(None);
        }
        if drag
            .state
            .get_untracked()
            .is_some_and(|s| s.pane == pane.id && s.line == id)
        {
            drag.cancel();
        }
    });
    let hidden = Memo::new(move |_| {
        pane.plot
            .hidden
            .with(|hidden| hidden.contains(&id.to_string()))
    });
    let mode = Memo::new(move |_| trace_color::current(pane, Some(id)).mode);
    let toggle = move || {
        pane.selected.set(Some(id));
        pane.plot.auto_fit.set(false);
        pane.plot.emphasized.set(None);
        pane.plot.hidden.update(|hidden| {
            let key = id.to_string();
            if !hidden.remove(&key) {
                hidden.insert(key);
            }
        });
    };
    let title = name.clone();
    view! {column(class="map-legend-trace",node_ref=row,class:map-trace-hidden=move ||hidden.get(),
        class:legend-dragging=move ||drag.state.get().is_some_and(|s|s.pane==pane.id&&s.line==id&&s.moving),
        class:legend-insert-before=move ||drag.target.get().is_some_and(|t|t.pane==pane.id&&t.before==Some(id)),
        class:legend-insert-after=move ||drag.target.get().is_some_and(|t|t.pane==pane.id&&t.before.is_none())&&host.keys(pane.display()).last()==Some(&id)) {
        row(class="map-legend-row") {
            ActionButton(icon=true,class="map-trace-visibility",a11y:label=format!("Show or hide {title}"),on:click=move |_|toggle()) {
                vector(class="map-eye",prop:viewBox="0 0 16 16",prop:d=move ||PropValue::from(if hidden.get(){"M2 2 L14 14 M6.5 3.5 A7 7 0 0 1 15 8 M10 12 A8 8 0 0 1 1 8 L4 5"}else{"M1 8 Q8 -1 15 8 Q8 17 1 8 Z M5.5 8 A2.5 2.5 0 1 0 10.5 8 A2.5 2.5 0 1 0 5.5 8"}),a11y:hidden=true)
            }
            control(class="map-trace-summary",node_ref=control,tabindex=Focus::Sequential,a11y:role=Role::Button,a11y:label=format!("Show or hide {name}; drag to reorder"),a11y:toggled_on=move ||!hidden.get(),
                on:pointer_enter=move |_|pane.plot.emphasized.set(Some(id.to_string())),on:pointer_leave=move |_|pane.plot.emphasized.set(None),
                on:pointer_down=move |event|{if event.button!=Some(zgui::vocab::PointerButton::Primary){return;}pane.selected.set(Some(id));control.focus();drag.cancel();drag.state.set(Some(DragLine{pane:pane.id,line:id,start:(event.position.x.0,event.position.y.0),moving:false}));event.capture_pointer();event.prevent_default();event.stop_propagation();},
                on:pointer_move=move |event|{host.update(event.position.x.0,event.position.y.0);if drag.state.get_untracked().is_some_and(|s|s.moving){pane.plot.emphasized.set(None);}event.stop_propagation();},
                on:pointer_up=move |event|{host.update(event.position.x.0,event.position.y.0);let source=drag.state.get_untracked();let target=drag.target.get_untracked();drag.cancel();event.release_pointer();event.prevent_default();event.stop_propagation();if let Some(source)=source {if source.moving {if let Some(target)=target {host.actions.move_line.run((source.pane,source.line,target.pane,target.before));}}else{toggle();}}},
                on:pointer_cancel=move |e|{drag.cancel();e.release_pointer();},
                on:key_down=move |event|{use zgui::vocab::{Key,NamedKey};match event.key {Key::Named(NamedKey::Space|NamedKey::Enter)=>{toggle();event.prevent_default();event.stop_propagation();},Key::Named(NamedKey::ArrowUp|NamedKey::ArrowDown)=>{let keys=host.keys(pane.display());if let Some(index)=keys.iter().position(|key|*key==id){let before=match event.key{Key::Named(NamedKey::ArrowUp) if index>0=>Some(keys[index-1]),Key::Named(NamedKey::ArrowDown) if index+1<keys.len()=>keys.get(index+2).copied(),_=>return};host.actions.move_line.run((pane.id,id,pane.id,before));event.prevent_default();event.stop_propagation();}},Key::Named(NamedKey::Escape)=>drag.cancel(),_=>{}}}) {
                row(class="map-trace-swatch") {for bucket in move ||0..4,key=|bucket|*bucket {box(style:background=move ||{let [r,g,b,_]=if mode.get()==Mode::Solid{pane.plot.style(&id.to_string()).color}else{colors.with(|c|c.palette_for(id,(bucket*21) as u8))};Some(format!("#{r:02x}{g:02x}{b:02x}"))}){}}}
                text(class="map-trace-name") {{name}}
            }
            ActionButton(icon=true,class="map-trace-remove",a11y:label=format!("Remove {title}"),on:click=move |_|{let source=pane.lines.with_untracked(|lines|lines.iter().find(|line|line.id==id).map(|line|line.source.clone()));pane.remove_line(id);let builder=workbench.trajectory_builder();if builder.open.get_untracked()&&builder.target.get_untracked()==Some(pane.id)&&let Some(draft)=builder.pane.lines.with_untracked(|lines|lines.iter().find(|line|Some(&line.source)==source.as_ref()).map(|line|line.id)){builder.pane.remove_line(draft);}}) {vector(class="map-eye",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)}
        }
        if move ||!hidden.get()&&colors.with(|c|c.scales.contains_key(&id)){GradientLegendRow(colors=colors,id=id,compact=true)}
    }}
}
