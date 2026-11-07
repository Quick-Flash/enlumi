//! Geographic flight paths in the normal dock/workspace lifecycle.
pub(crate) use super::camera::{WORLD, geographic, project};
use super::{
    camera::Camera,
    preview::MapViewProps,
    terrain_view::TerrainViewProps,
    tiles::{Provider, Service},
};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trajectory::{Spec, Tracks},
    trajectory_plot::{TrajectoryLegendProps, TrajectoryOverlayProps},
};
use plot::{PlotPoint, Ranges};
use serde::{Deserialize, Serialize};
use zgui::{prelude::*, reactive::RenderEffect};

/// A flat east/north frame, expressed in metres at the chosen map location.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalFrame {
    pub origin: Option<[f64; 2]>,
    pub at_first: bool,
}
impl Default for LocalFrame {
    fn default() -> Self {
        Self {
            origin: None,
            at_first: true,
        }
    }
}
impl LocalFrame {
    pub fn validate(self) -> Result<(), String> {
        if self.origin.is_some_and(|[lon, lat]| {
            !lon.is_finite()
                || !lat.is_finite()
                || !(-180. ..=180.).contains(&lon)
                || lat.abs() > super::camera::LIMIT
        }) {
            return Err(
                "Choose a valid starting longitude and latitude within map coverage".into(),
            );
        }
        Ok(())
    }
}

pub(crate) fn set_origin(pane: Pane, source: &str, origin: [f64; 2]) -> Result<(), String> {
    let index = pane
        .trajectory
        .get_untracked()
        .and_then(|s| s.pairs().iter().position(|p| p.y == source))
        .ok_or("Map trace is no longer available")?;
    set_origin_at(pane, index, origin)
}

pub(crate) fn set_origin_at(pane: Pane, index: usize, origin: [f64; 2]) -> Result<(), String> {
    LocalFrame {
        origin: Some(origin),
        at_first: true,
    }
    .validate()?;
    let mut spec = pane
        .trajectory
        .get_untracked()
        .ok_or("Map trace is no longer available")?;
    if index == 0 {
        spec.local
            .as_mut()
            .ok_or("Choose local metres for this trace")?
            .origin = Some(origin);
    } else {
        spec.extra
            .get_mut(index - 1)
            .and_then(|p| p.local.as_mut())
            .ok_or("Local trace is no longer available")?
            .origin = Some(origin);
    }
    spec.validate()?;
    pane.trajectory.set(Some(spec));
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_2d: Option<Camera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_3d: Option<Camera>,
    #[serde(default)]
    pub provider: Provider,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub terrain: bool,
    #[serde(default)]
    pub follow: bool,
    #[serde(default)]
    pub ground_projection: bool,
    #[serde(default)]
    pub coloring: super::telemetry::ColorBy,
    #[serde(default)]
    pub orbit: super::terrain::Orbit,
}
impl Config {
    pub fn remember_camera(&mut self, camera: Camera) {
        self.camera = Some(camera);
        if self.terrain {
            self.camera_3d = Some(camera);
        } else {
            self.camera_2d = Some(camera);
        }
    }
    pub fn set_terrain(&mut self, terrain: bool) {
        if self.terrain == terrain {
            return;
        }
        if self.terrain {
            self.camera_3d = self.camera;
        } else {
            self.camera_2d = self.camera;
        }
        self.terrain = terrain;
        self.camera = if terrain {
            self.camera_3d
        } else {
            self.camera_2d
        };
    }
    pub fn fit(&mut self) {
        self.camera = None;
        self.follow = false;
        if self.terrain {
            self.camera_3d = None;
        } else {
            self.camera_2d = None;
        }
    }
    pub fn validate(self) -> Result<(), String> {
        let o = self.orbit;
        if !o.bearing.is_finite()
            || !o.pitch.is_finite()
            || !(super::terrain::MIN_PITCH..=85.).contains(&o.pitch)
            || !o.exaggeration.is_finite()
            || !(0.1..=10.).contains(&o.exaggeration)
            || !o.view_distance_km.is_finite()
            || !(40. ..=160.).contains(&o.view_distance_km)
        {
            return Err("Invalid terrain orbit settings".into());
        }
        if [self.camera, self.camera_2d, self.camera_3d]
            .into_iter()
            .flatten()
            .any(|c| {
                !c.center.iter().all(|v| v.is_finite())
                    || c.center[1].abs() > WORLD / 2.
                    || !c.zoom.is_finite()
                    || !(0. ..=super::camera::MAX_ZOOM).contains(&c.zoom)
            })
        {
            return Err("Invalid map camera".into());
        }
        Ok(())
    }
}

