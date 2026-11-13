//! On-map controls: mode and layers at top left, navigation at bottom right.
use super::{
    camera::{Camera, MAX_ZOOM},
    display_options::MapDisplayOptionsProps,
    flight::Trail,
    layer_option::LayerOptionProps,
    settings::{MapBackendProps, update},
    terrain::Orbit,
};
use crate::{
    action_button::{ActionButtonProps, IconActionProps},
    state::{Pane, Workbench},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

const ZOOM_IN: &str = "M8 3 V13 M3 8 H13";
const ZOOM_OUT: &str = "M3 8 H13";
const FIT: &str = "M2 6 V2 H6 M14 6 V2 H10 M2 10 V14 H6 M14 10 V14 H10";
const FOLLOW: &str =
    "M8 5 A3 3 0 1 1 8 11 A3 3 0 1 1 8 5 M8 1 V3.5 M8 12.5 V15 M1 8 H3.5 M12.5 8 H15";
const MEASURE: &str =
    "M2 11.5 L11.5 2 L14 4.5 L4.5 14 Z M5 9.5 L6.2 10.7 M7 7.5 L8.2 8.7 M9 5.5 L10.2 6.7";
const LAYERS: &str = "M8 2 L14 5.2 L8 8.4 L2 5.2 Z M2 8.4 L8 11.6 L14 8.4 M2 11.2 L8 14.4 L14 11.2";

fn label(text: &'static str) -> Signal<String, LocalStorage> {
    Signal::stored_local(text.to_owned())
}
fn icon(path: &'static str) -> Signal<String, LocalStorage> {
    Signal::stored_local(path.to_owned())
}

#[component]
pub fn MapToolbar(
    workbench: Workbench,
    pane: Pane,
    orbit: RwSignal<Orbit, LocalStorage>,
    camera: RwSignal<Camera, LocalStorage>,
    measuring: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    let zoom = move |step: f64| {
        let mut next = camera.get_untracked();
        next.zoom = (next.zoom + step).clamp(0., MAX_ZOOM);
        camera.set(next);
    };
    view! {
        row(class="map-toolbar",a11y:role=Role::Toolbar,a11y:label="Map mode and layers") {
            ActionButton(class="map-mode-2d",class:setting-active=move ||!map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(false);})){"Map"}
            ActionButton(class="map-mode-3d",class:setting-active=move ||map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(true);})){"3D"}
            box(class="map-toolbar-divider") {}
            MapLayers(workbench=workbench,pane=pane)
        }
        column(class="map-nav",a11y:role=Role::Toolbar,a11y:label="Map navigation") {
            IconAction(class="map-zoom-in",label=label("Zoom in"),path=icon(ZOOM_IN),on_click=UnsyncCallback::new(move |()|zoom(1.)))
            IconAction(class="map-zoom-out",label=label("Zoom out"),path=icon(ZOOM_OUT),on_click=UnsyncCallback::new(move |()|zoom(-1.)))
            box(class="map-nav-divider") {}
                ActionButton(icon=true,class="map-compass icon-action",a11y:label="Compass: restore north-up bearing",on:click=move |_|orbit.update(|orbit|orbit.bearing=0.)) {
                    vector(class="map-compass-arrow",prop:svg=r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 1.5l3 7H5z" fill="#e07a5f"/><path d="M8 14.5l-3-6h6z" fill="none" stroke="currentColor"/></svg>"##,style:transform=move ||Some(format!("rotate({}deg)",if map.get().terrain{-orbit.get().bearing}else{0.})))
                }
            IconAction(class="map-measure-tool",label=label("Measure (M)"),path=icon(MEASURE),active=Signal::derive_local(move ||measuring.get()),on_click=UnsyncCallback::new(move |()|{workbench.measurement.clear();measuring.update(|on|*on= !*on);}))
            IconAction(class="map-fit",label=label("Fit traces"),path=icon(FIT),on_click=UnsyncCallback::new(move |()|pane.fit_axes(plot::interaction::Axes::BOTH)))
            IconAction(class="map-follow",label=label("Follow timeline marker"),path=icon(FOLLOW),active=Signal::derive_local(move ||map.get().follow),on_click=UnsyncCallback::new(move |()|{pane.timeline_link.set(true);update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.follow= !map.follow;});}))
        }
    }
}

/// Basemap, 3D and trace display options in one popover.
#[component]
fn MapLayers(workbench: Workbench, pane: Pane) -> impl IntoView {
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    let service = workbench.shared_map_tiles();
    let edit = move |change: fn(&mut super::flight::Config)| {
        update(workbench, pane, move |s| {
            if let Some(map) = s.map.as_mut() {
                change(map);
            }
        });
    };
    let trail = move |trail: Trail| {
        update(workbench, pane, move |s| {
            if let Some(map) = s.map.as_mut() {
                map.trail = trail;
            }
        });
    };
    view! {Popover {
        PopoverTrigger(class="app-action map-layers icon-action",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Map layers") {
            vector(class="icon-action-glyph",prop:viewBox="0 0 16 16",prop:d=LAYERS,a11y:hidden=true)
        }
        PopoverContent(class="map-layers-menu") {
            row(class="map-layers-columns") {
                column(class="map-layers-column") {
                    MapBackend(workbench=workbench,pane=pane)
                    LayerOption(class="map-offline map-layer-option",label="Offline tiles only",checked=Signal::derive_local(move ||map.get().offline),on_change=UnsyncCallback::new(move |_|edit(|m|m.offline= !m.offline)))
                    text(class="map-layers-heading"){"3D"}
                    LayerOption(class="map-ground-projection map-layer-option",label="Ground path",checked=Signal::derive_local(move ||map.get().ground_projection),on_change=UnsyncCallback::new(move |_|edit(|m|m.ground_projection= !m.ground_projection)))
                    LayerOption(class="map-exaggeration map-layer-option",label="Terrain height ×2",checked=Signal::derive_local(move ||map.get().orbit.exaggeration>1.),on_change=UnsyncCallback::new(move |_|edit(|m|m.orbit.exaggeration=if m.orbit.exaggeration>1.{1.}else{2.})))
                    text(class="map-tile-status",a11y:role=Role::Status){{move ||service.summary()}}
                    ActionButton(class="map-retry",on:click=move |_|service.retry()){"Retry tiles"}
                }
                column(class="map-layers-column") {
                    text(class="map-layers-heading"){"Trace"}
                    MapDisplayOptions(workbench=workbench,pane=pane)
                    text(class="map-layers-heading"){"Show path"}
                    RadioGroup(class="map-trail-options",label="Show path",value=Binding::controlled(Signal::derive_local(move ||match map.get().trail{Trail::Full=>"full",Trail::Flown=>"flown",Trail::Recent=>"recent"}.to_owned()),move |value:String|trail(match value.as_str(){"flown"=>Trail::Flown,"recent"=>Trail::Recent,_=>Trail::Full}))) {
                        for choice in move ||[("full","Whole flight"),("flown","Flown so far"),("recent","Last 30 s")],key=|(value,_)|*value {
                            {{let control=NodeRef::new();view! {row(class="map-layer-option") {RadioGroupItem(node_ref=control,value=choice.0,label=choice.1) Label(control=control,on:click=move |_|trail(match choice.0 {"flown"=>Trail::Flown,"recent"=>Trail::Recent,_=>Trail::Full})){{choice.1}}}}}}
                        }
                    }
                }
            }
        }
    }}
}
