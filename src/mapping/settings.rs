use super::{terrain::Orbit, tiles::Provider};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trajectory::Spec,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

fn target(workbench: Workbench, pane: Pane) -> Pane {
    if pane.id == 0 {
        workbench
            .trajectory_builder()
            .target
            .get()
            .and_then(|id| workbench.find(id))
            .unwrap_or(pane)
    } else {
        pane
    }
}
pub(crate) fn update(workbench: Workbench, pane: Pane, edit: impl Fn(&mut Spec)) {
    pane.trajectory.update(|spec| {
        if let Some(spec) = spec {
            edit(spec);
        }
    });
    if pane.id == 0 {
        if let Some(target) = workbench
            .trajectory_builder()
            .target
            .get_untracked()
            .and_then(|id| workbench.find(id))
        {
            target.trajectory.update(|spec| {
                if let Some(spec) = spec {
                    edit(spec);
                }
            });
        }
    }
}
pub(crate) fn bind_altitude(workbench: Workbench, pane: Pane, index: usize, source: String) {
    update(workbench, pane, |spec| {
        spec.altitudes.entry(index).or_default().source = source.clone();
        if let Some(map) = spec.map.as_mut()
            && map.terrain
        {
            map.camera = None;
        }
    });
    if !source.is_empty() {
        if let Some(target) = workbench.trajectory_builder().target.get_untracked() {
            workbench.activate(target);
            workbench.add(source);
        }
    }
}
#[component]
pub fn MapSettings(workbench: Workbench, pane: Pane) -> impl IntoView {
    install_stylesheet("enlumi-map", include_str!("preview.css"));
    let map = Memo::new(move |_| {
        target(workbench, pane)
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map))
            .unwrap_or_default()
    });
    let service = workbench.shared_map_tiles();
    view! {column(class="map-sidebar-settings") {
        Select(value=Binding::controlled(Signal::derive_local(move ||map.get().provider.key().to_owned()), move |key:String|if let Some(provider)=Provider::parse(&key){update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.provider=provider;});})) {
            SelectTrigger(label="Map provider"){SelectValue()}
            SelectContent {for provider in move ||Provider::ALL,key=|p|p.key(){SelectItem(value=provider.key()){{provider.label()}}}}
        }
        row(class="map-coordinate-units") {
            ActionButton(class:setting-active=move ||!map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.terrain=false;})){"2D map"}
            ActionButton(class:setting-active=move ||map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){if !map.terrain{map.camera=None;}map.terrain=true;})){"3D map"}
            ActionButton(class:setting-active=move ||map.get().offline,on:click=move |_|{let next=!map.get_untracked().offline;update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.offline=next;});}){{move ||if map.get().offline{"Offline tiles only"}else{"Online tiles"}}}
        }
        row(class="map-coordinate-units") {
            ActionButton(on:click=move |_|{update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.camera=None;});}){"Fit traces"}
            ActionButton(on:click=move |_|service.retry()){"Retry tiles"}
            ActionButton(class:setting-active=move ||target(workbench,pane).timeline_link.get(),on:click=move |_|target(workbench,pane).timeline_link.update(|linked|*linked=!*linked)){"Link timeline"}
            if move ||map.get().terrain {ActionButton(on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.orbit=Orbit::default();})){"Reset orbit"}}
        }
        if move ||map.get().terrain {text(class="muted"){"Drag to orbit; middle-drag or Ctrl-drag to pan; wheel to zoom. Local Z uses one terrain height at trace start."}}
        ActionButton(on:click=move |_|super::preview::open_attribution(map.get_untracked().provider.attribution_url())){{move ||map.get().provider.attribution()}}
        if move ||map.get().terrain {ActionButton(on:click=move |_|super::preview::open_attribution("https://mapterhorn.com/attribution/")){"Terrain: (c) Mapterhorn / source datasets"}}
        text(class="muted",a11y:role=Role::Status){{move ||service.status()}}
    }}
}
#[component]
pub fn AltitudeOptions(workbench: Workbench, pane: Pane, index: usize) -> impl IntoView {
    let node = NodeRef::new();
    pane.altitude_slots.update_value(|slots| {
        slots.insert(index, node);
    });
    on_cleanup_local(move || {
        pane.altitude_slots.update_value(|slots| {
            slots.remove(&index);
        })
    });
    let altitude = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.altitudes.get(&index).cloned()))
            .unwrap_or_default()
    });
    let draft = RwSignal::new_local(altitude.get_untracked().source);
    let sync = zgui::reactive::RenderEffect::new(move |_| draft.set(altitude.get().source));
    on_cleanup_local(move || drop(sync));
    view! {if move ||pane.trajectory.with(|s|s.as_ref().is_some_and(|s|s.map.is_some_and(|m|m.terrain))) {
        column(class="map-altitude-options",node_ref=node) {
            text {"Z / altitude"}
            Input(a11y:label="Z field ID",placeholder="Drop Z field here, or enter field ID",value=Binding::controlled(Signal::derive_local(move ||draft.get()),move |value:String|draft.set(value)),on:key_down=move |e|{if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){bind_altitude(workbench,pane,index,draft.get_untracked().trim().to_owned());}})
            row(class="map-coordinate-units") {
                ActionButton(on:click=move |_|bind_altitude(workbench,pane,index,draft.get_untracked().trim().to_owned())){"Bind Z"}
                ActionButton(on:click=move |_|bind_altitude(workbench,pane,index,String::new())){"Clear Z"}
                ActionButton(class:setting-active=move ||altitude.get().local,on:click=move |_|{let local=!altitude.get_untracked().local;update(workbench,pane,|s|{s.altitudes.entry(index).or_default().local=local;if let Some(map)=s.map.as_mut(){map.camera=None;}});}){{move ||if altitude.get().local{"Local Z: terrain at start"}else{"Global elevation"}}}
            }
            row(class="map-coordinate-units") {for unit in move ||[(1.,"Metres"),(0.001,"Millimetres"),(0.3048,"Feet")],key=|unit|unit.1 {
                ActionButton(class:setting-active=move ||altitude.get().scale.abs()==unit.0,on:click=move |_|update(workbench,pane,|s|{let altitude=s.altitudes.entry(index).or_default();altitude.scale=unit.0*altitude.scale.signum();if let Some(map)=s.map.as_mut(){map.camera=None;}})){{unit.1}}
            }}
            ActionButton(class:setting-active=move ||altitude.get().scale<0.,on:click=move |_|update(workbench,pane,|s|{let altitude=s.altitudes.entry(index).or_default();altitude.scale=-altitude.scale;if let Some(map)=s.map.as_mut(){map.camera=None;}})){{move ||if altitude.get().scale<0.{"Z increases down"}else{"Z increases up"}}}
            text(class="muted") {"No Z field: Z=0. Missing altitude samples break trace."}
        }
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_settings_update_open_map_and_survive_serialization() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let pane = workbench.pane();
        pane.trajectory.set(Some(Spec {
            map: Some(Default::default()),
            x: "x".into(),
            y: "y".into(),
            gps_scale: Some(1.),
            ..Default::default()
        }));
        workbench.edit_trajectory(pane);
        let builder = workbench.trajectory_builder();
        update(workbench, builder.pane, |s| {
            let map = s.map.as_mut().unwrap();
            map.provider = Provider::BingSatellite;
            map.terrain = true;
            map.offline = true;
            map.orbit.bearing = 35.;
            s.altitudes.insert(
                0,
                crate::trajectory::Altitude {
                    source: "z".into(),
                    scale: 0.001,
                    local: true,
                },
            );
        });
        let saved = pane.trajectory.get_untracked().unwrap();
        assert_eq!(saved, builder.pane.trajectory.get_untracked().unwrap());
        let json = serde_json::to_string(&saved).unwrap();
        let mut restored: Spec = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, saved);
        restored.validate().unwrap();
        for source in restored.sources_mut() {
            *source = format!("profile/{source}");
        }
        assert_eq!(restored.altitudes[&0].source, "profile/z");
        assert_eq!(restored.pairs()[0].y, "profile/y");
    }
}