impl Workbench {
    pub fn new_flight_map(self) {
        if self
            .pane()
            .trajectory
            .with_untracked(|s| s.as_ref().is_some_and(|s| s.map.is_some()))
        {
            self.edit_trajectory(self.pane());
            return;
        }
        let builder = self.trajectory_builder();
        if !builder
            .pane
            .trajectory
            .with_untracked(|s| s.as_ref().is_some_and(|s| s.map.is_some()))
        {
            builder.pane.trajectory.set(Some(Spec {
                map: Some(Config::default()),
                gps_scale: Some(1.),
                ..Default::default()
            }));
        }
        builder.pane.lines.set(self.pane().lines.get_untracked());
        builder.target.set(None);
        builder.open.set(true);
        self.analysis().open.set(false);
        self.filters_open.set(false);
    }
    pub(crate) fn shared_map_tiles(self) -> Service {
        if let Some(service) = self.map_tiles.get_value() {
            return service;
        }
        let mut config = super::TileConfig::default();
        if std::env::var_os("ENLUMI_MAP_CACHE").is_none()
            && let Some(path) = crate::preferences::cache_directory()
        {
            config.cache = path.join("maps");
        }
        let service = self
            .owner
            .with_value(|owner| owner.with(|| Service::new(config)));
        self.map_tiles.set_value(Some(service));
        service
    }
}

pub(crate) fn fit(
    data: &Tracks,
    size: [f64; 2],
    hidden: &std::collections::BTreeSet<String>,
) -> Option<Camera> {
    Camera::geometry(size)?;
    let ranges = plot::plot_data::PlotData::Scatter(data.scatter.clone()).fitted_ranges(hidden, 32);
    let mut camera = Camera::from_ranges(ranges, size)?;
    // Keep the same unwrapped world copy as a path crossing the dateline.
    camera.center[0] = (ranges.x.min + ranges.x.max) / 2.;
    camera.zoom = camera.zoom.min(super::camera::MAX_ZOOM);
    Some(camera)
}

pub(crate) fn pick_time(
    data: &Tracks,
    target: PlotPoint,
    ranges: Ranges,
    size: [f64; 2],
    hidden: &std::collections::BTreeSet<String>,
) -> Option<f64> {
    if size.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return None;
    }
    let sx = size[0] / ranges.x.span();
    let sy = size[1] / ranges.y.span();
    let mut best = (12_f64.powi(2), None);
    for trace in &data.traces {
        if hidden.contains(&trace.id.to_string()) {
            continue;
        }
        if trace.track.geographic.is_none() {
            continue;
        }
        if let Some(index) =
            trace.track.scatter.series()[0].nearest(target, ranges, (size[0], size[1]), 12.)
        {
            let p = trace.track.points[index];
            let distance = ((p.x - target.x) * sx).powi(2) + ((p.y - target.y) * sy).powi(2);
            if distance <= best.0 {
                best = (distance, Some(trace.track.times[index]));
            }
        }
        for (i, pair) in trace.track.points.windows(2).enumerate() {
            let a = pair[0];
            let b = pair[1];
            if ![a.x, a.y, b.x, b.y].iter().all(|v| v.is_finite()) {
                continue;
            }
            let a = [(a.x - target.x) * sx, (a.y - target.y) * sy];
            let b = [(b.x - target.x) * sx, (b.y - target.y) * sy];
            if a[0].min(b[0]) > 12.
                || a[0].max(b[0]) < -12.
                || a[1].min(b[1]) > 12.
                || a[1].max(b[1]) < -12.
            {
                continue;
            }
            let delta = [b[0] - a[0], b[1] - a[1]];
            let length = delta[0] * delta[0] + delta[1] * delta[1];
            let t = if length > 0. {
                (-(a[0] * delta[0] + a[1] * delta[1]) / length).clamp(0., 1.)
            } else {
                0.
            };
            let distance = (a[0] + t * delta[0]).powi(2) + (a[1] + t * delta[1]).powi(2);
            if distance < best.0 {
                best = (
                    distance,
                    Some(
                        trace.track.times[i]
                            + t * (trace.track.times[i + 1] - trace.track.times[i]),
                    ),
                );
            }
        }
    }
    best.1
}

