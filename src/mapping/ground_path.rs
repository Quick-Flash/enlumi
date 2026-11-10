//! Drape each segment over terrain triangle edges so hills cannot hide its middle.
use super::{camera::WORLD, flight3::Path3, terrain::Tile};

fn cuts(tile: &Tile, a: [f64; 2], b: [f64; 2], cuts: &mut Vec<f64>) {
    let span = WORLD / f64::from(1_u32 << tile.id.z);
    let local = |p: [f64; 2]| {
        [
            (p[0] + WORLD / 2.) / span - tile.id.x as f64,
            (WORLD / 2. - p[1]) / span - f64::from(tile.id.y),
        ]
    };
    let (a, b) = (local(a), local(b));
    let (mut lo, mut hi) = (0_f64, 1_f64);
    for axis in 0..2 {
        let delta = b[axis] - a[axis];
        if delta.abs() < 1e-14 {
            if !(0. ..=1.).contains(&a[axis]) {
                return;
            }
        } else {
            let (u, v) = (-a[axis] / delta, (1. - a[axis]) / delta);
            lo = lo.max(u.min(v));
            hi = hi.min(u.max(v));
        }
    }
    if lo >= hi {
        return;
    }
    cuts.extend([lo, hi]);
    for (start, end) in [
        (a[0] * 48., b[0] * 48.),
        (a[1] * 48., b[1] * 48.),
        ((a[0] + a[1]) * 48., (b[0] + b[1]) * 48.),
    ] {
        if (end - start).abs() < 1e-14 {
            continue;
        }
        let (left, right) = (start + (end - start) * lo, start + (end - start) * hi);
        for grid in left.min(right).ceil() as i32..=left.max(right).floor() as i32 {
            let t = (f64::from(grid) - start) / (end - start);
            if t > lo && t < hi {
                cuts.push(t);
            }
        }
    }
}

pub fn segments(path: &Path3, tiles: &[Tile]) -> Vec<[[f64; 3]; 2]> {
    let mut output = Vec::new();
    let stride = path.points.len().div_ceil(4096).max(1);
    let mut previous = None;
    for (index, point) in path.points.iter().enumerate() {
        let valid = |p: &[f64; 3]| p[..2].iter().all(|v| v.is_finite());
        if !valid(point) {
            previous = None;
            continue;
        }
        let endpoint = previous.is_none()
            || index + 1 == path.points.len()
            || path.points.get(index + 1).is_some_and(|p| !valid(p));
        if index % stride != 0 && !endpoint {
            continue;
        }
        let b = [point[0], point[1]];
        if let Some(a) = previous {
            let mut edges = vec![0., 1.];
            for tile in tiles {
                cuts(tile, a, b, &mut edges);
            }
            edges.sort_unstable_by(f64::total_cmp);
            edges.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
            let point = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            for edge in edges.windows(2) {
                let middle = point((edge[0] + edge[1]) / 2.);
                if let Some(tile) = tiles
                    .iter()
                    .find(|tile| tile.mesh_height_at(middle).is_some())
                {
                    let (a, b) = (point(edge[0]), point(edge[1]));
                    if let (Some(za), Some(zb)) = (tile.mesh_height_at(a), tile.mesh_height_at(b)) {
                        output.push([[a[0], a[1], f64::from(za)], [b[0], b[1], f64::from(zb)]]);
                    }
                }
            }
        }
        previous = Some(b);
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn sparse_ground_path_follows_ridge_without_cutting_through_triangles() {
        let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
        let paths = super::super::flight3::prepare(
            &tracks,
            &lines,
            &std::collections::BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        let mut path = paths[0].clone();
        let id = super::super::camera::TileId {
            z: 14,
            x: 8192,
            y: 8192,
        };
        let span = WORLD / f64::from(1_u32 << id.z);
        path.points = Arc::from([
            [span * 0.2, -span * 0.4, 0.],
            [span * 0.8, -span * 0.4, f64::NAN],
        ]);
        let tile = Tile {
            id,
            dem_id: id,
            raster_id: id,
            provider: super::super::tiles::Provider::OpenStreetMap,
            raster: None,
            elevation: Some(Arc::new(super::super::tiles::Payload::Elevation(
                super::super::tiles::Elevation {
                    version: 1,
                    width: 49,
                    height: 49,
                    heights: (0..49 * 49)
                        .map(|i| if i % 49 == 24 { 1000. } else { 0. })
                        .collect(),
                },
            ))),
        };
        let segments = segments(&path, std::slice::from_ref(&tile));
        assert!(
            segments.len() > 20,
            "Sparse endpoints must be split at mesh crossings"
        );
        assert!(segments.iter().flatten().any(|point| point[2] > 900.));
        for pair in &segments {
            let middle = [
                (pair[0][0] + pair[1][0]) / 2.,
                (pair[0][1] + pair[1][1]) / 2.,
            ];
            let height = f64::from(tile.mesh_height_at(middle).unwrap());
            assert!(
                ((pair[0][2] + pair[1][2]) / 2. - height).abs() < 0.01,
                "Ground line must follow each terrain triangle"
            );
        }
        assert!(segments.windows(2).all(|pair| pair[0][1] == pair[1][0]));
    }
}
