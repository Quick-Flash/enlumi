//! Two-click distance, bearing and, between samples, timing along the flight.
use super::{
    camera::{Camera, geographic},
    hover::Hit,
};
use crate::{state::Pane, trajectory::Tracks};
use zgui::prelude::*;

const EARTH_RADIUS: f64 = 6_371_008.8;

/// A measured end: a snapped sample, or a free point on the 2D map (mercator units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub point: [f64; 2],
    pub sample: Option<(u64, f64)>,
    /// Absolute elevation in metres, known for 3D samples.
    pub elevation: Option<f64>,
}
impl Mark {
    pub fn from_hit(hit: &Hit) -> Self {
        Self {
            point: hit.point,
            sample: Some((hit.id, hit.time)),
            elevation: hit.altitude.map(|altitude| altitude.1),
        }
    }
}

fn metres(a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let ([lon1, lat1], [lon2, lat2]) = (geographic(a), geographic(b));
    let lat = ((lat1 + lat2) / 2.).to_radians();
    (
        (lon2 - lon1).to_radians() * lat.cos() * EARTH_RADIUS,
        (lat2 - lat1).to_radians() * EARTH_RADIUS,
    )
}
fn distance(m: f64) -> String {
    if m < 1000. {
        format!("{m:.1} m")
    } else {
        format!("{:.2} km", m / 1000.)
    }
}

pub fn readout(a: Mark, b: Mark, data: Option<&Tracks>) -> Vec<String> {
    let (east, north) = metres(a.point, b.point);
    let horizontal = east.hypot(north);
    let bearing = format!(
        "bearing {:03.0}°",
        east.atan2(north).to_degrees().rem_euclid(360.)
    );
    let mut lines = match (a.elevation, b.elevation) {
        (Some(first), Some(second)) => {
            let vertical = second - first;
            vec![
                format!("3D {} · {bearing}", distance(horizontal.hypot(vertical))),
                format!(
                    "horizontal {} · vertical {}{}",
                    distance(horizontal),
                    if vertical < 0. { "−" } else { "+" },
                    distance(vertical.abs())
                ),
            ]
        }
        _ => vec![format!("horizontal {} · {bearing}", distance(horizontal))],
    };
    if let (Some((first, start)), Some((second, end))) = (a.sample, b.sample)
        && first == second
        && let Some(trace) = data.and_then(|data| data.traces.iter().find(|t| t.id == first))
    {
        let (from, to) = (start.min(end), start.max(end));
        let times = &trace.track.times;
        let points = &trace.track.points;
        let begin = times.partition_point(|t| *t < from);
        let finish = times.partition_point(|t| *t <= to);
        let mut flown = 0.;
        for index in begin.max(1)..finish {
            let (p, q) = (points[index - 1], points[index]);
            if [p.x, p.y, q.x, q.y].iter().all(|v| v.is_finite()) {
                let (e, n) = metres([p.x, p.y], [q.x, q.y]);
                flown += e.hypot(n);
            }
        }
        let elapsed = to - from;
        lines.push(format!(
            "Δt {elapsed:.1} s · flown {} · avg {:.1} m/s",
            distance(flown),
            if elapsed > 0. { flown / elapsed } else { 0. }
        ));
    } else if a.sample.is_none() || b.sample.is_none() {
        lines.push("Snap both ends to a trace for timing".into());
    }
    lines
}

/// Line, ends and readout. The line is drawn only in 2D; 3D shows the readout.
#[component]
pub fn MeasureView(
    pane: Pane,
    marks: RwSignal<Vec<Mark>, LocalStorage>,
    measuring: RwSignal<bool, LocalStorage>,
    camera: RwSignal<Camera, LocalStorage>,
    size: Memo<[f64; 2]>,
    terrain: Memo<bool>,
) -> impl IntoView {
    let screen = move |mark: Mark| {
        let size = size.get();
        let g = Camera::geometry(size)?;
        let ranges = camera.get().ranges(size);
        Some([g.x(mark.point[0], ranges.x), g.y(mark.point[1], ranges.y)])
    };
    let ends = Memo::new(move |_| {
        if terrain.get() {
            return Vec::new();
        }
        marks
            .get()
            .into_iter()
            .filter_map(screen)
            .collect::<Vec<_>>()
    });
    let lines = Memo::new(move |_| match marks.get().as_slice() {
        [a, b] => pane
            .trajectory_data
            .with(|data| readout(*a, *b, data.as_deref())),
        [_] => vec!["Click the second point".into()],
        _ => vec!["Click two points · clicks snap to samples".into()],
    });
    view! {if move ||measuring.get() {
        column(class="map-measure",a11y:role=Role::Status) {
            if move ||ends.with(|ends|ends.len()==2) {
                vector(class="map-measure-line",prop:viewBox=move ||{let [w,h]=size.get();PropValue::from(format!("0 0 {w} {h}"))},
                    prop:d=move ||PropValue::from(ends.with(|e|format!("M {} {} L {} {}",e[0][0],e[0][1],e[1][0],e[1][1]))),a11y:hidden=true)
            }
            for index in move ||0..ends.with(Vec::len),key=|index|*index {
                box(class="map-measure-end",style:left=move ||ends.with(|e|e.get(index).map(|p|format!("{}px",p[0]))),style:top=move ||ends.with(|e|e.get(index).map(|p|format!("{}px",p[1])))) {}
            }
            column(class="map-measure-readout") {
                for line in move ||lines.get().into_iter().enumerate().collect::<Vec<_>>(),key=|line|line.clone() {
                    text {{line.1}}
                }
                text(class="map-measure-hint") {"M or Esc to finish"}
            }
        }
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::camera::project;

    #[test]
    fn readout_reports_3d_parts_and_bearing() {
        let a = Mark {
            point: project(8.5456, 47.3977).unwrap(),
            sample: Some((1, 10.)),
            elevation: Some(500.),
        };
        let b = Mark {
            point: project(8.5456, 47.3986).unwrap(),
            sample: Some((2, 20.)),
            elevation: Some(530.),
        };
        let lines = readout(a, b, None);
        // 0.0009° of latitude is about 100 m due north.
        assert!(lines[0].starts_with("3D 104."), "{lines:?}");
        assert!(lines[0].ends_with("bearing 000°"), "{lines:?}");
        assert!(lines[1].contains("horizontal 100.") && lines[1].contains("vertical +30.0 m"));
        let free = Mark {
            sample: None,
            elevation: None,
            ..b
        };
        let lines = readout(a, free, None);
        assert!(lines[0].starts_with("horizontal 100."), "{lines:?}");
        assert_eq!(lines[1], "Snap both ends to a trace for timing");
    }
}