#[component]
pub fn FlightMap(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    install_stylesheet("enlumi-map", include_str!("preview.css"));
    install_stylesheet("enlumi-trajectory", include_str!("../trajectory.css"));
    let service = workbench.shared_map_tiles();
    let camera = RwSignal::new_local(
        pane.trajectory
            .with_untracked(|s| s.as_ref()?.map?.camera)
            .unwrap_or_default(),
    );
    let provider = RwSignal::new_local(
        pane.trajectory
            .with_untracked(|s| s.as_ref().and_then(|s| s.map.as_ref().map(|m| m.provider)))
            .unwrap_or_default(),
    );
    let offline = RwSignal::new_local(
        pane.trajectory
            .with_untracked(|s| s.as_ref().and_then(|s| s.map.as_ref().map(|m| m.offline)))
            .unwrap_or_default(),
    );
    let maps = RwSignal::new_local(true);
    let terrain = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.map.is_some_and(|m| m.terrain)))
    });
    let orbit = RwSignal::new_local(
        pane.trajectory
            .with_untracked(|s| s.as_ref().and_then(|s| s.map.map(|m| m.orbit)))
            .unwrap_or_default(),
    );
    let configuration = Memo::new(move |_| {
        pane.trajectory
            .with(|s| {
                s.as_ref()
                    .and_then(|s| s.map.map(|m| (m.provider, m.offline, m.orbit)))
            })
            .unwrap_or_default()
    });
    let active_mode = StoredValue::new_local(terrain.get_untracked());
    let settings = zgui::reactive::RenderEffect::new(move |_| {
        let mode = terrain.get();
        if mode != active_mode.get_value() {
            active_mode.set_value(mode);
            camera.set(
                pane.trajectory
                    .with_untracked(|s| s.as_ref()?.map?.camera)
                    .unwrap_or_default(),
            );
        }
        let (p, o, r) = configuration.get();
        if provider.get_untracked() != p {
            provider.set(p);
        }
        if offline.get_untracked() != o {
            offline.set(o);
        }
        if orbit.get_untracked() != r {
            orbit.set(r);
        }
    });
    let save_orbit = zgui::reactive::RenderEffect::new(move |_| {
        let next = orbit.get();
        if pane.trajectory.with_untracked(|s| {
            s.as_ref()
                .is_some_and(|s| s.map.is_some_and(|m| m.orbit != next))
        }) {
            pane.trajectory.update(|s| {
                if let Some(map) = s.as_mut().and_then(|s| s.map.as_mut()) {
                    map.orbit = next;
                }
            });
        }
    });
    let (paths, paths_ready) = super::flight3::install(workbench, pane);
    let colors = super::telemetry::install(workbench, pane);
    let ground_projection = Memo::new(move |_| {
        pane.trajectory.with(|s| {
            s.as_ref()
                .and_then(|s| s.map)
                .is_some_and(|m| m.ground_projection)
        })
    });
    provide_local_context(super::telemetry::ColorContext(colors));
    on_cleanup_local(move || {
        drop(settings);
        drop(save_orbit);
    });
    let host = NodeRef::new();
    let legend = NodeRef::new();
    super::export::install(workbench, pane, host, camera, provider, service, colors);
    let bounds = host.observe_border_box();
    let size = Memo::new(move |_| {
        bounds.get().map_or([0., 0.], |b| {
            [
                f64::from(b.size.width.0 / host.scale()),
                f64::from(b.size.height.0 / host.scale()),
            ]
        })
    });
    let fit_request = Memo::new(move |_| {
        pane.trajectory.with(|s| {
            s.as_ref()
                .is_some_and(|s| s.map.is_some_and(|m| m.camera.is_none()))
        })
    });
    let fitting = RenderEffect::new(move |_| {
        let pixels = size.get();
        let data = pane.trajectory_data.get();
        let spatial_paths = paths.get();
        if terrain.get() && (!paths_ready.get() || spatial_paths.is_empty()) {
            return;
        }
        if fit_request.get()
            && let Some(data) = data
            && pane.trajectory.with_untracked(|s| {
                s.as_ref().is_some_and(|spec| {
                    data.matches(&pane.lines.get_untracked(), spec)
                        && spec.pairs().iter().all(|p| {
                            pane.lines.with_untracked(|lines| {
                                lines.iter().any(|l| l.source == p.x)
                                    && lines.iter().any(|l| l.source == p.y)
                                    && (p.fix.is_empty() || lines.iter().any(|l| l.source == p.fix))
                            })
                        })
                })
            })
            && let Some(next) = fit(&data, pixels, &pane.plot.hidden.get())
        {
            let mut next = next;
            if terrain.get_untracked() {
                let mut extent = 0_f64;
                for path in spatial_paths.iter() {
                    let Some(ground) = super::flight3::ground_height(service, path.origin) else {
                        return;
                    };
                    let ground = f64::from(ground);
                    for point in path.points.iter().filter(|p| p[2].is_finite()) {
                        let z = point[2] + if path.local { ground } else { 0. };
                        extent = extent.max((z - ground).abs());
                    }
                }
                if extent > 0. {
                    let scale = super::terrain::ground_scale(next);
                    let maximum_scale =
                        720. * scale / (2. * (45_f64.to_radians() / 2.).tan() * extent * 3.);
                    let zoom = (maximum_scale * WORLD / 256.).log2();
                    next.zoom = next
                        .zoom
                        .min(zoom)
                        .clamp(0., super::camera::MAX_INTERACTIVE_ZOOM);
                }
            }
            camera.set(next);
            pane.trajectory.update(|s| {
                if let Some(map) = s.as_mut().and_then(|s| s.map.as_mut()) {
                    map.remember_camera(next);
                }
            });
        }
    });
    let view = RenderEffect::new(move |_| {
        if !visible.get() {
            return;
        }
        let mut c = camera.get();
        let pixels = size.get();
        if let Some(data) = pane.trajectory_data.get()
            && let Some(trace) = data.traces.first()
        {
            let shifted =
                c.center[0] + ((trace.track.origin.x - c.center[0]) / WORLD).round() * WORLD;
            if (shifted - c.center[0]).abs() > WORLD / 2. {
                c.center[0] = shifted;
                camera.set(c);
            }
        }
        if Camera::geometry(pixels).is_some() {
            let ranges = c.ranges(pixels);
            if pane.plot.ranges.get_untracked() != ranges {
                let _ = pane.plot.set_view(ranges);
            }
        }
        if pane.trajectory.with_untracked(|s| {
            s.as_ref().is_some_and(|s| {
                s.map
                    .is_some_and(|m| m.camera.is_some_and(|saved| saved != c))
            })
        }) {
            pane.trajectory.update(|s| {
                if let Some(map) = s.as_mut().and_then(|s| s.map.as_mut()) {
                    map.remember_camera(c);
                }
            });
        }
    });
    on_cleanup_local(move || {
        drop(fitting);
        drop(view);
    });
    let follow_mode = Memo::new(move |_| {
        pane.trajectory
            .with(|s| {
                s.as_ref()
                    .and_then(|s| s.map)
                    .map(|m| (m.follow, m.terrain))
            })
            .unwrap_or_default()
    });
    let follow = RenderEffect::new(move |_| {
        if !follow_mode.get().0 || !visible.get() || !pane.timeline_link.get() {
            return;
        }
        let Some(time) = workbench.timeline.time.get() else {
            return;
        };
        let Some(data) = pane.trajectory_data.get() else {
            return;
        };
        let selected = pane.selected.get();
        let hidden = pane.plot.hidden.get();
        let trace = data
            .traces
            .iter()
            .find(|t| Some(t.id) == selected && !hidden.contains(&t.id.to_string()))
            .or_else(|| {
                data.traces
                    .iter()
                    .find(|t| !hidden.contains(&t.id.to_string()))
            });
        if let Some(point) = trace.and_then(|trace| trace.track.at(time)) {
            let mut next = camera.get();
            next.center = [point.x, point.y];
            if next != camera.get_untracked() {
                camera.set(next);
            }
        }
    });
    on_cleanup_local(move || drop(follow));
    let stop_follow = move || {
        if follow_mode.get_untracked().0 {
            super::settings::update(workbench, pane, |s| {
                if let Some(map) = s.map.as_mut() {
                    map.follow = false;
                }
            });
        }
    };
    let hover = RwSignal::new_local(None::<super::hover::Hit>);
    let changed = super::hover::Changed::new(move |hit: Option<super::hover::Hit>| {
        if let Some(hit) = hit
            && pane.timeline_link.get_untracked()
            && !follow_mode.get_untracked().0
        {
            workbench.timeline.hover(hit.time);
        }
        if hover.get_untracked() != hit {
            hover.set(hit);
        }
    });
    let clear_hover = RenderEffect::new(move |_| {
        camera.get();
        orbit.get();
        size.get();
        terrain.get();
        visible.get();
        pane.trajectory_data.get();
        pane.plot.hidden.get();
        paths.get();
        hover.set(None);
    });
    on_cleanup_local(move || drop(clear_hover));
    use super::color_legend::ColorLegendProps;
    use super::hover::HoverViewProps;
    use super::toolbar::MapToolbarProps;
    use crate::plot_menu::PlotMenuEditorProps;
    let settings_open = RwSignal::new_local(false);
    let press = StoredValue::new_local(None::<[f64; 2]>);
    view! {column(class="flight-map",on:key_down=move |e|{if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape)&&pane.map_origin_target.get_untracked().is_some(){pane.map_origin_target.set(None);e.prevent_default();e.stop_propagation();}}) {
        MapToolbar(workbench=workbench,pane=pane,camera=camera,orbit=orbit,settings=settings_open)
        PlotMenuEditor(workbench=workbench,pane=pane,open=settings_open)
        if move ||pane.map_origin_target.get().is_some(){row(class="flight-map-controls"){text(class="map-origin-prompt"){"Choose anchor on 2D map"}ActionButton(on:click=move |_|pane.map_origin_target.set(None)){"Cancel"}}}
        column(class="flight-map-view",node_ref=host,on:pointer_move:capture=move |e| {
            if terrain.get_untracked() {return;}
            if press.get_value().is_some() || pane.map_origin_target.get_untracked().is_some() {changed.run(None);return;}
            if let Some((x,y))=plot::zgui_renderer::local_position(&legend,e.position) &&let Some(b)=legend.window_bounds()
                &&x>=0.&&y>=0.&&x<f64::from(b.size.width.0/legend.scale())&&y<f64::from(b.size.height.0/legend.scale()) {changed.run(None);return;}
            let hit=plot::zgui_renderer::local_position(&host,e.position).and_then(|(x,y)|pane.trajectory_data.with_untracked(|data|data.as_ref().and_then(|data|super::hover::flat(data,camera.get_untracked(),size.get_untracked(),[x,y],&pane.plot.hidden.get_untracked()))));
            changed.run(hit);
        },on:pointer_leave=move |_|changed.run(None),on:wheel:capture=move |_|{changed.run(None);stop_follow();},style:background=move ||Some(pane.background.get().map_or_else(||"#12181e".into(),|[r,g,b]|format!("#{r:02x}{g:02x}{b:02x}"))),on:pointer_down:capture=move |e| {
            changed.run(None);
            if let Some((x,y))=plot::zgui_renderer::local_position(&legend,e.position) &&let Some(b)=legend.window_bounds()
                &&x>=0.&&y>=0.&&x<f64::from(b.size.width.0/legend.scale())&&y<f64::from(b.size.height.0/legend.scale()) {press.set_value(None);return;}
            if matches!(e.button,Some(zgui::vocab::PointerButton::Primary | zgui::vocab::PointerButton::Middle)){stop_follow();}
            if !terrain.get_untracked() && e.button==Some(zgui::vocab::PointerButton::Primary) &&let Some(source)=pane.map_origin_target.get_untracked() {
                if let Some((x,y))=plot::zgui_renderer::local_position(&host,e.position) &&let Some(g)=Camera::geometry(size.get_untracked()) {
                    let c=camera.get_untracked();let ranges=c.ranges(size.get_untracked());
                    let mut coords=geographic([g.x_value(x,ranges.x),g.y_value(y,ranges.y)]);coords[0]=(coords[0]+180.).rem_euclid(360.)-180.;
                    if let Err(error)=set_origin(pane,&source,coords){workbench.status.set(error);}else {
                        pane.trajectory.update(|s|if let Some(map)=s.as_mut().and_then(|s|s.map.as_mut()){map.remember_camera(c);});
                        let builder=workbench.trajectory_builder();if builder.target.get_untracked()==Some(pane.id){let _=set_origin(builder.pane,&source,coords);}
                    }
                }
                pane.map_origin_target.set(None);press.set_value(None);e.prevent_default();e.stop_propagation();return;
            }
            if e.button==Some(zgui::vocab::PointerButton::Primary) && let Some((x,y))=plot::zgui_renderer::local_position(&host,e.position){press.set_value(Some([x,y]));}
        },on:pointer_up:capture=move |e| {
            if e.button==Some(zgui::vocab::PointerButton::Primary) {
                if !terrain.get_untracked() && pane.timeline_link.get_untracked() &&let Some(start)=press.get_value()
                    &&let Some((x,y))=plot::zgui_renderer::local_position(&host,e.position)
                    &&(start[0]-x).hypot(start[1]-y)<4.
                    &&let Some(g)=Camera::geometry(size.get_untracked())
                    &&let Some(data)=pane.trajectory_data.get_untracked() {
                    let ranges=pane.plot.ranges.get_untracked();
                    let target=PlotPoint{x:g.x_value(x,ranges.x),y:g.y_value(y,ranges.y)};
                    if let Some(time)=pick_time(&data,target,ranges,size.get_untracked(),&pane.plot.hidden.get_untracked()){let _=workbench.timeline.pin(time);}
                }
                press.set_value(None);
            }
        },on:pointer_cancel=move |_|press.set_value(None)) {
            if move ||visible.get() {
                if move ||terrain.get(){
                    TerrainView(camera=camera,orbit=orbit,service=service,provider=Some(provider),offline=Some(offline),paths=Some(paths),colors=Some(colors),ground_projection=Some(Signal::derive_local(move ||ground_projection.get())),demand_owner=pane.id+1000,show_help=false,on_hover=Some(changed),time=Some(Signal::derive_local(move ||if pane.timeline_link.get(){workbench.timeline.time.get()}else{None})),on_pick=Some(zgui::reactive::UnsyncCallback::new(move |(scene,camera,orbit,size,cursor):(std::sync::Arc<super::terrain::Scene>,Camera,super::terrain::Orbit,[f64;2],[f64;2])|{if pane.timeline_link.get_untracked() &&let Some(time)=super::flight3::pick_time(&scene,camera,orbit,size,cursor){let _=workbench.timeline.pin(time);}})))
                }else{
                    MapView(camera=camera,gpu=false,maps=maps,service=service,provider=Some(provider),offline=Some(offline),transparent_background=true,demand_owner=pane.id+1000,on_reset=zgui::reactive::UnsyncCallback::new(move |()|pane.fit_axes(plot::interaction::Axes::BOTH)))
                    TrajectoryOverlay(workbench=workbench,pane=pane,visible=visible)
                }
                HoverView(pane=pane,hit=hover,size=Signal::derive_local(move ||size.get()))
                ColorLegend(colors=colors)
                column(class="trajectory-plot-controls",node_ref=legend,on:pointer_enter=move |_|changed.run(None),on:pointer_down:capture=move |_|press.set_value(None)) {TrajectoryLegend(workbench=workbench,pane=pane)}
            }
        }
    }}
}

#[cfg(test)]
#[path = "flight_tests.rs"]
mod tests;
