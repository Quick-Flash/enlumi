use super::{
    camera::Camera,
    settings::{MapBackendProps, update},
    terrain::Orbit,
};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;
#[component]
pub fn MapToolbar(
    workbench: Workbench,
    pane: Pane,
    camera: RwSignal<Camera, LocalStorage>,
    orbit: RwSignal<Orbit, LocalStorage>,
    settings: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    view! {row(class="map-toolbar",a11y:role=Role::Toolbar,a11y:label="Map controls") {
        MapBackend(workbench=workbench,pane=pane,compact=true)
        ActionButton(class="map-mode-2d",class:setting-active=move ||!map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(false);})){"2D"}
        ActionButton(class="map-mode-3d",class:setting-active=move ||map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(true);})){"3D"}
        ActionButton(class="map-fit",a11y:label="Fit all visible traces",on:click=move |_|pane.fit_axes(plot::interaction::Axes::BOTH)){"Fit"}
        ActionButton(class="map-reset",a11y:label="Reset map framing and orbit",on:click=move |_|{if map.get_untracked().terrain {orbit.set(Orbit::default());}pane.fit_axes(plot::interaction::Axes::BOTH);}){"Reset"}
        ActionButton(class="map-follow",class:setting-active=move ||map.get().follow,a11y:label="Follow timeline marker; manual navigation pauses follow",on:click=move |_|{pane.timeline_link.set(true);update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.follow= !map.follow;});}){{move ||if map.get().follow{"Following"}else{"Follow marker"}}}
        if move ||map.get().terrain {ActionButton(class="map-ground-projection",class:setting-active=move ||map.get().ground_projection,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.ground_projection= !map.ground_projection;})){"Ground path"}}
        ActionButton(class="map-compass",a11y:label="Compass: restore north-up bearing",on:click=move |_|if map.get_untracked().terrain {orbit.update(|orbit|orbit.bearing=0.);}) {
            vector(class="map-compass-arrow",prop:viewBox="0 0 20 20",prop:d="M10 1 L16 17 L10 13 L4 17 Z",style:transform=move ||Some(format!("rotate({}deg)",if map.get().terrain {-orbit.get().bearing} else {0.})))
            text{{move ||if map.get().terrain {format!("N · {:.0}°",orbit.get().bearing.rem_euclid(360.))}else{"N".into()}}}
        }
        text(class="map-toolbar-zoom"){{move ||format!("z{:.1}",camera.get().zoom)}}
        ActionButton(class="map-toolbar-settings",on:click=move |_|settings.set(true)){"Settings"}
    }}
}
