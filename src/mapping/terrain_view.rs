use super::{
    camera::{Camera, MAX_INTERACTIVE_ZOOM},
    gpu::Producer,
    terrain::{Orbit, Scene, Selector},
    tiles::{Provider, Service},
};
use std::sync::Arc;
use zgui::{prelude::*, reactive::RenderEffect};

#[derive(Clone, Copy)]
struct Drag {
    start: [f64; 2],
    orbit: Orbit,
    camera: Camera,
    pan: bool,
    button: zgui::vocab::PointerButton,
}

#[component]
pub fn TerrainView(
    camera: RwSignal<Camera, LocalStorage>,
    orbit: RwSignal<Orbit, LocalStorage>,
    service: Service,
    #[prop(optional)] provider: Option<RwSignal<Provider, LocalStorage>>,
    #[prop(optional)] offline: Option<RwSignal<bool, LocalStorage>>,
    #[prop(optional)] paths: Option<Signal<Arc<[super::flight3::Path3]>, LocalStorage>>,
    #[prop(optional)] time: Option<Signal<Option<f64>, LocalStorage>>,
    #[prop(optional)] on_pick: Option<super::flight3::Pick>,
    #[prop(optional)] on_hover: Option<super::hover::Changed>,
    #[prop(default = 2)] demand_owner: u64,
    #[prop(default = true)] show_help: bool,
) -> impl IntoView {
    let provider = provider.unwrap_or_else(|| RwSignal::new_local(Provider::OpenStreetMap));
    let offline = offline.unwrap_or_else(|| RwSignal::new_local(false));
    let paths = paths.unwrap_or_else(|| Signal::stored_local(Arc::from([])));
    let time = time.unwrap_or_else(|| Signal::stored_local(None));
    let terrain_camera = Memo::new(move |_| {
        let mut camera = camera.get();
        if !camera.center.iter().all(|value| value.is_finite()) {
            camera.center = Camera::default().center;
        }
        camera.zoom = if camera.zoom.is_finite() {
            camera.zoom.clamp(0., MAX_INTERACTIVE_ZOOM)
        } else {
            12.
        };
        camera
    });
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let size = Memo::new(move |_| {
        bounds.get().map_or([0., 0.], |b| {
            [
                f64::from(b.size.width.0 / host.scale()),
                f64::from(b.size.height.0 / host.scale()),
            ]
        })
    });
    let producer = Producer::new();
    super::export::register_surface(producer.capture());
    #[cfg(test)]
    super::tests::record_producer(producer.stats.clone());
    let surface = zgui::elements::surface()
        .class("map-gpu map-terrain-surface")
        .source(&producer.handle)
        .into_view();
    let selector = StoredValue::new_local(std::cell::RefCell::new(Selector::default()));
    let coverage = Memo::new(move |_| {
        let selected = selector.with_value(|selector| {
            selector
                .borrow_mut()
                .select(orbit.get(), terrain_camera.get(), size.get())
        });
        provider.get().expand_tiles(selected)
    });
    let raster_coverage = Memo::new(move |_| {
        let camera = terrain_camera.get();
        let orbit = orbit.get();
        let size = size.get();
        let target_zoom = (camera.zoom.ceil() as u8).saturating_add(1).min(19);
        let mut tiles = std::collections::BTreeSet::new();
        for leaf in coverage.get() {
            let pixels = orbit.projected_tile_size(camera, size, leaf);
            let extra = if pixels.is_finite() {
                ((pixels.max(256.) / 256.).log2().ceil() as u8).min(3)
            } else {
                0
            };
            let zoom = leaf.z.saturating_add(extra).min(target_zoom.max(leaf.z));
            let side = 1_i64 << (zoom - leaf.z);
            for y in 0..side {
                for x in 0..side {
                    tiles.insert(super::camera::TileId {
                        z: zoom,
                        x: leaf.x * side + x,
                        y: leaf.y * side as u32 + y as u32,
                    });
                }
            }
        }
        let mut tiles = provider.get().expand_tiles(tiles.into_iter().collect());
        camera.center_first(&mut tiles);
        tiles.truncate(512);
        tiles
    });
    let demand = RenderEffect::new(move |_| {
        let leaves = coverage.get();
        let mut requested = std::collections::BTreeSet::new();
        for &leaf in &leaves {
            // Load coarse rasters and DEM ancestors for immediate fallback.
            let first_zoom = leaf.z.saturating_sub(3);
            for z in first_zoom..=leaf.z {
                let shift = leaf.z - z;
                requested.insert(super::camera::TileId {
                    z,
                    x: leaf.x.div_euclid(1_i64 << shift),
                    y: leaf.y >> shift,
                });
            }
        }
        let mut elevations = requested.iter().copied().collect::<Vec<_>>();
        let mut rasters = requested.into_iter().collect::<Vec<_>>();
        rasters.extend(raster_coverage.get());
        let mut rasters = provider.get().expand_tiles(rasters);
        // Favor tiles closest to the focus point at every zoom. This keeps
        // detail beneath the camera ahead of lower-resolution distant terrain.
        terrain_camera.get_untracked().center_first(&mut rasters);
        terrain_camera.get_untracked().center_first(&mut elevations);
        rasters.truncate(768);
        let mut anchors = vec![super::flight3::anchor_tile(
            terrain_camera.get_untracked().center,
        )];
        anchors.extend(
            paths
                .get()
                .iter()
                .map(|path| super::flight3::anchor_tile(path.origin)),
        );
        anchors.extend(elevations);
        elevations = anchors;
        service.demand_layers(
            demand_owner,
            &rasters,
            &elevations,
            provider.get(),
            !offline.get(),
        );
    });
    let scene = RwSignal::new_local(Arc::new(Scene::default()));
    let data = RenderEffect::new(move |_| {
        service.revision.get();
        scene.set(Arc::new(
            Scene::resolve(
                service,
                &coverage.get(),
                &raster_coverage.get(),
                &scene.get_untracked(),
                terrain_camera.get(),
                provider.get(),
            )
            .with_paths(service, paths.get()),
        ));
    });
    let render = RenderEffect::new(move |_| {
        let frame = scene.get();
        let current = time.get();
        let frame = if frame.time == current {
            frame
        } else {
            let mut frame = (*frame).clone();
            frame.time = current;
            Arc::new(frame)
        };
        producer.terrain(terrain_camera.get(), orbit.get(), frame)
    });
    on_cleanup_local(move || {
        drop(demand);
        drop(data);
        drop(render);
        service.demand(
            demand_owner,
            &[],
            false,
            provider.get_untracked(),
            !offline.get_untracked(),
        );
    });
    let drag = StoredValue::new_local(None::<Drag>);
    let distance_scale = super::scale::scale_bar(
        move || {
            let [w, h] = size.get();
            let camera = terrain_camera.get();
            let orbit = orbit.get();
            let a = orbit.ground_at(camera, [w, h], [w / 2. - 0.5, h / 2.])?;
            let b = orbit.ground_at(camera, [w, h], [w / 2. + 0.5, h / 2.])?;
            Some((b[0] - a[0]).hypot(b[1] - a[1]) * super::terrain::ground_scale(camera))
        },
        true,
    );
    let hover_index = Memo::new(move |_| {
        if on_hover.is_some() {
            super::hover::Spatial::new(&scene.get(), terrain_camera.get(), orbit.get(), size.get())
        } else {
            super::hover::Spatial::default()
        }
    });
    view! {
        box(class="map-viewport map-terrain",node_ref=host) {
            {surface}
            box(class="map-terrain-input",a11y:label="3D terrain: drag to orbit, middle-drag or Ctrl-drag to pan, wheel to zoom",tabindex=Focus::Sequential,
                on:pointer_down=move |event| {
                    let Some(button @ (zgui::vocab::PointerButton::Primary | zgui::vocab::PointerButton::Middle))=event.button else{return;};
                    if drag.get_value().is_some(){return;}
                    let Some((x,y))=plot::zgui_renderer::local_position(&host,event.position) else{return;};
                    if let Some(changed)=on_hover {changed.run(None);}
                    drag.set_value(Some(Drag{start:[x,y],orbit:orbit.get_untracked(),camera:terrain_camera.get_untracked(),pan:button==zgui::vocab::PointerButton::Middle||event.modifiers.control(),button}));
                    event.capture_pointer();event.prevent_default();
                },
                on:pointer_move=move |event| {
                    if drag.get_value().is_none() {
                        if let Some(changed)=on_hover {let hit=plot::zgui_renderer::local_position(&host,event.position).and_then(|(x,y)|hover_index.with_untracked(|index|index.nearest([x,y])));changed.run(hit);}
                    }
                    if let Some(initial)=drag.get_value()
                        &&let Some((x,y))=plot::zgui_renderer::local_position(&host,event.position) {
                        if initial.pan {
                            if let Some(next)=initial.orbit.pan(initial.camera,size.get_untracked(),initial.start,[x,y]){camera.set(next);}
                        }else{orbit.set(initial.orbit.dragged([x-initial.start[0],y-initial.start[1]]));}
                    }
                },
                on:pointer_leave=move |_|{if let Some(changed)=on_hover {changed.run(None);}},
                on:pointer_up=move |event|{
                    if let Some(initial)=drag.get_value() && event.button==Some(initial.button) {
                        if !initial.pan && let Some((x,y))=plot::zgui_renderer::local_position(&host,event.position) && (x-initial.start[0]).hypot(y-initial.start[1])<4. && let Some(pick)=on_pick {pick.run((scene.get_untracked(),terrain_camera.get_untracked(),orbit.get_untracked(),size.get_untracked(),[x,y]));}
                        drag.set_value(None);event.release_pointer();event.prevent_default();event.stop_propagation();
                    }
                },
                on:pointer_cancel=move |event|{if let Some(initial)=drag.get_value(){orbit.set(initial.orbit);camera.set(initial.camera);}drag.set_value(None);event.release_pointer();},
                on:wheel=move |event| {
                    let factor=plot::zgui_surface::wheel_factor(&event.delta);
                    let zoom_delta=factor.log2();
                    camera.update(|camera|{
                        let zoom=if camera.zoom.is_finite(){camera.zoom.clamp(0.,MAX_INTERACTIVE_ZOOM)}else{12.};
                        if zoom_delta.is_finite(){camera.zoom=(zoom-zoom_delta).clamp(0.,MAX_INTERACTIVE_ZOOM);}else{camera.zoom=zoom;}
                    });
                    event.prevent_default();event.stop_propagation();
                }) {}
            {distance_scale}
            if move ||show_help {text(class="map-terrain-help") {"Drag to orbit | Middle-drag / Ctrl-drag to pan | wheel to zoom"}}
        }
    }
}
