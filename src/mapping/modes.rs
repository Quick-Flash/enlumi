//! State changes from the plot's first state band, marked along the path.
use super::{
    camera::Camera,
    hover::Projection,
    terrain::{Orbit, Scene},
};
use crate::state::{Pane, Workbench};
use zgui::prelude::*;

const MAX_MARKERS: usize = 60;

#[derive(Clone, PartialEq)]
struct Marker {
    x: f64,
    y: f64,
    /// Rows to shift the label up so nearby changes stay readable.
    stack: usize,
    name: String,
    color: [u8; 3],
    faded: bool,
}

#[component]
#[allow(clippy::too_many_arguments)] // Independent component props.
pub fn ModeMarkers(
    workbench: Workbench,
    pane: Pane,
    camera: RwSignal<Camera, LocalStorage>,
    size: Memo<[f64; 2]>,
    terrain: Memo<bool>,
    orbit: RwSignal<Orbit, LocalStorage>,
    scene: RwSignal<Option<std::sync::Arc<Scene>>, LocalStorage>,
) -> impl IntoView {
    let markers = Memo::new(move |_| {
        let enabled = pane
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map).is_some_and(|m| m.modes));
        if !enabled {
            return Vec::new();
        }
        let size = size.get();
        let (Some(g), Some(data)) = (Camera::geometry(size), pane.trajectory_data.get()) else {
            return Vec::new();
        };
        let ranges = camera.get().ranges(size);
        let scene = terrain.get().then(|| scene.get()).flatten();
        let projection = scene
            .as_ref()
            .map(|scene| Projection::new(scene, camera.get(), orbit.get(), size));
        let hidden = pane.plot.hidden.get();
        let window = super::flight::highlight_window(workbench, pane);
        let hidden_geometry = pane.hidden_geometry.get();
        let Some(states) = workbench
            .ordered_geometry(pane)
            .into_iter()
            .filter(|item| !hidden_geometry.contains(&item.id))
            .find_map(|item| item.states.filter(|s| s.error.is_none()))
        else {
            return Vec::new();
        };
        let mut out: Vec<Marker> = Vec::new();
        for trace in data.traces.iter().filter(|trace| {
            !hidden.contains(&trace.id.to_string())
                && crate::trace_color::current(pane, Some(trace.id)).state_markers
        }) {
            let mut previous = None;
            for interval in states.intervals.iter() {
                if previous == Some(interval.value) {
                    continue;
                }
                previous = Some(interval.value);
                let Some(rule) = states.rules.iter().find(|r| r.value == interval.value) else {
                    continue;
                };
                let screen = if terrain.get() {
                    scene
                        .as_ref()
                        .zip(projection.as_ref())
                        .and_then(|(scene, projection)| {
                            scene
                                .paths
                                .iter()
                                .filter(|path| path.id & !super::flight3::FADED == trace.id)
                                .find_map(|path| {
                                    projection
                                        .point(path.at(interval.start)?, scene.path_base(path)?)
                                        .map(|(point, _)| point)
                                })
                        })
                } else {
                    trace
                        .track
                        .at(interval.start)
                        .map(|point| [g.x(point.x, ranges.x), g.y(point.y, ranges.y)])
                };
                let Some([x, y]) = screen else {
                    continue;
                };
                if !(0. ..=size[0]).contains(&x) || !(0. ..=size[1]).contains(&y) {
                    continue;
                }
                let stack = out
                    .iter()
                    .filter(|m| (m.x - x).hypot(m.y - y) < 20.)
                    .count();
                out.push(Marker {
                    x,
                    y,
                    stack,
                    name: rule.name.clone(),
                    color: rule.color,
                    faded: window
                        .is_some_and(|[start, end]| interval.start < start || interval.start > end),
                });
                if out.len() == MAX_MARKERS {
                    return out;
                }
            }
        }
        out
    });
    view! {
        for marker in move ||markers.get(),key=|m|(m.name.clone(),m.x.to_bits(),m.y.to_bits(),m.stack,m.color,m.faded) {
            row(class="map-mode-marker",style:left=format!("{}px",marker.x),style:top=format!("{}px",marker.y),style:opacity=if marker.faded {"0.35"} else {"1"}) {
                box(class="map-mode-dot",style:background=format!("#{:02x}{:02x}{:02x}",marker.color[0],marker.color[1],marker.color[2])) {}
                text(class="map-mode-label",style:top=format!("{}px",-22.-(marker.stack as f64)*18.),
                    style:border_left_color=format!("#{:02x}{:02x}{:02x}",marker.color[0],marker.color[1],marker.color[2])) {{marker.name.clone()}}
            }
        }
    }
}
