//! Sample snapping shared by map hover overlays.
use super::{
    camera::Camera,
    terrain::{Orbit, Scene},
};
use crate::trajectory::Tracks;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub id: u64,
    pub time: f64,
    pub point: [f64; 2],
    pub screen: [f64; 2],
    pub altitude: Option<(f64, f64, bool)>,
}
pub type Changed = zgui::reactive::UnsyncCallback<Option<Hit>, ()>;

pub fn flat(
    data: &Tracks,
    camera: Camera,
    size: [f64; 2],
    cursor: [f64; 2],
    hidden: &BTreeSet<String>,
) -> Option<Hit> {
    let g = Camera::geometry(size)?;
    let ranges = camera.ranges(size);
    let target = plot::PlotPoint {
        x: g.x_value(cursor[0], ranges.x),
        y: g.y_value(cursor[1], ranges.y),
    };
    let mut best = (144., None);
    for trace in &data.traces {
        if hidden.contains(&trace.id.to_string()) {
            continue;
        }
        let Some(index) =
            trace.track.scatter.series()[0].nearest(target, ranges, (size[0], size[1]), 12.)
        else {
            continue;
        };
        let p = trace.track.points[index];
        let screen = [g.x(p.x, ranges.x), g.y(p.y, ranges.y)];
        let distance = (screen[0] - cursor[0]).powi(2) + (screen[1] - cursor[1]).powi(2);
        if distance <= best.0 {
            best = (
                distance,
                Some(Hit {
                    id: trace.id,
                    time: trace.track.times[index],
                    point: [p.x, p.y],
                    screen,
                    altitude: None,
                }),
            );
        }
    }
    best.1
}

pub struct Projection {
    matrix: [f32; 16],
    camera: Camera,
    size: [f64; 2],
    scale: f64,
    base: f64,
    exaggeration: f64,
}
impl Projection {
    pub fn new(scene: &Scene, camera: Camera, orbit: Orbit, size: [f64; 2]) -> Self {
        Self {
            matrix: orbit.matrix(camera, size),
            camera,
            size,
            scale: super::terrain::ground_scale(camera),
            base: f64::from(scene.center_height(camera)),
            exaggeration: f64::from(orbit.exaggeration),
        }
    }
    pub fn point(&self, point: [f64; 3], origin: f32) -> Option<([f64; 2], f64)> {
        if !point.iter().all(|v| v.is_finite()) {
            return None;
        }
        let v = [
            ((point[0] - self.camera.center[0]) * self.scale) as f32,
            ((point[2] + f64::from(origin) - self.base) * self.exaggeration + 0.5) as f32,
            ((self.camera.center[1] - point[1]) * self.scale) as f32,
            1.,
        ];
        let clip = std::array::from_fn::<_, 4, _>(|row| {
            (0..4)
                .map(|col| self.matrix[col * 4 + row] * v[col])
                .sum::<f32>()
        });
        if !clip.iter().all(|v| v.is_finite()) || clip[3] <= 0. || clip[2] < 0. || clip[2] > clip[3]
        {
            return None;
        }
        Some((
            [
                (f64::from(clip[0] / clip[3]) + 1.) * self.size[0] / 2.,
                (1. - f64::from(clip[1] / clip[3])) * self.size[1] / 2.,
            ],
            f64::from(clip[3]),
        ))
    }
}

#[derive(Default, PartialEq)]
pub struct Spatial(HashMap<(i32, i32), Vec<(Hit, f64)>>);
impl Spatial {
    pub fn new(scene: &Scene, camera: Camera, orbit: Orbit, size: [f64; 2]) -> Self {
        let mut index = Self::default();
        if Camera::geometry(size).is_none() {
            return index;
        }
        let projection = Projection::new(scene, camera, orbit, size);
        for path in scene.paths.iter() {
            let Some(base) = scene.path_base(path) else {
                continue;
            };
            for (&point, &time) in path.points.iter().zip(path.times.iter()) {
                let Some((screen, depth)) = projection.point(point, base) else {
                    continue;
                };
                if screen[0] < -12.
                    || screen[1] < -12.
                    || screen[0] > size[0] + 12.
                    || screen[1] > size[1] + 12.
                {
                    continue;
                }
                let hit = Hit {
                    id: path.id,
                    time,
                    point: [point[0], point[1]],
                    screen,
                    altitude: Some((point[2], point[2] + f64::from(base), path.local)),
                };
                index
                    .0
                    .entry(Self::cell(screen))
                    .or_default()
                    .push((hit, depth));
            }
        }
        index
    }
    fn cell(p: [f64; 2]) -> (i32, i32) {
        ((p[0] / 24.).floor() as i32, (p[1] / 24.).floor() as i32)
    }
    pub fn nearest(&self, cursor: [f64; 2]) -> Option<Hit> {
        let cell = Self::cell(cursor);
        let mut best = (144., f64::INFINITY, None);
        for x in cell.0 - 1..=cell.0 + 1 {
            for y in cell.1 - 1..=cell.1 + 1 {
                for &(hit, depth) in self.0.get(&(x, y)).into_iter().flatten() {
                    let distance =
                        (hit.screen[0] - cursor[0]).powi(2) + (hit.screen[1] - cursor[1]).powi(2);
                    if distance < best.0 || (distance == best.0 && depth < best.1) {
                        best = (distance, depth, Some(hit));
                    }
                }
            }
        }
        best.2
    }
}

