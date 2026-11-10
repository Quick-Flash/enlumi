use super::{settings::update, terrain::Orbit};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;

#[component]
pub fn MapViewOptions(workbench: Workbench, pane: Pane) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    view! {row(class="map-coordinate-units") {
        if move ||map.get().terrain {
            ActionButton(class="map-ground-projection",class:setting-active=move ||map.get().ground_projection,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.ground_projection= !map.ground_projection;})){"Ground path"}
            ActionButton(class="map-reset",on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.orbit=Orbit::default();map.fit();})){"Reset view"}
        }
    }}
}
