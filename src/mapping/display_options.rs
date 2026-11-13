use super::{layer_option::LayerOptionProps, settings::update};
use crate::action_button::ActionButtonProps;
use crate::state::{Pane, Workbench};
use zgui::prelude::*;

#[component]
pub fn MapDisplayOptions(workbench: Workbench, pane: Pane) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    let source = move || {
        state_source(workbench, pane).map_or_else(
            || "Attach a state band to show state markers.".to_owned(),
            |name| format!("State band: {name}"),
        )
    };
    view! {column(class="map-display-options") {
        ActionButton(class="map-modes map-setting-toggle",class:setting-active=move ||map.get().modes,a11y:toggled_on=move ||map.get().modes,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.modes= !map.modes;})) {{move ||if map.get().modes {"State markers: on"} else {"State markers: off"}}}
        text(class="map-mode-source") {{source}}
        LayerOption(class="map-profile-toggle map-layer-option",label="Altitude profile (2D)",checked=Signal::derive_local(move ||map.get().profile),on_change=UnsyncCallback::new(move |value|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.profile=value;})))
    }}
}

pub(super) fn state_source(workbench: Workbench, pane: Pane) -> Option<String> {
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
        .map(|item| item.name)
}