pub fn label(hit: Hit, name: &str) -> String {
    let [lon, lat] = super::camera::geographic(hit.point);
    let lon = (lon + 180.).rem_euclid(360.) - 180.;
    let mut label = format!(
        "{name}\nt: {:.3} s\nLat: {lat:.7}\u{b0}\nLon: {lon:.7}\u{b0}",
        hit.time
    );
    if let Some((z, elevation, local)) = hit.altitude {
        label.push_str(&format!(
            "\nZ: {z:.2} m ({})",
            if local { "relative" } else { "absolute" }
        ));
        if local {
            label.push_str(&format!("\nElevation: {elevation:.2} m"));
        }
    }
    label
}

use zgui::prelude::*;
#[component]
pub fn HoverView(
    pane: crate::state::Pane,
    hit: RwSignal<Option<Hit>, LocalStorage>,
    size: Signal<[f64; 2], LocalStorage>,
) -> impl IntoView {
    let pin_left = move || Some(format!("{}px", hit.get().map_or(0., |h| h.screen[0])));
    let pin_top = move || Some(format!("{}px", hit.get().map_or(0., |h| h.screen[1])));
    let left = move || {
        Some(format!(
            "{}px",
            hit.get().map_or(0., |h| (h.screen[0] + 16.)
                .min((size.get()[0] - 258.).max(4.)))
        ))
    };
    let top = move || {
        Some(format!(
            "{}px",
            hit.get().map_or(0., |h| (h.screen[1] + 16.)
                .min((size.get()[1] - 170.).max(4.)))
        ))
    };
    view! {
        if move ||hit.get().is_some() {
            box(class="map-hover-pin", style:left=pin_left, style:top=pin_top) {}
            text(class="map-hover-tooltip", style:left=left, style:top=top) {
                {move ||hit.get().map_or_else(String::new,|hit| {
                    let name=pane.lines.with(|lines|lines.iter().find(|l|l.id==hit.id).map(|l|l.name.clone()).unwrap_or_default());
                    label(hit,&name)
                })}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn flat_hover_snaps_samples_respects_hidden_and_wraps_longitude() {
        let lines = [
            ("lon", vec![179.99, -179.99, -179.98]),
            ("lat", vec![40., 40., 40.]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (source, values))| crate::state::Line {
            id: i as u64 + 1,
            source: source.into(),
            name: source.into(),
            revision: 0,
            data: plot::time_series::SeriesData::new(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(i, y)| plot::PlotPoint { x: i as f64, y })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        })
        .collect::<Vec<_>>();
        let data = Tracks::prepare(
            &lines,
            &crate::trajectory::Spec {
                x: "lon".into(),
                y: "lat".into(),
                gps_scale: Some(1.),
                map: Some(Default::default()),
                ..Default::default()
            },
        )
        .unwrap();
        let camera = Camera {
            center: super::super::camera::project(180.01, 40.).unwrap(),
            zoom: 14.,
        };
        let size = [800., 600.];
        let hit = flat(&data, camera, size, [404., 302.], &BTreeSet::new()).unwrap();
        assert_eq!(hit.time, 1.);
        assert!((hit.screen[0] - 400.).abs() < 0.001 && (hit.screen[1] - 300.).abs() < 0.001);
        assert!(label(hit, "GPS").contains("Lon: -179.9900000\u{b0}"));
        assert!(
            flat(
                &data,
                camera,
                size,
                [404., 302.],
                &BTreeSet::from(["2".into()])
            )
            .is_none()
        );
        assert!(flat(&data, camera, size, [0., 0.], &BTreeSet::new()).is_none());
    }
    #[test]
    fn terrain_hover_snaps_projected_samples_and_reports_unexaggerated_altitude() {
        let camera = Camera {
            center: [0., 0.],
            zoom: 14.,
        };
        let orbit = Orbit {
            exaggeration: 2.,
            ..Default::default()
        };
        let size = [800., 600.];
        let mut scene = Scene::default();
        scene.paths = Arc::from([super::super::flight3::Path3 {
            id: 7,
            origin: [0., 0.],
            points: Arc::from([[-100., 0., 20.], [100., 0., 20.], [f64::NAN, 0., 20.]]),
            times: Arc::from([0., 1., 2.]),
            color: [255; 4],
            width: 2.,
            drawing: Default::default(),
            radius: 3.,
            marker: plot::plot_data::MarkerShape::Circle,
            local: false,
        }]);
        let projection = Projection::new(&scene, camera, orbit, size);
        let (screen, _) = projection.point([100., 0., 20.], 0.).unwrap();
        let index = Spatial::new(&scene, camera, orbit, size);
        let hit = index.nearest([screen[0] + 3., screen[1] + 2.]).unwrap();
        assert_eq!(
            (hit.id, hit.time, hit.altitude),
            (7, 1., Some((20., 20., false)))
        );
        assert_eq!(hit.screen, screen);
        assert!(index.nearest([0., 0.]).is_none());
        assert!(projection.point([0., 0., 1e12], 0.).is_none());
        let local = Hit {
            altitude: Some((20., 1220., true)),
            ..hit
        };
        let text = label(local, "local");
        assert!(text.contains("Z: 20.00 m (relative)") && text.contains("Elevation: 1220.00 m"));
    }
}
