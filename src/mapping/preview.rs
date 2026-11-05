//! Offline integration gate; deliberately separate from saved flight workspaces.
use super::{
    camera::{Camera, geographic, project},
    gpu::Producer,
    terrain::Orbit,
    terrain_view::TerrainViewProps,
    tiles::{Config, Provider, Service},
};
use crate::{action_button::ActionButtonProps, theme::AppThemeProps};
use plot::{
    interaction::{Axes, Gesture, HitZones, RangeUpdate},
    workspace::{layout::Layout, plot_visible},
    zgui_pointer::PlotPointer,
    zgui_surface::{PlotSurfaceProps, SurfaceInteraction, SurfaceOptions},
    zgui_workspace::*,
};
use zgui::{
    prelude::*,
    reactive::{RenderEffect, UnsyncCallback},
};
use zgui_ui::prelude::*;

pub fn stylesheet() -> String {
    format!(
        "{}\n{}\n{}\n{}",
        plot::zgui_surface::stylesheet(),
        plot::zgui_workspace::stylesheet(),
        crate::theme::stylesheet(),
        include_str!("preview.css")
    )
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    app()
        .with_title("Enlumi - map and 3D terrain demo")
        .with_size(1200., 760.)
        .with_min_size(640., 400.)
        .with_icon(crate::app_icon::window())
        .with_stylesheet(stylesheet())
        .run_on(
            crate::startup::desktop_driver(),
            || view! { Preview(online=true,config=Some(Config::default())) },
        )?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct Model {
    pub camera: RwSignal<Camera, LocalStorage>,
    pub layout: RwSignal<Layout, LocalStorage>,
    pub maximized: RwSignal<Option<u64>, LocalStorage>,
    pub maps: RwSignal<bool, LocalStorage>,
    pub orbit: RwSignal<Orbit, LocalStorage>,
    pub service: Service,
    pub provider: RwSignal<Provider, LocalStorage>,
}

impl Model {
    fn location(self, lon: f64, lat: f64) {
        self.camera.set(Camera {
            center: project(lon, lat).expect("Demo location"),
            zoom: 12.,
        });
        self.maps.set(true);
        self.orbit.set(Orbit::default());
    }
    fn restore(self) {
        self.maximized.set(None);
        self.layout.set(Layout::flat(&[(1, 1.), (2, 1.)], false));
    }
}

#[component]
pub fn Preview(
    #[prop(default = false)] online: bool,
    #[prop(optional)] config: Option<Config>,
) -> impl IntoView {
    let model = Model {
        camera: RwSignal::new_local(if online {
            Camera {
                center: project(7.6586, 45.9763).unwrap(),
                zoom: 12.,
            }
        } else {
            Camera::default()
        }),
        layout: RwSignal::new_local(Layout::flat(&[(1, 1.), (2, 1.)], false)),
        maximized: RwSignal::new_local(None),
        maps: RwSignal::new_local(online),
        orbit: RwSignal::new_local(Orbit::default()),
        service: Service::new(config.unwrap_or_default()),
        provider: RwSignal::new_local(Provider::OpenStreetMap),
    };
    #[cfg(test)]
    super::tests::record_model(model);
    let panes = [DockPane::new(1), DockPane::new(2)];
    let active = RwSignal::new_local(1);
    let dock = DockState::new(model.layout, Signal::derive_local(move || panes.to_vec()));
    let geometry = WorkspaceGeometry::fitted(model.layout, model.maximized);
    let drag = RwSignal::new_local(None);
    view! {
        AppTheme(palette=Signal::stored_local(crate::theme::Palette::default())) {
            column(class="page workbench map-preview") {
                row(class="map-preview-toolbar") {
                    text {"Map + 3D terrain demo"}
                    ActionButton(class="map-reset",on:click=move |_|if model.maps.get_untracked(){model.location(7.6586,45.9763);}else{model.camera.set(Camera::default());}) {"Reset view"}
                    ActionButton(class="map-toggle",on:click=move |_|if model.maps.get_untracked(){model.maps.set(false);model.camera.set(Camera::default());}else{model.location(7.6586,45.9763);}) {{move ||if model.maps.get(){"Checkerboard"}else{"Show maps"}}}
                    ActionButton(on:click=move |_|model.location(7.6586,45.9763)) {"Matterhorn"}
                    ActionButton(on:click=move |_|model.location(-105.645,40.276)) {"Rockies"}
                    ActionButton(on:click=move |_|model.service.retry()) {"Retry tiles"}
                    Select(value=Binding::controlled(
                        Signal::derive_local(move ||model.provider.get().key().to_owned()),
                        move |key:String|if let Some(provider)=Provider::parse(&key){model.provider.set(provider);}
                    )) {
                        SelectTrigger(label="Map provider") {SelectValue()}
                        SelectContent {for source in move || Provider::ALL,key=|source|source.key() {SelectItem(value=source.key()) {{source.label()}}}}
                    }
                    ActionButton(class="map-restore",on:click=move |_|model.restore()) {"Restore panes"}
                }
                text(class="muted") {{move ||if model.maps.get(){"2D: drag to pan; Shift-drag to zoom. 3D: drag to orbit; middle-drag or Ctrl-drag to pan. Wheel zooms both views."}else{"Synthetic tiles only. Drag to pan; wheel to zoom; Shift-drag to box zoom; double-click to reset."}}}
                text(class="map-status") {{move || { let camera=model.camera.get(); let [lon,lat]=geographic(camera.center); format!("Center {lat:.5}, {lon:.5} | zoom {:.2}",camera.zoom) }}}
                if move ||model.maps.get() {
                    row(class="map-preview-toolbar") {
                        text(class="map-load-status") {{move ||model.service.status()}}
                        ActionButton(on:click=move |_|model.orbit.set(Orbit::default())) {"Reset orbit"}
                        ActionButton(class="map-range",on:click=move |_|model.orbit.update(|orbit|orbit.view_distance_km=if orbit.view_distance_km<80.{80.}else if orbit.view_distance_km<160.{160.}else{40.})) {{move ||format!("View distance: {:.0} km",model.orbit.get().visibility_metres()/1000.)}}
                        text(class="map-pitch") {{move ||format!("Angle: {:.0}°",model.orbit.get().pitch)}}
                        ActionButton(class="map-exaggeration",on:click=move |_|model.orbit.update(|orbit|orbit.exaggeration=if orbit.exaggeration==1.{2.}else{1.})) {{move ||format!("Height: {}x",model.orbit.get().exaggeration)}}
                    }
                }
                column(class="map-preview-workspace") {
                    WorkspaceCanvas(geometry=geometry,drag=drag) {
                        WorkspaceDockOverlay(dock=dock,geometry=Some(geometry))
                        for pane in move ||panes.to_vec(),key=|pane|pane.id {
                            WorkspacePane(pane=pane,dock=dock,geometry=geometry,active=active,maximized=model.maximized,
                                on_select=UnsyncCallback::new(move |id|active.set(id))) {
                                row(class="plot-strip") {
                                    PlotTabs(pane=pane,dock=dock,title=UnsyncCallback::new(move |id|if model.maps.get_untracked(){if id==1{"2D map".into()}else{"3D terrain".into()}}else if id==1 {"Image tiles".into()}else{"GPU surface".into()}),
                                        on_select=UnsyncCallback::new(move |id|active.set(id)),
                                        on_close=UnsyncCallback::new(move |id| {model.layout.update(|layout|layout.remove(id));model.maximized.set(None);}),
                                        on_add=UnsyncCallback::new(move |_|model.restore()))
                                    ActionButton(class="map-maximize",on:click=move |_|model.maximized.update(|id|*id=if *id==Some(pane.id){None}else{Some(pane.id)})) {"Maximize / restore"}
                                }
                                if move ||plot_visible(pane.id,model.maximized.get(),model.layout.get()) {
                                    if move ||model.maps.get() &&pane.id==2 {
                                        TerrainView(camera=model.camera,orbit=model.orbit,service=model.service,provider=Some(model.provider))
                                    }else{MapView(camera=model.camera,gpu=pane.id==2,maps=model.maps,service=model.service,provider=if pane.id==1 {Some(model.provider)}else{None})}
                                }
                            }
                        }
                    }
                }
                if move ||model.maps.get() {
                    row(class="map-attribution") {
                        ActionButton(on:click=move |_|open_attribution(model.provider.get().attribution_url())) {{move ||model.provider.get().attribution()}}
                        ActionButton(on:click=move |_|open_attribution("https://mapterhorn.com/attribution/")) {"Terrain: © Mapterhorn / source datasets"}
                    }
                }
            }
        }
    }
}

#[component]
#[allow(clippy::too_many_arguments)] // Independent component props.
pub(crate) fn MapView(
    camera: RwSignal<Camera, LocalStorage>,
    gpu: bool,
    maps: RwSignal<bool, LocalStorage>,
    service: Service,
    #[prop(optional)] provider: Option<RwSignal<Provider, LocalStorage>>,
    #[prop(optional)] offline: Option<RwSignal<bool, LocalStorage>>,
    #[prop(default = 1)] demand_owner: u64,
    #[prop(optional)] on_reset: Option<UnsyncCallback<(), ()>>,
    #[prop(default = false)] transparent_background: bool,
) -> impl IntoView {
    let provider = provider.unwrap_or_else(|| RwSignal::new_local(Provider::OpenStreetMap));
    let offline = offline.unwrap_or_else(|| RwSignal::new_local(false));
    if maps.get_untracked() && !gpu {
        camera.update(|camera| {
            camera.zoom = if camera.zoom.is_finite() {
                camera.zoom.clamp(0., super::camera::MAX_INTERACTIVE_ZOOM)
            } else {
                12.
            };
        });
    }
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let size = Memo::new(move |_| {
        bounds.get().map_or([0., 0.], |bounds| {
            [
                f64::from(bounds.size.width.0 / host.scale()),
                f64::from(bounds.size.height.0 / host.scale()),
            ]
        })
    });
    let pointer = PlotPointer::new(
        HitZones {
            x_edge: 0.,
            bottom_band: (-1., -1.),
            y_axis_width: 0.,
            y_min_handle: false,
        },
        Axes::BOTH,
    );
    let aspect = RenderEffect::new(move |_| {
        let [w, h] = size.get();
        pointer.set_selection_aspect((h > 0.).then_some(w / h));
    });
    on_cleanup_local(move || drop(aspect));
    let owner = if gpu { 2 } else { demand_owner };
    let demand = RenderEffect::new(move |_| {
        let camera = camera.get();
        let viewport = size.get();
        let selected = provider.get();
        let online = !offline.get();
        let visible_tiles = if maps.get() && !gpu {
            selected.expand_tiles(camera.map_tiles(viewport))
        } else {
            Vec::new()
        };
        let mut tiles = if visible_tiles.is_empty() {
            Vec::new()
        } else {
            let mut requested = selected.expand_tiles(camera.map_fallback_tiles(viewport));
            requested.extend(visible_tiles.iter().copied());
            requested
        };
        // Warm adjacent zoom levels at lower priority to reuse decoded tiles.
        if let Some(level) = visible_tiles.first().map(|tile| tile.z) {
            let mut levels = Vec::new();
            if level > 0 {
                levels.push(level - 1);
            }
            if level < 19 {
                levels.push(level + 1);
            }
            for adjacent in levels {
                let mut neighbor = camera;
                neighbor.zoom = f64::from(adjacent);
                tiles.extend(selected.expand_tiles(neighbor.map_tiles(viewport)));
            }
            let mut unique = std::collections::BTreeSet::new();
            tiles.retain(|tile| unique.insert(*tile));
        }
        service.demand(owner, &tiles, false, selected, online)
    });
    on_cleanup_local(move || {
        drop(demand);
        service.demand(
            owner,
            &[],
            false,
            provider.get_untracked(),
            !offline.get_untracked(),
        );
    });
    let apply = move |ranges, pan| {
        let next = if pan {
            camera.get_untracked().pan_to(ranges, size.get_untracked())
        } else {
            Camera::from_ranges(ranges, size.get_untracked())
        };
        if let Some(mut next) = next {
            if maps.get_untracked() {
                next.zoom = next.zoom.min(super::camera::MAX_INTERACTIVE_ZOOM);
            }
            camera.set(next);
        }
    };
    let interaction = SurfaceInteraction {
        geometry: UnsyncCallback::new(move |()| Camera::geometry(size.get_untracked())),
        ranges: UnsyncCallback::new(move |()| camera.get_untracked().ranges(size.get_untracked())),
        on_update: UnsyncCallback::new(move |update: RangeUpdate| {
            // PlotSurface coalesces input in frame callbacks. Read its current
            // gesture as an imperative snapshot, without a signal subscription.
            let _snapshot = zgui::reactive::enter_non_reactive_zone();
            apply(
                update.apply(camera.get_untracked().ranges(size.get_untracked())),
                pointer.active() == Some(Gesture::Pan),
            )
        }),
        on_reset: UnsyncCallback::new(move |()| {
            if let Some(reset) = on_reset {
                reset.run(());
                return;
            }
            camera.set(if maps.get_untracked() {
                Camera {
                    center: project(7.6586, 45.9763).unwrap(),
                    zoom: 12.,
                }
            } else {
                Camera::default()
            })
        }),
        on_hover: UnsyncCallback::new(|_| {}),
    };
    let options = SurfaceOptions {
        show_pointer: false,
        show_cursor_values: false,
        background: if transparent_background {
            "transparent".into()
        } else {
            "#12181e".into()
        },
        on_cancel: Some(UnsyncCallback::new(move |ranges| apply(ranges, true))),
        ..Default::default()
    };
    let content = if gpu {
        let producer = Producer::new();
        #[cfg(test)]
        super::tests::record_producer(producer.stats.clone());
        let surface = zgui::elements::surface()
            .class("map-gpu")
            .source(&producer.handle)
            .into_view();
        let effect = RenderEffect::new(move |_| producer.camera(camera.get()));
        on_cleanup_local(move || drop(effect));
        AnyView::new(surface)
    } else {
        let bytes = zgui_image::ImageBytes::new(
            include_bytes!("../../assets/map-checkerboard.png").to_vec(),
        );
        let url = StoredValue::new_local(bytes.url());
        on_cleanup_local(move || drop(bytes));
        AnyView::new(view! {
            box(class="map-tiles") {
                for tile in move ||if maps.get(){provider.get().expand_tiles(camera.get().map_fallback_tiles(size.get()))}else{Vec::new()},key=|tile|*tile {
                    box(class="map-tile",style:transform=move ||{
                        let [x,y,scale]=camera.get().place(tile,size.get());
                        let dpi=f64::from(host.scale());
                        let snap=|v:f64|(v*dpi).round()/dpi;
                        let [left,top,right,bottom]=[x,y,x+256.*scale,y+256.*scale].map(snap);
                        Some(format!("translate({left}px, {top}px) scale({}, {})",(right-left)/256.,(bottom-top)/256.))
                    }) {
                        {zgui::elements::image().class("map-tile-image").src(Signal::derive_local(move ||service.url(tile,provider.get()))).alt(Some("Coarse fallback map tile".to_string())).into_view()}
                    }
                }
                for tile in move ||if maps.get(){provider.get().expand_tiles(camera.get().map_tiles(size.get()))}else{camera.get().tiles(size.get())},key=|tile|*tile {
                    box(class="map-tile",style:transform=move ||{
                        let [x,y,scale]=camera.get().place(tile,size.get());
                        // Adjacent tiles share rounded device-pixel edges. In
                        // particular, a tile clipped at the viewport must end
                        // exactly where its visible neighbour begins.
                        let dpi=f64::from(host.scale());
                        let snap=|v:f64|(v*dpi).round()/dpi;
                        let [left,top,right,bottom]=[x,y,x+256.*scale,y+256.*scale].map(snap);
                        Some(format!("translate({left}px, {top}px) scale({}, {})",(right-left)/256.,(bottom-top)/256.))
                    }) {
                        {zgui::elements::image().class("map-tile-image").src(Signal::derive_local(move ||if maps.get(){service.url(tile,provider.get())}else{Some(url.get_value())})).alt(Some("Map tile".to_string())).into_view()}
                        if move ||!maps.get() {text(class="map-tile-label") {{format!("{}/{}/{}",tile.z,tile.canonical_x(),tile.y)}}}
                    }
                }
            }
        })
    };
    let distance_scale = super::scale::scale_bar(
        move || {
            let camera = camera.get();
            Some(super::terrain::ground_scale(camera) / camera.scale())
        },
        false,
    );
    view! {PlotSurface(node_ref=host,class="map-viewport",label=if gpu {"GPU map preview".into()}else{"Image tile map preview".into()},pointer=pointer,interaction=interaction,options=options) { {content} {distance_scale} }}
}

pub(crate) fn open_attribution(url: &'static str) {
    zgui::reactive::spawn_local(async move {
        if let Err(error) = blocking(move || crate::updates::open_url(url)).await {
            eprintln!("{error}");
        }
    });
}
