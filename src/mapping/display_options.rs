use super::trace_markers::TraceMarkersProps;
use super::{layer_option::LayerOptionProps, settings::update};
use crate::state::{Pane, Workbench};
use zgui::prelude::*;

#[component]
pub fn MapDisplayOptions(
    workbench: Workbench,
    pane: Pane,
    #[prop(default = false)] editor: bool,
) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    let arrows = Signal::derive_local(move || {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.arrows))
    });
    let source = move || {
        let hidden = pane.hidden_geometry.get();
        workbench
            .ordered_geometry(pane)
            .into_iter()
            .find(|item| {
                !hidden.contains(&item.id)
                    && item
                        .states
                        .as_ref()
                        .is_some_and(|states| states.error.is_none())
            })
            .map_or_else(
                || "Add a state band in line options to show state markers.".to_owned(),
                |item| format!("State band: {}", item.name),
            )
    };
    view! {column(class="map-display-options") {
        LayerOption(class=if editor {"map-arrows-editor map-layer-option"}else{"map-arrows map-layer-option"},label="Direction arrows",checked=arrows,on_change=UnsyncCallback::new(move |value|update(workbench,pane,|s|s.arrows=value)))
        if move ||editor {
            text(class="heading") {{move ||{let selected=crate::trace_color::selected(pane);pane.view_lines.with(|lines|lines.iter().find(|line|Some(line.id)==selected).map_or_else(||"Line markers".to_owned(),|line|format!("Markers: {}",super::legend::trace_name(&line.name))))}}}
            TraceMarkers(workbench=workbench,pane=pane,selected=Signal::derive_local(move ||crate::trace_color::selected(pane)))
        }
        if move ||!editor {
        LayerOption(class="map-modes map-layer-option",label="State markers (all lines)",checked=Signal::derive_local(move ||map.get().modes),on_change=UnsyncCallback::new(move |value|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.modes=value;})))
        text(class="map-mode-source") {{source}}
        LayerOption(class="map-profile-toggle map-layer-option",label="Altitude profile (2D)",checked=Signal::derive_local(move ||map.get().profile),on_change=UnsyncCallback::new(move |value|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.profile=value;})))
        }
    }}
}
