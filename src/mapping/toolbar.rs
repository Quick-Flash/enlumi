use super::{settings::update, terrain::Orbit};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;
#[component]
pub fn MapToolbar(
    workbench: Workbench,
    pane: Pane,
    orbit: RwSignal<Orbit, LocalStorage>,
) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    view! {row(class="map-toolbar",a11y:role=Role::Toolbar,a11y:label="Map controls") {
        ActionButton(class="map-mode-2d",class:setting-active=move ||!map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(false);})){"2D"}
        ActionButton(class="map-mode-3d",class:setting-active=move ||map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(true);})){"3D"}
        ActionButton(class="map-fit",a11y:label="Fit all visible traces",on:click=move |_|pane.fit_axes(plot::interaction::Axes::BOTH)){"Fit"}
        ActionButton(class="map-follow",class:setting-active=move ||map.get().follow,a11y:label="Follow timeline marker; manual navigation pauses follow",on:click=move |_|{pane.timeline_link.set(true);update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.follow= !map.follow;});}){"Follow"}
        ActionButton(class="map-compass",a11y:label="Compass: restore north-up bearing",on:click=move |_|if map.get_untracked().terrain {orbit.update(|orbit|orbit.bearing=0.);}) {
            vector(class="map-compass-arrow",prop:viewBox="0 0 20 20",prop:d="M10 1 L16 17 L10 13 L4 17 Z",style:transform=move ||Some(format!("rotate({}deg)",if map.get().terrain {-orbit.get().bearing} else {0.})))
        }
    }}
}
