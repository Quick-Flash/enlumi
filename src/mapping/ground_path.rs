//! Drape each segment over terrain triangle edges so hills cannot hide its middle.
use super::{camera::WORLD, flight3::Path3, terrain::Tile};
pub type Key = Vec<(super::camera::TileId, super::camera::TileId, usize)>;
pub fn key(tiles: &[Tile]) -> Key {
    tiles
        .iter()
        .map(|tile| {
            (
                tile.id,
                tile.dem_id,
                tile.elevation
                    .as_ref()
                    .map_or(0, |elevation| std::sync::Arc::as_ptr(elevation) as usize),
            )
        })
        .collect()
}

pub(super) fn cuts(tile: &Tile, a: [f64; 2], b: [f64; 2], cuts: &mut Vec<f64>) {
    let span = WORLD / f64::from(1_u32 << tile.id.z);
    let local = |p: [f64; 2]| {
        [
            (p[0] + WORLD / 2.) / span - tile.id.x as f64,
            (WORLD / 2. - p[1]) / span - f64::from(tile.id.y),
        ]
        .map(|value| {
            if value.abs() < 1e-8 {
                0.
            } else if (value - 1.).abs() < 1e-8 {
                1.
            } else {
                value
            }
        })
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

#[cfg(test)]
pub fn segments(path: &Path3, tiles: &[Tile]) -> Vec<[[f64; 3]; 2]> {
    build(path, &super::ground_mesh::Sampler::new(tiles))
}
fn build(path: &Path3, sampler: &super::ground_mesh::Sampler) -> Vec<[[f64; 3]; 2]> {
    let mut output = Vec::with_capacity(path.points.len().saturating_sub(1));
    let mut edges = Vec::with_capacity(16);
    let mut previous = None;
    let mut last_end: Option<[f64; 3]> = None;
    for point in path.points.iter() {
        let valid = |p: &[f64; 3]| p[..2].iter().all(|v| v.is_finite());
        if !valid(point) {
            previous = None;
            last_end = None;
            continue;
        }
        let b = [point[0], point[1]];
        if let Some(a) = previous {
            edges.clear();
            edges.extend([0., 1.]);
            sampler.cuts(a, b, &mut edges);
            edges.sort_unstable_by(f64::total_cmp);
            edges.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
            let point = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            for edge in edges.windows(2) {
                let (a, b) = (point(edge[0]), point(edge[1]));
                if let Some([za, zb]) = sampler.heights(a, b) {
                    let segment = [[a[0], a[1], f64::from(za)], [b[0], b[1], f64::from(zb)]];
                    if let Some(end) = last_end
                        && (end[0] - a[0]).hypot(end[1] - a[1]) < 1e-6
                        && (end[2] - segment[0][2]).abs() > 1e-5
                    {
                        output.push([end, segment[0]]);
                    }
                    output.push(segment);
                    last_end = Some(segment[1]);
                } else {
                    last_end = None;
                }
            }
        }
        previous = Some(b);
    }
    output
}
pub type Segments = std::sync::Arc<[[[f64; 3]; 2]]>;
struct CachedPath {
    points: std::sync::Arc<[[f64; 3]]>,
    ground: Segments,
    bounds: ([f64; 2], [f64; 2]),
    terrain: Key,
}
fn region(key: &Key, (lo, hi): ([f64; 2], [f64; 2])) -> Key {
    key.iter()
        .filter(|(id, _, _)| {
            let span = WORLD / f64::from(1_u32 << id.z);
            let left = -WORLD / 2. + id.x as f64 * span;
            let top = WORLD / 2. - f64::from(id.y) * span;
            let pad = span * 1e-8;
            hi[0] >= left - pad
                && lo[0] <= left + span + pad
                && hi[1] >= top - span - pad
                && lo[1] <= top + pad
        })
        .copied()
        .collect()
}
#[derive(Default)]
pub struct Cache {
    pub builds: u64,
    terrain: Key,
    sampler: Option<super::ground_mesh::Sampler>,
    paths: std::collections::BTreeMap<u64, CachedPath>,
}
impl Cache {
    pub fn prepare(&mut self, paths: &[Path3], tiles: &[Tile]) {
        let key = key(tiles);
        if self.sampler.is_none() || self.terrain != key {
            self.terrain = key;
            self.sampler = Some(super::ground_mesh::Sampler::new(tiles));
        }
        self.paths
            .retain(|id, _| paths.iter().any(|path| path.id == *id));
    }
    pub fn get(&mut self, path: &Path3) -> Segments {
        if let Some(cached) = self.paths.get(&path.id)
            && std::sync::Arc::ptr_eq(&cached.points, &path.points)
            && cached.terrain == region(&self.terrain, cached.bounds)
        {
            return cached.ground.clone();
        }
        let ground: Segments = build(
            path,
            self.sampler
                .as_ref()
                .expect("Prepare ground cache before use"),
        )
        .into();
        self.builds += 1;
        let bounds = path
            .points
            .iter()
            .filter(|p| p[..2].iter().all(|v| v.is_finite()))
            .fold(
                ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
                |(mut lo, mut hi), p| {
                    for i in 0..2 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                    (lo, hi)
                },
            );
        self.paths.insert(
            path.id,
            CachedPath {
                points: path.points.clone(),
                ground: ground.clone(),
                bounds,
                terrain: region(&self.terrain, bounds),
            },
        );
        ground
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn path() -> Path3 {
        let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
        super::super::flight3::prepare(
            &tracks,
            &lines,
            &std::collections::BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
            &Default::default(),
            &Default::default(),
        )
        .unwrap()[0]
            .clone()
    }
    fn flat(id: super::super::camera::TileId, height: f32) -> Tile {
        Tile {
            id,
            dem_id: id,
            raster_id: id,
            provider: super::super::tiles::Provider::OpenStreetMap,
            raster: None,
            elevation: Some(Arc::new(super::super::tiles::Payload::Elevation(
                super::super::tiles::Elevation {
                    version: 1,
                    width: 2,
                    height: 2,
                    heights: vec![height; 4],
                },
            ))),
        }
    }
    #[test]
    #[ignore = "manual ground-path CPU profile"]
    fn profile_ground_path() {
        let mut path = path();
        let span = WORLD / 16384.;
        path.points = (0..20_000)
            .map(|i| {
                [
                    span * (7.2 + i as f64 / 20_000. * 0.6),
                    -span * (7.4 + if i == 10_000 { 0.2 } else { 0. }),
                    0.,
                ]
            })
            .collect::<Vec<_>>()
            .into();
        let tiles = (0..64)
            .map(|i| {
                flat(
                    super::super::camera::TileId {
                        z: 14,
                        x: 8192 + i % 8,
                        y: 8192 + (i / 8) as u32,
                    },
                    5.,
                )
            })
            .collect::<Vec<_>>();
        let start = std::time::Instant::now();
        let ground = segments(&path, &tiles);
        eprintln!(
            "20k samples / 64 terrain tiles: {:?}, {} ground segments",
            start.elapsed(),
            ground.len()
        );
        assert!(!ground.is_empty());
        let mut cache = Cache::default();
        cache.prepare(std::slice::from_ref(&path), &tiles);
        let start = std::time::Instant::now();
        let first = cache.get(&path);
        eprintln!("cached projection cold: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        for _ in 0..100 {
            cache.prepare(std::slice::from_ref(&path), &tiles);
            assert!(Arc::ptr_eq(&first, &cache.get(&path)));
        }
        eprintln!(
            "cached projection warm per frame: {:?}",
            start.elapsed() / 100
        );
    }
    #[test]
    fn ground_cache_reuses_style_changes_and_invalidates_elevation_updates() {
        let mut path = path();
        let tile = flat(super::super::camera::TileId { z: 0, x: 0, y: 0 }, 5.);
        let mut cache = Cache::default();
        cache.prepare(std::slice::from_ref(&path), std::slice::from_ref(&tile));
        let first = cache.get(&path);
        path.color = [100, 200, 30, 255];
        cache.prepare(std::slice::from_ref(&path), std::slice::from_ref(&tile));
        assert!(Arc::ptr_eq(&first, &cache.get(&path)));
        cache.prepare(std::slice::from_ref(&path), &[flat(tile.id, 20.)]);
        let updated = cache.get(&path);
        assert!(!Arc::ptr_eq(&first, &updated));
        assert!(updated.iter().flatten().all(|p| p[2] == 20.));
    }
    #[test]
    fn unrelated_terrain_detail_does_not_invalidate_route_projection() {
        let path = path();
        let tile = flat(
            super::super::camera::TileId {
                z: 14,
                x: 8192,
                y: 8192,
            },
            5.,
        );
        let mut cache = Cache::default();
        cache.prepare(std::slice::from_ref(&path), std::slice::from_ref(&tile));
        let first = cache.get(&path);
        let remote = flat(
            super::super::camera::TileId {
                z: 14,
                x: 8200,
                y: 8200,
            },
            20.,
        );
        cache.prepare(std::slice::from_ref(&path), &[tile, remote]);
        assert!(Arc::ptr_eq(&first, &cache.get(&path)));
        assert_eq!(cache.builds, 1);
    }
    #[test]
    fn cached_terrain_lod_and_elevation_changes_invalidate_ground_geometry() {
        let tile = flat(
            super::super::camera::TileId {
                z: 14,
                x: 8192,
                y: 8192,
            },
            5.,
        );
        let initial = key(std::slice::from_ref(&tile));
        let mut child = tile.clone();
        child.id = super::super::camera::TileId {
            z: 15,
            x: 16384,
            y: 16384,
        };
        assert_ne!(
            initial,
            key(&[child]),
            "Orbit can select cached finer mesh without download revision changing"
        );
        assert_ne!(initial, key(&[flat(tile.id, 20.)]));
    }
    #[test]
    fn terrain_sampling_accepts_exact_edges_across_many_tiles() {
        let span = WORLD / 16384.;
        for x in 0..16384 {
            let id = super::super::camera::TileId { z: 14, x, y: 6789 };
            let tile = flat(id, 5.);
            let point = [-WORLD / 2. + x as f64 * span, WORLD / 2. - 6789.5 * span];
            assert_eq!(
                tile.mesh_height_at(point),
                Some(5.),
                "Mesh left edge at x={x}"
            );
            assert_eq!(
                tile.height_at(super::super::camera::Camera {
                    center: point,
                    zoom: 14.
                }),
                Some(5.),
                "DEM left edge at x={x}"
            );
        }
    }
    #[test]
    fn root_dem_right_edge_does_not_wrap_to_left_edge() {
        let mut tile = flat(super::super::camera::TileId { z: 0, x: 0, y: 0 }, 0.);
        tile.elevation = Some(Arc::new(super::super::tiles::Payload::Elevation(
            super::super::tiles::Elevation {
                version: 1,
                width: 2,
                height: 2,
                heights: vec![0., 100., 0., 100.],
            },
        )));
        let height = tile
            .height_at(super::super::camera::Camera {
                center: [WORLD / 2., 0.],
                zoom: 0.,
            })
            .unwrap();
        assert_eq!(height, 100., "GPU samples rightmost DEM texel at u=1");
    }
    #[test]
    fn route_along_rounded_tile_edge_keeps_terrain_triangle_crossings() {
        let id = super::super::camera::TileId {
            z: 14,
            x: 2,
            y: 6789,
        };
        let span = WORLD / 16384.;
        let left = -WORLD / 2. + id.x as f64 * span;
        let top = WORLD / 2. - f64::from(id.y) * span;
        let mut tile = flat(id, 0.);
        tile.elevation = Some(Arc::new(super::super::tiles::Payload::Elevation(
            super::super::tiles::Elevation {
                version: 1,
                width: 49,
                height: 49,
                heights: (0..49 * 49)
                    .map(|i| if i / 49 == 24 { 1000. } else { 0. })
                    .collect(),
            },
        )));
        let mut path = path();
        path.points = Arc::from([[left, top - span * 0.2, 0.], [left, top - span * 0.8, 0.]]);
        let ground = segments(&path, &[tile]);
        assert!(
            ground.len() > 20,
            "Tile-edge route must still split at mesh rows"
        );
        assert!(
            ground.iter().flatten().any(|point| point[2] > 900.),
            "Ground route must follow ridge even along exact tile edge"
        );
    }
    #[test]
    fn dense_track_keeps_turn_between_old_stride_samples() {
        let mut path = path();
        path.points = (0..8200)
            .map(|i| [i as f64 * 0.01, if i == 17 { 20. } else { 0. }, 0.])
            .collect::<Vec<_>>()
            .into();
        let tile = flat(super::super::camera::TileId { z: 0, x: 0, y: 0 }, 5.);
        let ground = segments(&path, &[tile]);
        assert!(
            ground
                .iter()
                .flatten()
                .any(|p| (p[0] - 0.17).abs() < 1e-8 && (p[1] - 20.).abs() < 1e-8),
            "Ground path must retain each turn, including non-stride vertices"
        );
    }
    #[test]
    fn tile_height_step_has_explicit_join_and_missing_tiles_stay_gaps() {
        let mut path = path();
        let id = super::super::camera::TileId {
            z: 14,
            x: 8192,
            y: 8192,
        };
        let span = WORLD / f64::from(1_u32 << id.z);
        path.points = Arc::from([[span * 0.5, -span * 0.4, 0.], [span * 1.5, -span * 0.4, 0.]]);
        let tiles = [
            flat(id, 5.),
            flat(super::super::camera::TileId { x: id.x + 1, ..id }, 20.),
        ];
        let ground = segments(&path, &tiles);
        assert!(
            ground
                .iter()
                .any(|[a, b]| (a[0] - b[0]).abs() < 1e-6 && a[2] == 5. && b[2] == 20.),
            "Adjacent terrain heights need connecting segment"
        );
        for pair in ground.windows(2) {
            for (a, b) in pair[0][1].iter().zip(pair[1][0]) {
                assert!((a - b).abs() < 1e-5);
            }
        }
        assert!(
            segments(&path, &tiles[..1])
                .iter()
                .flatten()
                .all(|p| p[0] <= span + 1e-6)
        );
    }
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
