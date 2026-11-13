use super::tiles::Provider;
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trajectory::Spec,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

pub(crate) fn target(workbench: Workbench, pane: Pane) -> Pane {
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
/// Set the map-wide color field; traces switch to field coloring while one is bound.
pub(crate) fn bind_color(workbench: Workbench, pane: Pane, source: String) {
    update(workbench, pane, |spec| spec.color_source = source.clone());
    let target = target(workbench, pane);
    if target.id == 0 {
        return;
    }
    if !source.is_empty() {
        workbench.activate(target.id);
        workbench.add(source.clone());
    }
    let traces = target.trajectory.with_untracked(|s| {
        s.as_ref()
            .map(|s| s.pairs().into_iter().map(|p| p.y).collect::<Vec<_>>())
            .unwrap_or_default()
    });
    for id in target.lines.with_untracked(|lines| {
        lines
            .iter()
            .filter(|line| traces.contains(&line.source))
            .map(|line| line.id)
            .collect::<Vec<_>>()
    }) {
        let _ = crate::trace_color::change(target, Some(id), |color| {
            if !source.is_empty() {
                color.mode = crate::trace_color::Mode::Field;
            } else if color.mode == crate::trace_color::Mode::Field {
                color.mode = crate::trace_color::Mode::Solid;
            }
        });
    }
}
pub(crate) fn set_provider(workbench: Workbench, pane: Pane, provider: Provider) {
    let edit = |spec: &mut Spec| {
        if let Some(map) = spec.map.as_mut() {
            map.provider = provider;
            map.basemap = true;
        }
    };
    update(workbench, pane, edit);
}
#[component]
pub fn MapBackend(workbench: Workbench, pane: Pane) -> impl IntoView {
    install_stylesheet("enlumi-map", include_str!("preview.css"));
    let provider = Memo::new(move |_| {
        target(workbench, pane)
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map).unwrap_or_default().provider)
    });
    let actual = target(workbench, pane);
    let shown = Memo::new(move |_| {
        actual
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map).is_none_or(|map| map.basemap))
    });
    view! {column(class="map-backend-settings") {
        text(class="map-layers-heading") {"Basemap"}
        RadioGroup(label="Basemap",value=Binding::controlled(Signal::derive_local(move ||if shown.get(){provider.get().key().to_owned()}else{"none".into()}),move |key:String|{
            update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.basemap=key!="none";if let Some(provider)=Provider::parse(&key){map.provider=provider;}});
        })) {
            for choice in move ||[("osm","OpenStreetMap"),("esri-imagery","Esri World Imagery"),("none","None (grid)")],key=|(key,_)|*key {
                {{let control=NodeRef::new();view! {row(class="map-layer-option") {RadioGroupItem(value=choice.0,node_ref=control,label=choice.1) Label(control=control,on:click=move |_|update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.basemap=choice.0!="none";if let Some(provider)=Provider::parse(choice.0){map.provider=provider;}})){{choice.1}}}}}}
            }
        }
        Select(value=Binding::controlled(Signal::derive_local(move ||provider.get().key().to_owned()),move |key:String|if let Some(provider)=Provider::parse(&key){set_provider(workbench,pane,provider);})) {
            SelectTrigger(class="map-backend-trigger",label="More basemaps"){"More basemaps…"}
            SelectContent {for provider in move ||Provider::ALL,key=|p|p.key(){SelectItem(value=provider.key()){{provider.label()}}}}
        }
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
            if slots.get(&index) == Some(&node) {
                slots.remove(&index);
            }
        })
    });
    let altitude = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().and_then(|s| s.altitudes.get(&index).cloned()))
            .unwrap_or_default()
    });
    let update_altitude = move |edit: fn(&mut crate::trajectory::Altitude)| {
        update(workbench, pane, move |s| {
            edit(s.altitudes.entry(index).or_default());
            if let Some(map) = s.map.as_mut()
                && map.terrain
            {
                map.camera = None;
            }
        });
    };
    let reference = Memo::new(move |_| {
        let a = altitude.get();
        (a.local, a.at_first)
    });
    let unit = Memo::new(move |_| altitude.get().scale.abs());
    view! {if move ||force||pane.trajectory.with(|s|s.as_ref().is_some_and(|s|s.map.is_some_and(|m|m.terrain||m.coloring==super::telemetry::ColorBy::Altitude))) {
        column(class="map-altitude-options trace-card-group") {
            row(class="trace-card-group-title") {text {"Altitude"}}
            row(class="trace-map-slot") {text(class="trace-map-axis") {"Z"}
            column(class="trajectory-axis-slot altitude-slot",class:axis-slot-filled=move ||!altitude.get().source.is_empty(),node_ref=node) {
                if move ||altitude.get().source.is_empty() {text(class="trajectory-drop-label") {"Drop Z field (optional)"}}
                if move ||!altitude.get().source.is_empty() {
                    row(class="trajectory-input-chip") {
                        text(class="signal-name") {{move ||workbench.field_name(&altitude.get().source)}}
                        ActionButton(icon=true,class="trajectory-remove-input",a11y:label="Remove Z input",on:click=move |_|bind_altitude(workbench,pane,index,String::new())) {
                            vector(class="trajectory-control-icon",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)
                        }
                    }
                }
            }
            }
            row(class="trace-card-segment",a11y:role=Role::Group,a11y:label="Altitude reference") {
                ActionButton(class="altitude-amsl",class:setting-active=move ||!reference.get().0,a11y:label="Z is absolute elevation above sea level",on:click=move |_|update_altitude(|a|{a.local=false;a.at_first=false;})) {"AMSL"}
                ActionButton(class="altitude-terrain",class:setting-active=move ||reference.get()==(true,false),a11y:label="Z is added to terrain height at the trace start",on:click=move |_|update_altitude(|a|{a.local=true;a.at_first=false;})) {"Above terrain"}
                ActionButton(class="altitude-ground",class:setting-active=move ||reference.get()==(true,true),a11y:label="The first Z sample sits on the ground",on:click=move |_|update_altitude(|a|{a.local=true;a.at_first=true;})) {"Start on ground"}
            }
            row(class="trace-card-segment") {
                ActionButton(class="altitude-metres",class:setting-active=move ||unit.get()==1.,on:click=move |_|update_altitude(|a|a.scale=a.scale.signum())) {"m"}
                ActionButton(class="altitude-millimetres",class:setting-active=move ||unit.get()==0.001,on:click=move |_|update_altitude(|a|a.scale=0.001*a.scale.signum())) {"mm"}
                ActionButton(class="altitude-feet",class:setting-active=move ||unit.get()==0.3048,on:click=move |_|update_altitude(|a|a.scale=0.3048*a.scale.signum())) {"ft"}
                box(class="trace-card-gap") {}
                ActionButton(class="altitude-up",class:setting-active=move ||altitude.get().scale>0.,on:click=move |_|update_altitude(|a|a.scale=a.scale.abs())) {"Up +"}
                ActionButton(class="altitude-down",class:setting-active=move ||altitude.get().scale<0.,on:click=move |_|update_altitude(|a|a.scale= -a.scale.abs())) {"Down + (NED)"}
            }
        }
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_settings_choose_independent_2d_and_3d_map_backends() {
        use crate::theme::AppThemeProps;
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
                        if move ||first_open.get() {MapBackend(workbench=workbench,pane=first)}
                        if move ||second_open.get() {MapBackend(workbench=workbench,pane=second)}
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
        click(&mut harness, "map-backend-trigger", "More basemaps");
        click(&mut harness, "zui-select__item", "Google Satellite");
        assert_eq!(provider(first), Provider::GoogleSatellite);
        assert_eq!(provider(second), Provider::EsriImagery);
        assert_eq!(
            provider(workbench.trajectory_builder().pane),
            Provider::GoogleSatellite
        );
        first_open.set(false);
        harness.settle(128);
        harness.advance(std::time::Duration::from_secs(1));
        second_open.set(true);
        harness.settle(128);
        click(&mut harness, "map-backend-trigger", "More basemaps");
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
                .any(|node| window.dom().text_content(node).contains("More basemaps"))
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
