//! Flight-mode changes from the plot's first state band, marked along the 2D path.
use super::camera::Camera;
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
}

#[component]
pub fn ModeMarkers(
    workbench: Workbench,
    pane: Pane,
    camera: RwSignal<Camera, LocalStorage>,
    size: Memo<[f64; 2]>,
    terrain: Memo<bool>,
) -> impl IntoView {
    let markers = Memo::new(move |_| {
        let enabled = pane
            .trajectory
            .with(|s| s.as_ref().and_then(|s| s.map).is_some_and(|m| m.modes));
        if !enabled || terrain.get() {
            return Vec::new();
        }
        let size = size.get();
        let (Some(g), Some(data)) = (Camera::geometry(size), pane.trajectory_data.get()) else {
            return Vec::new();
        };
        let ranges = camera.get().ranges(size);
        let hidden = pane.plot.hidden.get();
        let Some(trace) = data
            .traces
            .iter()
            .find(|trace| !hidden.contains(&trace.id.to_string()))
        else {
            return Vec::new();
        };
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
        let mut previous = None;
        for interval in states.intervals.iter() {
            if previous == Some(interval.value) {
                continue;
            }
            previous = Some(interval.value);
            let Some(rule) = states.rules.iter().find(|r| r.value == interval.value) else {
                continue;
            };
            let Some(point) = trace.track.at(interval.start) else {
                continue;
            };
            let (x, y) = (g.x(point.x, ranges.x), g.y(point.y, ranges.y));
            if !(0. ..=size[0]).contains(&x) || !(0. ..=size[1]).contains(&y) {
                continue;
            }
            let stack = out
                .iter()
                .filter(|m| (m.x - x).hypot(m.y - y) < 18.)
                .count();
            out.push(Marker {
                x,
                y,
                stack,
                name: rule.name.clone(),
                color: rule.color,
            });
            if out.len() == MAX_MARKERS {
                break;
            }
        }
        out
    });
    view! {
        for marker in move ||markers.get(),key=|m|(m.name.clone(),m.x.to_bits(),m.y.to_bits()) {
            row(class="map-mode-marker",style:left=format!("{}px",marker.x),style:top=format!("{}px",marker.y)) {
                box(class="map-mode-dot",style:background=format!("#{:02x}{:02x}{:02x}",marker.color[0],marker.color[1],marker.color[2])) {}
                text(class="map-mode-label",style:transform=format!("translateY({}px)",-10.-(marker.stack as f64)*17.),
                    style:border_left_color=format!("#{:02x}{:02x}{:02x}",marker.color[0],marker.color[1],marker.color[2])) {{marker.name.clone()}}
            }
        }
    }
}
