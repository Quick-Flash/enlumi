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
#[allow(clippy::too_many_arguments)] // Independent component props.
pub fn TerrainView(
    camera: RwSignal<Camera, LocalStorage>,
    orbit: RwSignal<Orbit, LocalStorage>,
    service: Service,
    #[prop(optional)] provider: Option<RwSignal<Provider, LocalStorage>>,
    #[prop(optional)] offline: Option<RwSignal<bool, LocalStorage>>,
    #[prop(optional)] paths: Option<Signal<Arc<[super::flight3::Path3]>, LocalStorage>>,
    #[prop(optional)] colors: Option<Signal<Arc<super::telemetry::Coloring>, LocalStorage>>,
    #[prop(optional)] ground_projection: Option<Signal<bool, LocalStorage>>,
    #[prop(optional)] time: Option<Signal<Option<f64>, LocalStorage>>,
    #[prop(optional)] on_pick: Option<super::flight3::Pick>,
    #[prop(optional)] on_hover: Option<super::hover::Changed>,
    /// True while a pointer drag moves the view.
    #[prop(optional)]
    dragging: Option<RwSignal<bool, LocalStorage>>,
    #[prop(default = 2)] demand_owner: u64,
    #[prop(default = true)] show_help: bool,
) -> impl IntoView {
    let provider = provider.unwrap_or_else(|| RwSignal::new_local(Provider::OpenStreetMap));
    let offline = offline.unwrap_or_else(|| RwSignal::new_local(false));
    let paths = paths.unwrap_or_else(|| Signal::stored_local(Arc::from([])));
    let time = time.unwrap_or_else(|| Signal::stored_local(None));
    let colors = colors
        .unwrap_or_else(|| Signal::stored_local(Arc::new(super::telemetry::Coloring::default())));
    let ground_projection = ground_projection.unwrap_or_else(|| Signal::stored_local(false));
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
        let (orbit, camera, size, provider) = (
            orbit.get(),
            terrain_camera.get(),
            size.get(),
            provider.get(),
        );
        let _profile = crate::frame_profile::section("map.select");
        let selected =
            selector.with_value(|selector| selector.borrow_mut().select(orbit, camera, size));
        provider.expand_tiles(selected)
    });
    let raster_coverage = Memo::new(move |_| {
        let camera = terrain_camera.get();
        let orbit = orbit.get();
        let size = size.get();
        let (leaves, provider) = (coverage.get(), provider.get());
        let _profile = crate::frame_profile::section("map.raster");
        let target_zoom = (camera.zoom.ceil() as u8)
            .saturating_add(1)
            .min(super::tiles::MAX_TILE_ZOOM);
        let mut tiles = std::collections::BTreeSet::new();
        for leaf in leaves {
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
        let mut tiles = provider.expand_tiles(tiles.into_iter().collect());
        orbit.screen_first(camera, size, &mut tiles);
        tiles.truncate(512);
        tiles
    });
    // Requests only steer downloads, not what is drawn: a drag need not
    // recompute them every frame, but the final view is always requested.
    let demand_throttle = super::throttle::Throttle::new(std::time::Duration::from_millis(50));
    let request = move || {
        let _profile = crate::frame_profile::section("map.demand");
        let leaves = coverage.get_untracked();
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
        rasters.extend(raster_coverage.get_untracked());
        let mut rasters = provider.get_untracked().expand_tiles(rasters);
        // Coarse fallbacks load first, then the detail largest on screen.
        let (camera, view) = (terrain_camera.get_untracked(), orbit.get_untracked());
        view.screen_first(camera, size.get_untracked(), &mut rasters);
        view.screen_first(camera, size.get_untracked(), &mut elevations);
        rasters.truncate(768);
        let mut anchors = vec![super::flight3::anchor_tile(
            terrain_camera.get_untracked().center,
        )];
        anchors.extend(
            paths
                .get_untracked()
                .iter()
                .map(|path| super::flight3::anchor_tile(path.origin)),
        );
        anchors.extend(elevations);
        elevations = anchors;
        service.demand_layers(
            demand_owner,
            &rasters,
            &elevations,
            provider.get_untracked(),
            !offline.get_untracked(),
        );
    };
    let demand = RenderEffect::new(move |_| {
        coverage.track();
        raster_coverage.track();
        provider.track();
        offline.track();
        paths.track();
        demand_throttle.run(request);
    });
    // Resolve lazily with current coverage before submitting its matching camera.
    // Separate effects can briefly pair a new orbit with the previous view's tiles.
    let scene = Memo::new_owning(move |previous: Option<Arc<Scene>>| {
        service.revision.get();
        let (ids, raster_ids, paths) = (coverage.get(), raster_coverage.get(), paths.get());
        let _profile = crate::frame_profile::section("map.scene");
        let mut frame = Scene::resolve(
            service,
            &ids,
            &raster_ids,
            previous.as_deref().unwrap_or(&Scene::default()),
            terrain_camera.get(),
            orbit.get(),
            size.get(),
            provider.get(),
        )
        .with_paths(service, paths);
        frame.colors = colors.get();
        frame.ground_projection = ground_projection.get();
        (Arc::new(frame), true)
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
        let (camera, orbit) = (terrain_camera.get(), orbit.get());
        let _profile = crate::frame_profile::section("map.render");
        producer.terrain(camera, orbit, frame)
    });
    on_cleanup_local(move || {
        drop(demand);
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
            let (scene, camera, orbit, size) =
                (scene.get(), terrain_camera.get(), orbit.get(), size.get());
            let _profile = crate::frame_profile::section("map.hover");
            super::hover::Spatial::new(&scene, camera, orbit, size)
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
                    if let Some(dragging)=dragging {dragging.set(true);}
                    event.capture_pointer();event.prevent_default();
                },
                on:pointer_move=move |event| {
                    if drag.get_value().is_none() && let Some(changed)=on_hover {
                        let hit=plot::zgui_renderer::local_position(&host,event.position).and_then(|(x,y)|hover_index.with_untracked(|index|index.nearest([x,y])));changed.run(hit);
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
                        drag.set_value(None);if let Some(dragging)=dragging {dragging.set(false);}event.release_pointer();event.prevent_default();event.stop_propagation();
                    }
                },
                on:pointer_cancel=move |event|{if let Some(initial)=drag.get_value(){orbit.set(initial.orbit);camera.set(initial.camera);}drag.set_value(None);if let Some(dragging)=dragging {dragging.set(false);}event.release_pointer();},
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
