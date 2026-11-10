//! Fit the actual projected 3D bounds, preserving current orbit.
use super::{
    camera::{Camera, MAX_INTERACTIVE_ZOOM},
    hover::Projection,
    terrain::{Orbit, Scene},
};

pub fn fit(mut camera: Camera, scene: &Scene, orbit: Orbit, size: [f64; 2]) -> Option<Camera> {
    Camera::geometry(size)?;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for path in scene.paths.iter() {
        let base = scene.path_base(path)?;
        for point in path
            .points
            .iter()
            .filter(|p| p.iter().all(|v| v.is_finite()))
        {
            let point = [point[0], point[1], point[2] + f64::from(base)];
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
    }
    if !min.iter().all(|v| v.is_finite()) {
        return None;
    }
    let corners = (0..8)
        .map(|bits| {
            std::array::from_fn(|axis| {
                if bits & (1 << axis) == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            })
        })
        .collect::<Vec<_>>();
    let fits = |camera| {
        let projection = Projection::new(scene, camera, orbit, size);
        corners.iter().all(|&point| {
            projection.point(point, 0.).is_some_and(|(p, _)| {
                p[0] >= size[0] * 0.1
                    && p[0] <= size[0] * 0.9
                    && p[1] >= size[1] * 0.1
                    && p[1] <= size[1] * 0.9
            })
        })
    };
    camera.zoom = camera.zoom.clamp(0., MAX_INTERACTIVE_ZOOM);
    if fits(camera) {
        return Some(camera);
    }
    let (mut low, mut high) = (0., camera.zoom);
    camera.zoom = low;
    if !fits(camera) {
        return Some(camera);
    }
    for _ in 0..24 {
        camera.zoom = (low + high) / 2.;
        if fits(camera) {
            low = camera.zoom;
        } else {
            high = camera.zoom;
        }
    }
    camera.zoom = low;
    Some(camera)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_contains_tall_traces_across_orbits_and_viewport_shapes() {
        let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
        let paths = super::super::flight3::prepare(
            &tracks,
            &lines,
            &std::collections::BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        let mut scene = Scene::default();
        scene.paths = paths;
        for path in std::sync::Arc::make_mut(&mut scene.paths) {
            for point in std::sync::Arc::make_mut(&mut path.points) {
                point[2] *= 100.;
            }
        }
        for size in [[320., 900.], [1400., 250.], [800., 600.]] {
            for (bearing, pitch) in [(0., 2.), (90., 45.), (210., 85.)] {
                let orbit = Orbit {
                    bearing,
                    pitch,
                    exaggeration: 3.,
                    ..Default::default()
                };
                let camera = super::super::flight::fit(&tracks, size, &Default::default()).unwrap();
                let fitted = fit(camera, &scene, orbit, size).unwrap();
                assert!(fitted.zoom < camera.zoom);
                let projection = Projection::new(&scene, fitted, orbit, size);
                for point in scene.paths[0]
                    .points
                    .iter()
                    .filter(|p| p.iter().all(|v| v.is_finite()))
                {
                    let (pixel, _) = projection.point(*point, 0.).unwrap();
                    assert!(pixel[0] >= size[0] * 0.1 && pixel[0] <= size[0] * 0.9);
                    assert!(pixel[1] >= size[1] * 0.1 && pixel[1] <= size[1] * 0.9);
                }
            }
        }
    }
}
