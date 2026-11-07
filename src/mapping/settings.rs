use super::{terrain::Orbit, tiles::Provider};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trajectory::Spec,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

pub(super) fn target(workbench: Workbench, pane: Pane) -> Pane {
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
    if pane.id == 0
        && let Some(target) = workbench
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
    if pane.id != 0 {
        let builder = workbench.trajectory_builder();
        if builder.target.get_untracked() == Some(pane.id) {
            builder.pane.trajectory.update(|spec| {
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
    let target = target(workbench, pane);
    if !source.is_empty() && target.id != 0 {
        workbench.activate(target.id);
        workbench.add(source);
    }
}
pub(crate) fn set_provider(workbench: Workbench, pane: Pane, provider: Provider) {
    let edit = |spec: &mut Spec| {
        if let Some(map) = spec.map.as_mut() {
            map.provider = provider;
        }
    };
    update(workbench, pane, edit);
}
#[component]
pub fn MapBackend(
    workbench: Workbench,
    pane: Pane,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    install_stylesheet("enlumi-map", include_str!("preview.css"));
    let provider = Memo::new(move |_| {
        target(workbench, pane)
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map).unwrap_or_default().provider)
    });
    view! {column(class="map-backend-settings") {
        if move ||!compact {text {"Map backend"}}
        Select(value=Binding::controlled(Signal::derive_local(move ||provider.get().key().to_owned()),move |key:String|if let Some(provider)=Provider::parse(&key){set_provider(workbench,pane,provider);})) {
            SelectTrigger(class="map-backend-trigger",label="Map backend"){{move ||provider.get().label()}}
            SelectContent {for provider in move ||Provider::ALL,key=|p|p.key(){SelectItem(value=provider.key()){{provider.label()}}}}
        }
    }}
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
        MapBackend(workbench=workbench,pane=pane)
        row(class="map-coordinate-units") {
            ActionButton(class:setting-active=move ||!map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(false);})){"2D map"}
            ActionButton(class:setting-active=move ||map.get().terrain,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.set_terrain(true);})){"3D map"}
            ActionButton(class:setting-active=move ||map.get().offline,on:click=move |_|{let next=!map.get_untracked().offline;update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.offline=next;});}){{move ||if map.get().offline{"Offline tiles only"}else{"Online tiles"}}}
        }
        row(class="map-coordinate-units") {
            ActionButton(on:click=move |_|{update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.camera=None;});}){"Fit traces"}
            ActionButton(on:click=move |_|service.retry()){"Retry tiles"}
            ActionButton(class:setting-active=move ||target(workbench,pane).timeline_link.get(),on:click=move |_|target(workbench,pane).timeline_link.update(|linked|*linked= !*linked)){"Link timeline"}
            if move ||map.get().terrain {ActionButton(on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.orbit=Orbit::default();})){"Reset orbit"}}
        }
        if move ||map.get().terrain {text(class="muted"){"Drag to orbit; middle-drag or Ctrl-drag to pan; wheel to zoom. Local Z uses one terrain height at trace start."}}
        ActionButton(on:click=move |_|super::preview::open_attribution(map.get_untracked().provider.attribution_url())){{move ||map.get().provider.attribution()}}
        if move ||map.get().terrain {ActionButton(on:click=move |_|super::preview::open_attribution("https://mapterhorn.com/attribution/")){"Terrain: (c) Mapterhorn / source datasets"}}
        text(class="muted",a11y:role=Role::Status){{move ||service.status()}}
    }}
}
#[component]
pub fn AltitudeOptions(
    workbench: Workbench,
    pane: Pane,
    index: usize,
    #[prop(default = false)] force: bool,
) -> impl IntoView {
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
    view! {if move ||force||pane.trajectory.with(|s|s.as_ref().is_some_and(|s|s.map.is_some_and(|m|m.terrain||m.coloring==super::telemetry::ColorBy::Altitude))) {
        column(class="map-altitude-options",node_ref=node) {
            text {"Z / altitude"}
            Input(a11y:label="Z field ID",placeholder="Drop Z field here, or enter field ID",value=Binding::controlled(Signal::derive_local(move ||draft.get()),move |value:String|draft.set(value)),on:key_down=move |e|{if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){bind_altitude(workbench,pane,index,draft.get_untracked().trim().to_owned());}})
            row(class="map-coordinate-units") {
                ActionButton(on:click=move |_|bind_altitude(workbench,pane,index,draft.get_untracked().trim().to_owned())){"Bind Z"}
                ActionButton(on:click=move |_|bind_altitude(workbench,pane,index,String::new())){"Clear Z"}
            }
            row(class="map-coordinate-units") {for mode in move ||[(true,false,"Local Z offset"),(true,true,"First Z on ground"),(false,false,"Global elevation")],key=|mode|mode.2 {
                ActionButton(class:setting-active=move ||{let a=altitude.get();a.local==mode.0&&a.at_first==mode.1},on:click=move |_|update(workbench,pane,|s|{let a=s.altitudes.entry(index).or_default();a.local=mode.0;a.at_first=mode.1;if let Some(map)=s.map.as_mut(){map.camera=None;}})){{mode.2}}
            }}
            row(class="map-coordinate-units") {for unit in move ||[(1.,"Metres"),(0.001,"Millimetres"),(0.3048,"Feet")],key=|unit|unit.1 {
                ActionButton(class:setting-active=move ||altitude.get().scale.abs()==unit.0,on:click=move |_|update(workbench,pane,|s|{let altitude=s.altitudes.entry(index).or_default();altitude.scale=unit.0*altitude.scale.signum();if let Some(map)=s.map.as_mut(){map.camera=None;}})){{unit.1}}
            }}
            ActionButton(class:setting-active=move ||altitude.get().scale<0.,on:click=move |_|update(workbench,pane,|s|{let altitude=s.altitudes.entry(index).or_default();altitude.scale= -altitude.scale;if let Some(map)=s.map.as_mut(){map.camera=None;}})){{move ||if altitude.get().scale<0.{"Z increases down"}else{"Z increases up"}}}
            text(class="muted") {"First Z on ground removes starting altitude. Local Z offset adds Z to terrain. Global elevation uses absolute metres. No Z field: Z=0. Missing altitude samples break trace."}
        }
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_settings_choose_independent_2d_and_3d_map_backends() {
        use crate::{plot_menu::PlotMenuEditorProps, theme::AppThemeProps};
        use std::{cell::RefCell, rc::Rc};
        use zgui::{
            geom::{CssPx, DevicePx, Point, Size},
            platform::SurfaceEvent,
            render::Renderer,
            vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp},
        };
        use zgui_platform_headless::Harness;
        let captured = Rc::new(RefCell::new(None));
        let saved = captured.clone();
        let handler = app()
            .with_size(900., 700.)
            .with_stylesheet(super::super::preview::stylesheet())
            .with_renderer(Box::new(|_, target| {
                let mut renderer = zgui_testkit_scene::CaptureRenderer::new();
                renderer.configure(target);
                Ok(Box::new(renderer))
            }))
            .into_handler(move || {
                let workbench = Workbench::new();
                let first = workbench.pane();
                let second = workbench.add_plot(first.id, true, false);
                for (pane, terrain) in [(first, false), (second, true)] {
                    pane.trajectory.set(Some(Spec {
                        map: Some(super::super::flight::Config {
                            terrain,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }));
                }
                workbench.edit_trajectory(first);
                let first_open = RwSignal::new_local(true);
                let second_open = RwSignal::new_local(false);
                *saved.borrow_mut() = Some((workbench, first, second, first_open, second_open));
                view! {AppTheme(palette=crate::theme::Palette::Forest) {
                    column(class="page workbench") {
                        PlotMenuEditor(workbench=workbench,pane=first,open=first_open)
                        PlotMenuEditor(workbench=workbench,pane=second,open=second_open)
                    }
                }}
            })
            .unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(900.),
            DevicePx(700.),
        )));
        harness.settle(128);
        let (workbench, first, second, first_open, second_open) = captured.borrow().unwrap();
        let click = |harness: &mut Harness<zgui::runtime::Runtime>, class: &str, label: &str| {
            let window = &harness.app().windows()[0];
            let node = window
                .nodes_with_class(class)
                .into_iter()
                .find(|node| window.dom().text_content(*node).contains(label))
                .unwrap();
            let bounds = zgui::view::ViewHost::window_box(window.host().as_ref(), node).unwrap();
            for action in [PointerAction::Pressed, PointerAction::Released] {
                let mut event = PointerEvent::mouse(Point::new(
                    CssPx(bounds.origin.x.0 + bounds.size.width.0 / 2.),
                    CssPx(bounds.origin.y.0 + bounds.size.height.0 / 2.),
                ));
                event.button = Some(PointerButton::Primary);
                harness.deliver_to_first(SurfaceEvent::Pointer {
                    action,
                    event,
                    modifiers: Modifiers::NONE,
                    timestamp: Timestamp::ORIGIN,
                });
            }
            harness.settle(128);
        };
        let provider = |pane: Pane| {
            pane.trajectory
                .get_untracked()
                .unwrap()
                .map
                .unwrap()
                .provider
        };
        click(&mut harness, "map-backend-trigger", "OpenStreetMap");
        click(&mut harness, "zui-select__item", "Google Satellite");
        assert_eq!(provider(first), Provider::GoogleSatellite);
        assert_eq!(provider(second), Provider::OpenStreetMap);
        assert_eq!(
            provider(workbench.trajectory_builder().pane),
            Provider::GoogleSatellite
        );
        first_open.set(false);
        harness.settle(128);
        harness.advance(std::time::Duration::from_secs(1));
        second_open.set(true);
        harness.settle(128);
        click(&mut harness, "map-backend-trigger", "OpenStreetMap");
        click(&mut harness, "zui-select__item", "Esri World Imagery");
        assert_eq!(provider(first), Provider::GoogleSatellite);
        assert_eq!(provider(second), Provider::EsriImagery);
        assert_eq!(
            provider(workbench.trajectory_builder().pane),
            Provider::GoogleSatellite
        );
        for pane in [first, second] {
            let spec = pane.trajectory.get_untracked().unwrap();
            let restored: Spec =
                serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
            assert_eq!(restored.map, spec.map);
        }
        let window = &harness.app().windows()[0];
        assert!(
            window
                .nodes_with_class("map-backend-trigger")
                .into_iter()
                .any(|node| window
                    .dom()
                    .text_content(node)
                    .contains("Esri World Imagery"))
        );
    }
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
                    at_first: true,
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
