//! Balanced 3D error hierarchy. Raw samples remain available for picking and time.
use super::{
    camera::Camera,
    flight3::Path3,
    ground_path::Segments,
    telemetry::Coloring,
    terrain::{Orbit, ground_scale},
};
use std::{collections::BTreeMap, sync::Arc};
fn finite(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}
fn distance(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
    let v = std::array::from_fn::<_, 3, _>(|i| p[i] - a[i]);
    let length = d.iter().map(|v| v * v).sum::<f64>();
    let t = if length > 0. {
        (v.iter().zip(d).map(|(a, b)| a * b).sum::<f64>() / length).clamp(0., 1.)
    } else {
        0.
    };
    (0..3).map(|i| (v[i] - d[i] * t).powi(2)).sum()
}
struct Hierarchy {
    points: Arc<[[f64; 3]]>,
    errors: Vec<f64>,
    runs: Vec<(usize, usize)>,
    bounds: ([f64; 3], [f64; 3]),
}
impl Hierarchy {
    fn new(points: Arc<[[f64; 3]]>) -> Self {
        let mut hierarchy = Self {
            errors: vec![0.; points.len()],
            points,
            runs: Vec::new(),
            bounds: ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
        };
        let mut start = None;
        for (i, &point) in hierarchy.points.iter().enumerate() {
            if finite(point) {
                start.get_or_insert(i);
                for (axis, value) in point.into_iter().enumerate() {
                    hierarchy.bounds.0[axis] = hierarchy.bounds.0[axis].min(value);
                    hierarchy.bounds.1[axis] = hierarchy.bounds.1[axis].max(value);
                }
            } else if let Some(start) = start.take() {
                hierarchy.runs.push((start, i - 1));
            }
        }
        if let Some(start) = start {
            hierarchy.runs.push((start, hierarchy.points.len() - 1));
        }
        for (a, b) in hierarchy.runs.clone() {
            hierarchy.build(a, b);
        }
        hierarchy
    }
    fn build(&mut self, a: usize, b: usize) {
        if b <= a + 1 {
            return;
        }
        let mid = (a + b) / 2;
        self.errors[mid] = self.points[a + 1..b]
            .iter()
            .map(|&p| distance(p, self.points[a], self.points[b]))
            .fold(0., f64::max);
        self.build(a, mid);
        self.build(mid, b);
    }
    fn visit(&self, a: usize, b: usize, tolerance: f64, indices: &mut Vec<usize>) {
        if b <= a + 1 || self.errors[(a + b) / 2] <= tolerance {
            indices.push(b);
        } else {
            let mid = (a + b) / 2;
            self.visit(a, mid, tolerance, indices);
            self.visit(mid, b, tolerance, indices);
        }
    }
    fn indices(&self, tolerance: f64) -> Vec<usize> {
        let mut output = Vec::new();
        for &(a, b) in &self.runs {
            if let Some(&last) = output.last() {
                output.push(last + 1);
            }
            output.push(a);
            if a != b {
                self.visit(a, b, tolerance * tolerance, &mut output);
            }
        }
        output
    }
}
#[derive(Clone, Copy)]
pub struct View {
    pub camera: Camera,
    pub orbit: Orbit,
    pub size: [f64; 2],
    pub focus: f32,
}
impl View {
    fn tolerance(self, hierarchy: &Hierarchy, origin: f32) -> f64 {
        let (lo, hi) = hierarchy.bounds;
        let scale = ground_scale(self.camera);
        let exaggeration = f64::from(self.orbit.exaggeration);
        let x = (lo[0] - self.camera.center[0])
            .abs()
            .max((hi[0] - self.camera.center[0]).abs())
            * scale;
        let y = (lo[1] - self.camera.center[1])
            .abs()
            .max((hi[1] - self.camera.center[1]).abs())
            * scale;
        let z = (lo[2] + f64::from(origin - self.focus))
            .abs()
            .max((hi[2] + f64::from(origin - self.focus)).abs())
            * exaggeration;
        let radius = (x * x + y * y + z * z).sqrt();
        let tangent = (45_f64.to_radians() / 2.).tan();
        let distance = (720. / (2. * tangent * self.camera.scale()) * scale).max(1.);
        // All orbit directions share this conservative perspective error bound.
        // Near/inside the bounds, keep exact geometry rather than risk clipping.
        if radius >= distance * 0.9 || !radius.is_finite() {
            return 0.;
        }
        let focal = self.size[1].max(1.) / (2. * tangent);
        // Inserting exact color boundaries can at most double chord error.
        0.1 * (distance - radius).powi(2) / (focal * distance * scale.max(exaggeration))
    }
}
struct Trace {
    hierarchy: Hierarchy,
    colors: Option<Arc<Coloring>>,
    forced: Vec<usize>,
}
struct Ground {
    source: Segments,
    hierarchy: Hierarchy,
}
#[derive(Default)]
pub struct Cache {
    traces: BTreeMap<u64, Trace>,
    grounds: BTreeMap<u64, Ground>,
}
impl Cache {
    pub fn retain(&mut self, paths: &[Path3]) {
        self.traces
            .retain(|id, _| paths.iter().any(|p| p.id == *id));
        self.grounds
            .retain(|id, _| paths.iter().any(|p| p.id == *id));
    }
    pub fn trace(
        &mut self,
        path: &Path3,
        colors: &Arc<Coloring>,
        view: View,
        origin: f32,
    ) -> Vec<usize> {
        let entry = self.traces.entry(path.id).or_insert_with(|| Trace {
            hierarchy: Hierarchy::new(path.points.clone()),
            colors: None,
            forced: Vec::new(),
        });
        if !Arc::ptr_eq(&entry.hierarchy.points, &path.points) {
            *entry = Trace {
                hierarchy: Hierarchy::new(path.points.clone()),
                colors: None,
                forced: Vec::new(),
            };
        }
        if entry
            .colors
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, colors))
        {
            entry.forced.clear();
            if let Some(values) = colors.samples.get(&path.id) {
                let bucket = |i| {
                    values
                        .get(i)
                        .and_then(|&value| colors.bucket_for(path.id, value))
                };
                for i in 1..path.points.len() {
                    if bucket(i) != bucket(i - 1) {
                        entry.forced.extend([i - 1, i]);
                    }
                }
            }
            for tint in &colors.tints {
                for interval in tint.states.intervals.iter() {
                    for time in [interval.start, interval.end] {
                        let i = path.times.partition_point(|t| *t < time);
                        if i > 0 && i < path.points.len() {
                            entry.forced.extend([i - 1, i]);
                        }
                    }
                }
            }
            entry.colors = Some(colors.clone());
        }
        let mut indices = entry
            .hierarchy
            .indices(view.tolerance(&entry.hierarchy, origin));
        indices.extend(&entry.forced);
        indices.sort_unstable();
        indices.dedup();
        indices
    }
    pub fn ground(&mut self, id: u64, source: Segments, view: View) -> Vec<[[f64; 3]; 2]> {
        let entry = self
            .grounds
            .entry(id)
            .or_insert_with(|| Self::ground_entry(source.clone()));
        if !Arc::ptr_eq(&entry.source, &source) {
            *entry = Self::ground_entry(source);
        }
        let hierarchy = &entry.hierarchy;
        hierarchy
            // Preserve terrain draping: a screen-small shortcut could otherwise
            // cut through a ridge and reopen gaps in the ground annotation.
            .indices(view.tolerance(hierarchy, 0.).min(0.01))
            .windows(2)
            .filter_map(|pair| {
                let (a, b) = (hierarchy.points[pair[0]], hierarchy.points[pair[1]]);
                (finite(a) && finite(b)).then_some([a, b])
            })
            .collect()
    }
    fn ground_entry(source: Segments) -> Ground {
        let mut points = Vec::new();
        let mut last = None;
        for &[a, b] in source.iter() {
            if last != Some(a) {
                if last.is_some() {
                    points.push([f64::NAN; 3]);
                }
                points.push(a);
            }
            points.push(b);
            last = Some(b);
        }
        Ground {
            source,
            hierarchy: Hierarchy::new(points.into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_curve_stays_within_quarter_pixel_across_orbits() {
        let points: Arc<[[f64; 3]]> = (0..5000)
            .map(|i| {
                let a = i as f64 / 4999. * std::f64::consts::TAU;
                [300. * a.cos(), 300. * a.sin(), 150. + 50. * (a * 3.).sin()]
            })
            .collect::<Vec<_>>()
            .into();
        let hierarchy = Hierarchy::new(points.clone());
        let scene = super::super::terrain::Scene::default();
        for size in [[800., 600.], [320., 900.]] {
            for pitch in [3., 45., 80.] {
                for bearing in [0., 70., 200.] {
                    let view = View {
                        camera: Camera {
                            center: [0., 0.],
                            zoom: 16.,
                        },
                        orbit: Orbit {
                            pitch,
                            bearing,
                            ..Default::default()
                        },
                        size,
                        focus: 0.,
                    };
                    let indices = hierarchy.indices(view.tolerance(&hierarchy, 0.));
                    assert!(
                        indices.len() < 500,
                        "Dense curve should substantially reduce display work"
                    );
                    let projection =
                        super::super::hover::Projection::new(&scene, view.camera, view.orbit, size);
                    for pair in indices.windows(2) {
                        let a = projection.point(points[pair[0]], 0.).unwrap().0;
                        let b = projection.point(points[pair[1]], 0.).unwrap().0;
                        for &point in &points[pair[0]..=pair[1]] {
                            let p = projection.point(point, 0.).unwrap().0;
                            assert!(
                                distance([p[0], p[1], 0.], [a[0], a[1], 0.], [b[0], b[1], 0.])
                                    .sqrt()
                                    < 0.25
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn gaps_and_ground_ridges_survive_detail_reduction() {
        let mut cache = Cache::default();
        let view = View {
            camera: Camera {
                center: [0., 0.],
                zoom: 16.,
            },
            orbit: Orbit::default(),
            size: [800., 600.],
            focus: 0.,
        };
        let source: Segments = Arc::from([
            [[0., 0., 0.], [1., 0., 30.]],
            [[1., 0., 30.], [2., 0., 0.]],
            [[8., 0., 0.], [9., 0., 0.]],
        ]);
        let lines = cache.ground(1, source, view);
        assert!(lines.iter().flatten().any(|p| p[2] == 30.));
        assert!(lines.iter().all(|[a, b]| b[0] <= 2. || a[0] >= 8.));
    }
    #[test]
    fn scalar_peaks_and_state_boundary_samples_remain_exact() {
        let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
        let mut path = super::super::flight3::prepare(
            &tracks,
            &lines,
            &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
            &Default::default(),
            &Default::default(),
        )
        .unwrap()[0]
            .clone();
        path.points = (0..1000)
            .map(|i| [i as f64 * 0.01, 0., 100.])
            .collect::<Vec<_>>()
            .into();
        path.times = (0..1000)
            .map(|i| i as f64 * 0.01)
            .collect::<Vec<_>>()
            .into();
        let mut values = vec![0.; 1000];
        values[500] = 100.;
        let colors = Arc::new(Coloring {
            samples: BTreeMap::from([(path.id, values.into())]),
            range: Some([0., 100.]),
            tints: vec![crate::state_tint::tests::fixture()],
            ..Default::default()
        });
        let mut cache = Cache::default();
        let view = View {
            camera: Camera {
                center: [0., 0.],
                zoom: 16.,
            },
            orbit: Orbit::default(),
            size: [800., 600.],
            focus: 0.,
        };
        let indices = cache.trace(&path, &colors, view, 0.);
        for index in [99, 100, 199, 200, 299, 300, 399, 400, 499, 500, 501] {
            assert!(
                indices.contains(&index),
                "Lost source/color boundary {index}"
            );
        }
        assert!(indices.len() < 100);
    }
}
