//! Local, camera-relative terrain coordinates and orbit projection.
use super::{
    camera::{Camera, TileId, WORLD, geographic},
    tiles::{Key, Payload, Provider, Service},
};
use std::{collections::BTreeMap, sync::Arc};
use zgui::prelude::{Get, GetUntracked};

#[derive(Clone)]
pub struct Tile {
    pub id: TileId,
    /// Elevation source may be coarser than imagery geometry.
    pub dem_id: TileId,
    pub raster_id: TileId,
    pub provider: Provider,
    pub raster: Option<Arc<Payload>>,
    pub elevation: Option<Arc<Payload>>,
}
#[derive(Default, Clone)]
pub struct Scene {
    pub tiles: Vec<Tile>,
    pub revision: u64,
    pub paths: Arc<[super::flight3::Path3]>,
    pub time: Option<f64>,
    pub colors: Arc<super::telemetry::Coloring>,
    pub ground_projection: bool,
    path_bases: std::collections::BTreeMap<(u64, u64, u64), f32>,
    anchor: Option<Tile>,
    base: f32,
    anchor_zoom: f64,
}

/// `id` and its ancestors, coarsest first.
fn ancestors(id: TileId) -> impl DoubleEndedIterator<Item = TileId> {
    (0..=id.z).map(move |z| {
        let shift = id.z - z;
        TileId {
            z,
            x: id.x.div_euclid(1_i64 << shift),
            y: id.y >> shift,
        }
    })
}
/// Strict descendants of `id`, by ordered range per zoom instead of a full scan.
fn descendants<T>(map: &BTreeMap<TileId, T>, id: TileId) -> impl Iterator<Item = &T> {
    (id.z + 1..=super::tiles::MAX_TILE_ZOOM).flat_map(move |z| {
        let shift = z - id.z;
        let x = id.x * (1_i64 << shift);
        map.range(
            TileId { z, x, y: 0 }..TileId {
                z,
                x: x + (1_i64 << shift),
                y: 0,
            },
        )
        .filter(move |(tile, _)| tile.y >> shift == id.y)
        .map(|(_, value)| value)
    })
}
/// Finest ready tile covering `id` (itself included) that has elevation.
fn height_source(ready: &BTreeMap<TileId, Tile>, id: TileId) -> Option<Tile> {
    ancestors(id)
        .filter_map(|id| ready.get(&id))
        .filter(|tile| tile.elevation.is_some())
        .max_by_key(|tile| Key::elevation(tile.dem_id).z)
        .cloned()
}
/// `height_source` for many tiles at once: siblings share every ancestor, so
/// each ancestor's best source is looked up once. `memo` stays valid while
/// `ready` gains no ancestors of later queries.
fn height_source_memo(
    ready: &BTreeMap<TileId, Tile>,
    id: TileId,
    memo: &mut std::collections::HashMap<TileId, Option<TileId>>,
) -> Option<TileId> {
    if let Some(&best) = memo.get(&id) {
        return best;
    }
    let above = (id.z > 0).then(|| {
        height_source_memo(
            ready,
            TileId {
                z: id.z - 1,
                x: id.x.div_euclid(2),
                y: id.y / 2,
            },
            memo,
        )
    });
    let dem = |id: TileId| Key::elevation(ready[&id].dem_id).z;
    let own = ready
        .get(&id)
        .is_some_and(|tile| tile.elevation.is_some())
        .then_some(id);
    // Finer tiles win ties, as in `height_source`.
    let best = match (own, above.flatten()) {
        (Some(own), Some(above)) => Some(if dem(own) >= dem(above) { own } else { above }),
        (own, above) => own.or(above),
    };
    memo.insert(id, best);
    best
}
/// Finest complete ready tile covering `id`, itself included.
fn complete_ancestor(ready: &BTreeMap<TileId, Tile>, id: TileId) -> Option<TileId> {
    ancestors(id)
        .rev()
        .find(|id| ready.get(id).is_some_and(Tile::complete))
}
pub fn contains(parent: TileId, child: TileId) -> bool {
    parent.z <= child.z
        && child.x.div_euclid(1_i64 << (child.z - parent.z)) == parent.x
        && child.y >> (child.z - parent.z) == parent.y
}
impl Tile {
    pub(crate) fn mesh_height_at(&self, point: [f64; 2]) -> Option<f32> {
        let span = WORLD / f64::from(1_u32 << self.id.z);
        let u = (point[0] + WORLD / 2.) / span - self.id.x as f64;
        let v = (WORLD / 2. - point[1]) / span - f64::from(self.id.y);
        if !(-1e-8..=1. + 1e-8).contains(&u) || !(-1e-8..=1. + 1e-8).contains(&v) {
            return None;
        }
        let x = (u * 48.).clamp(0., 48. - 1e-9);
        let y = (v * 48.).clamp(0., 48. - 1e-9);
        let sample = |dx: f64, dy: f64| {
            self.height_at(Camera {
                center: [
                    -WORLD / 2. + (self.id.x as f64 + (x.floor() + dx) / 48.) * span,
                    WORLD / 2. - (f64::from(self.id.y) + (y.floor() + dy) / 48.) * span,
                ],
                zoom: 0.,
            })
        };
        let a = sample(0., 0.)?;
        let b = sample(1., 0.)?;
        let c = sample(0., 1.)?;
        let d = sample(1., 1.)?;
        let (fx, fy) = (x.fract() as f32, y.fract() as f32);
        Some(if fx + fy <= 1. {
            a + (b - a) * fx + (c - a) * fy
        } else {
            d + (c - d) * (1. - fx) + (b - d) * (1. - fy)
        })
    }
    fn complete(&self) -> bool {
        self.raster.is_some() && self.elevation.is_some()
    }
    pub(crate) fn height_at(&self, camera: Camera) -> Option<f32> {
        let tile_span = WORLD / f64::from(1_u32 << self.id.z);
        let tile_u = (camera.center[0] + WORLD / 2.) / tile_span - self.id.x as f64;
        let tile_v = (WORLD / 2. - camera.center[1]) / tile_span - f64::from(self.id.y);
        if !(-1e-8..=1. + 1e-8).contains(&tile_u) || !(-1e-8..=1. + 1e-8).contains(&tile_v) {
            return None;
        }
        let key = Key::elevation(self.dem_id);
        // Match terrain.wgsl's tile-local DEM UV transform. Recomputing global
        // coordinates rejects roundoff at left/top edges and wraps root u=1 to 0.
        let ratio = f64::from(1_u32 << (self.id.z - key.z));
        let u = (f64::from(self.id.canonical_x()).rem_euclid(ratio) + tile_u.clamp(0., 1.)) / ratio;
        let v = (f64::from(self.id.y).rem_euclid(ratio) + tile_v.clamp(0., 1.)) / ratio;
        let Payload::Elevation(e) = self.elevation.as_ref()?.as_ref() else {
            return None;
        };
        let x = (u * f64::from(e.width) - 0.5).clamp(0., f64::from(e.width - 1));
        let y = (v * f64::from(e.height) - 0.5).clamp(0., f64::from(e.height - 1));
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(e.width - 1);
        let y1 = (y0 + 1).min(e.height - 1);
        let sample = |x, y| e.heights[(y * e.width + x) as usize];
        let mix = |a: f32, b: f32, t: f64| a + (b - a) * t as f32;
        Some(mix(
            mix(sample(x0, y0), sample(x1, y0), x.fract()),
            mix(sample(x0, y1), sample(x1, y1), x.fract()),
            y.fract(),
        ))
    }
}
impl Scene {
    #[allow(clippy::too_many_arguments)] // View and selection inputs.
    pub fn resolve(
        service: Service,
        ids: &[TileId],
        raster_ids: &[TileId],
        previous: &Self,
        camera: Camera,
        orbit: Orbit,
        size: [f64; 2],
        provider: Provider,
    ) -> Self {
        // One subscription covers every tile read below.
        service.revision.get();
        let mut ready = previous
            .tiles
            .iter()
            .filter(|t| t.complete() && t.provider == provider)
            .map(|t| (t.id, t.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        for &id in ids {
            let mut parent = id;
            loop {
                let tile = Tile {
                    id: parent,
                    dem_id: parent,
                    raster_id: parent,
                    provider,
                    raster: service.payload_untracked(Key::raster_from(parent, provider)),
                    elevation: service.payload_untracked(Key::elevation(parent)),
                };
                if tile.complete() {
                    ready.insert(parent, tile);
                    break;
                }
                if parent.z == 0 {
                    break;
                }
                parent = TileId {
                    z: parent.z - 1,
                    x: parent.x.div_euclid(2),
                    y: parent.y / 2,
                };
            }
        }
        // Raster tiles never contain one another, so inserting them below
        // keeps the memo's ancestor answers valid.
        let mut memo = std::collections::HashMap::new();
        let rasters =
            service.payloads_untracked(raster_ids.iter().map(|&id| Key::raster_from(id, provider)));
        for (&raster_id, raster) in raster_ids.iter().zip(rasters) {
            let Some(raster) = raster else {
                continue;
            };
            let Some(height_source) =
                height_source_memo(&ready, raster_id, &mut memo).map(|id| ready[&id].clone())
            else {
                continue;
            };
            ready.entry(raster_id).or_insert(Tile {
                id: raster_id,
                dem_id: height_source.dem_id,
                raster_id,
                provider,
                raster: Some(raster),
                elevation: height_source.elevation,
            });
        }
        for &id in ids {
            let Some(height_source) = height_source(&ready, id) else {
                continue;
            };
            for child in service.cached_raster_children(id, provider) {
                ready.entry(child).or_insert_with(|| Tile {
                    id: child,
                    dem_id: height_source.dem_id,
                    raster_id: child,
                    provider,
                    raster: service.payload_untracked(Key::raster_from(child, provider)),
                    elevation: height_source.elevation.clone(),
                });
            }
        }
        let tiles = Self::handover(ids, &ready, previous, provider, camera, orbit, size);
        // Keep the same height source across LOD changes. Bilinear sampling of
        // that source avoids both whole-scene jumps and nearest-pixel steps.
        let mut scene = Self::assembled(tiles, previous, camera, service.revision.get_untracked());
        // Focus height must not remain locked to a world-view fallback DEM
        // after fitting a distant flight. Use one fixed-resolution source.
        let id = super::flight3::anchor_tile(camera.center);
        if let Some(elevation) = service.payload_untracked(Key::elevation(id)) {
            let anchor = Tile {
                id,
                dem_id: id,
                raster_id: id,
                provider,
                raster: None,
                elevation: Some(elevation),
            };
            if let Some(height) = anchor.height_at(camera) {
                scene.base = height;
                scene.anchor = Some(anchor);
                scene.anchor_zoom = camera.zoom;
            }
        }
        scene
    }
    fn assembled(tiles: Vec<Tile>, previous: &Self, camera: Camera, revision: u64) -> Self {
        let preferred = tiles
            .iter()
            .filter(|t| t.height_at(camera).is_some())
            .max_by_key(|t| Key::elevation(t.dem_id).z);
        let old = previous
            .anchor
            .as_ref()
            .filter(|t| t.height_at(camera).is_some());
        let upgrade = old.zip(preferred).is_some_and(|(old, new)| {
            camera.zoom > previous.anchor_zoom + 0.5
                && Key::elevation(new.dem_id).z > Key::elevation(old.dem_id).z
        });
        let anchor = if upgrade {
            preferred.cloned()
        } else {
            old.or(preferred).cloned()
        };
        let anchor_zoom = if old.is_some() && !upgrade {
            previous.anchor_zoom
        } else {
            camera.zoom
        };
        let base = anchor
            .as_ref()
            .and_then(|t| t.height_at(camera))
            .unwrap_or(previous.base);
        Self {
            tiles,
            revision,
            anchor,
            base,
            anchor_zoom,
            paths: previous.paths.clone(),
            time: previous.time,
            colors: previous.colors.clone(),
            ground_projection: previous.ground_projection,
            path_bases: previous.path_bases.clone(),
        }
    }
    #[allow(clippy::too_many_arguments)] // View and selection inputs.
    pub(super) fn handover(
        ids: &[TileId],
        ready: &std::collections::BTreeMap<TileId, Tile>,
        previous: &Self,
        _provider: Provider,
        camera: Camera,
        orbit: Orbit,
        size: [f64; 2],
    ) -> Vec<Tile> {
        let mut selected = std::collections::BTreeSet::new();
        for &id in ids {
            // Keep detailed old leaves when they still cover this coarser
            // selection. A cached parent must not replace sharp imagery just
            // because orbit angle or view budget merged the terrain LOD.
            let frontier = descendants(ready, id)
                .filter(|t| t.provider == _provider && t.complete())
                .collect::<Vec<_>>();
            if !frontier.is_empty() {
                if ready.get(&id).is_some_and(Tile::complete) {
                    selected.insert(id);
                } else if let Some(ancestor) = complete_ancestor(ready, id) {
                    selected.insert(ancestor);
                }
                selected.extend(frontier.iter().map(|t| t.id));
                continue;
            }
            if ready.get(&id).is_some_and(|t| t.complete()) {
                selected.insert(id);
                continue;
            }
            let old_children = previous
                .tiles
                .iter()
                .filter(|t| contains(id, t.id) && t.complete())
                .map(|t| t.id)
                .collect::<Vec<_>>();
            if !old_children.is_empty()
                && !previous
                    .tiles
                    .iter()
                    .any(|t| contains(t.id, id) && t.complete())
            {
                selected.extend(
                    ready
                        .get(&id)
                        .into_iter()
                        .chain(descendants(ready, id))
                        .filter(|t| t.complete())
                        .map(|t| t.id),
                );
                continue;
            }
            if let Some(ancestor) = complete_ancestor(ready, id) {
                selected.insert(ancestor);
            } else {
                selected.extend(
                    ready
                        .get(&id)
                        .into_iter()
                        .chain(descendants(ready, id))
                        .filter(|t| t.complete())
                        .map(|t| t.id),
                );
            }
        }
        let candidates = selected
            .iter()
            .filter_map(|id| ready.get(id).cloned())
            .collect::<Vec<_>>();
        Self::partition(candidates, camera, orbit, size)
    }
    fn partition(candidates: Vec<Tile>, camera: Camera, orbit: Orbit, size: [f64; 2]) -> Vec<Tile> {
        let mut split = std::collections::BTreeSet::new();
        for tile in &candidates {
            // Strict ancestors; once one is known, so are all above it.
            for ancestor in ancestors(tile.id).rev().skip(1) {
                if !split.insert(ancestor) {
                    break;
                }
            }
        }
        let ids = candidates
            .iter()
            .map(|tile| tile.id)
            .collect::<std::collections::BTreeSet<_>>();
        // Leaves are candidates without a candidate ancestor.
        let mut leaves = candidates
            .iter()
            .filter(|tile| !ancestors(tile.id).rev().skip(1).any(|id| ids.contains(&id)))
            .cloned()
            .collect::<Vec<_>>();
        let sources = candidates
            .iter()
            .map(|tile| (tile.id, tile))
            .collect::<std::collections::BTreeMap<_, _>>();
        // Split largest on screen first, so a full budget drops only the
        // smallest detail; the heap holds exactly the splittable leaves.
        let pose = orbit.pose(camera, size);
        let scale = ground_scale(camera);
        let key = |id: TileId| {
            let pixels = pose.projected_size(camera, id, size[1], scale);
            let (edge, center) = camera.center_key(id);
            (
                pixels.to_bits(),
                std::cmp::Reverse((edge.to_bits(), center.to_bits(), id)),
            )
        };
        let mut splittable = leaves
            .iter()
            .filter(|leaf| split.contains(&leaf.id))
            .map(|tile| key(tile.id))
            .collect::<std::collections::BinaryHeap<_>>();
        while let Some((_, std::cmp::Reverse((_, _, id)))) = splittable.pop() {
            if leaves.len() + 3 > MAX_RENDER_TILES {
                break;
            }
            let index = leaves.iter().position(|tile| tile.id == id).unwrap();
            let parent = leaves.swap_remove(index);
            for y in 0..2 {
                for x in 0..2 {
                    let region = TileId {
                        z: id.z + 1,
                        x: id.x * 2 + x,
                        y: id.y * 2 + y,
                    };
                    let mut tile = parent.clone();
                    tile.id = region;
                    if let Some(finer) = sources.get(&region) {
                        if finer.raster_id.z >= tile.raster_id.z {
                            tile.raster_id = finer.raster_id;
                            tile.raster = finer.raster.clone();
                        }
                        if Key::elevation(finer.dem_id).z >= Key::elevation(tile.dem_id).z {
                            tile.dem_id = finer.dem_id;
                            tile.elevation = finer.elevation.clone();
                        }
                    }
                    leaves.push(tile);
                    if split.contains(&region) {
                        splittable.push(key(region));
                    }
                }
            }
        }
        leaves
    }
    pub fn with_paths(mut self, service: Service, paths: Arc<[super::flight3::Path3]>) -> Self {
        self.path_bases.retain(|key, _| {
            paths
                .iter()
                .any(|path| (path.id, path.origin[0].to_bits(), path.origin[1].to_bits()) == *key)
        });
        for path in paths.iter().filter(|path| path.local) {
            let key = (path.id, path.origin[0].to_bits(), path.origin[1].to_bits());
            if self.path_bases.contains_key(&key) {
                continue;
            }
            let id = super::flight3::anchor_tile(path.origin);
            let tile = Tile {
                id,
                dem_id: id,
                raster_id: id,
                provider: Provider::OpenStreetMap,
                raster: None,
                elevation: service.payload(Key::elevation(id)),
            };
            if let Some(base) = tile.height_at(Camera {
                center: path.origin,
                zoom: 14.,
            }) {
                self.path_bases.insert(key, base);
            }
        }
        self.paths = paths;
        self
    }
    pub fn path_base(&self, path: &super::flight3::Path3) -> Option<f32> {
        if path.local {
            self.path_bases
                .get(&(path.id, path.origin[0].to_bits(), path.origin[1].to_bits()))
                .copied()
        } else {
            Some(0.)
        }
    }
    pub fn center_height(&self, camera: Camera) -> f32 {
        self.anchor
            .as_ref()
            .and_then(|t| t.height_at(camera))
            .unwrap_or(self.base)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Orbit {
    pub bearing: f32,
    pub pitch: f32,
    pub exaggeration: f32,
    pub view_distance_km: f32,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            bearing: -25.,
            pitch: 45.,
            exaggeration: 1.,
            view_distance_km: 80.,
        }
    }
}
pub fn ground_scale(camera: Camera) -> f64 {
    geographic(camera.center)[1].to_radians().cos()
}

pub const MAX_TERRAIN_TILES: usize = 128;
pub const MAX_RENDER_TILES: usize = 256;
const SPLIT_PIXELS: f64 = 320.;
const MERGE_PIXELS: f64 = 160.;
pub const MIN_PITCH: f32 = 2.;

impl Orbit {
    pub fn dragged(self, delta: [f64; 2]) -> Self {
        Self {
            bearing: self.bearing - delta[0] as f32 * 0.3,
            pitch: (self.pitch + delta[1] as f32 * 0.25).clamp(MIN_PITCH, 85.),
            ..self
        }
    }

    pub fn visibility_metres(self) -> f64 {
        f64::from(self.view_distance_km.clamp(40., 160.)) * 1000.
    }
    /// Ground shown past the focus. Up close it is the view distance; once
    /// the orbit is farther than 4 km it scales with that distance (20x at
    /// the default 80 km), keeping terrain to within ~1/20 of the pitch below
    /// the horizon instead of leaving an empty band at regional zooms.
    fn reach(self, distance: f64) -> f64 {
        self.visibility_metres() * (distance / 4000.).max(1.)
    }

    fn pose(self, camera: Camera, size: [f64; 2]) -> Pose {
        // Zoom sets a world-space orbit distance. Resizing changes aspect only;
        // 720px is the reference viewport height for the shared 2D zoom scale.
        let tan_y = (45_f64.to_radians() / 2.).tan();
        let distance = (720. / (2. * tan_y * camera.scale()) * ground_scale(camera)).max(1.);
        let pitch = f64::from(self.pitch).to_radians();
        let angle = f64::from(self.bearing).to_radians();
        let eye = [
            angle.sin() * pitch.cos() * distance,
            pitch.sin() * distance,
            angle.cos() * pitch.cos() * distance,
        ];
        let forward = normalize(eye.map(|v| -v));
        let right = normalize(cross(forward, [0., 1., 0.]));
        let up = cross(right, forward);
        let top_down = pitch.sin() - pitch.cos() * tan_y;
        let reach = self.reach(distance);
        // Ground footprint of the top screen edge. It grows without bound as
        // the horizon nears, so cap it at the visible reach: far then changes
        // continuously instead of collapsing once the horizon enters view.
        let footprint = if top_down > 0. {
            (1.1 * distance * pitch.cos() * tan_y / top_down).min(reach)
        } else {
            reach
        };
        Pose {
            eye,
            forward,
            right,
            up,
            near: (distance / 1000.).max(0.01),
            // Cover the viewport's ground footprint when dollying out. A fixed
            // distance beyond focus clips the upper screen at regional zooms.
            far: distance + self.visibility_metres().max(footprint),
            reach,
            distance,
            tan_x: tan_y * size[0].max(1.) / size[1].max(1.),
            tan_y,
        }
    }

    pub fn matrix(self, camera: Camera, size: [f64; 2]) -> [f32; 16] {
        let Pose {
            eye,
            forward,
            right,
            up,
            near,
            far,
            tan_x,
            tan_y,
            ..
        } = self.pose(camera, size);
        let view = [
            right[0],
            up[0],
            -forward[0],
            0.,
            right[1],
            up[1],
            -forward[1],
            0.,
            right[2],
            up[2],
            -forward[2],
            0.,
            -dot(right, eye),
            -dot(up, eye),
            dot(forward, eye),
            1.,
        ]
        .map(|v| v as f32);
        let projection = [
            1. / tan_x,
            0.,
            0.,
            0.,
            0.,
            1. / tan_y,
            0.,
            0.,
            0.,
            0.,
            near / (far - near),
            -1.,
            0.,
            0.,
            near * far / (far - near),
            0.,
        ]
        .map(|v| v as f32);
        multiply(projection, view)
    }

    /// Intersect the cursor ray with the focus-height plane. Rays above the
    /// horizon (or beyond the view range) have no usable ground anchor.
    pub fn ground_at(self, camera: Camera, size: [f64; 2], point: [f64; 2]) -> Option<[f64; 2]> {
        Camera::geometry(size)?;
        let pose = self.pose(camera, size);
        let x = (2. * point[0] / size[0] - 1.) * pose.tan_x;
        let y = (1. - 2. * point[1] / size[1]) * pose.tan_y;
        let ray = std::array::from_fn::<_, 3, _>(|i| {
            pose.forward[i] + pose.right[i] * x + pose.up[i] * y
        });
        if ray[1] >= -1e-9 {
            return None;
        }
        let depth = -pose.eye[1] / ray[1];
        if depth < pose.near || depth > pose.far {
            return None;
        }
        let scale = ground_scale(camera);
        Some([
            camera.center[0] + (pose.eye[0] + ray[0] * depth) / scale,
            camera.center[1] - (pose.eye[2] + ray[2] * depth) / scale,
        ])
    }

    pub fn pan(
        self,
        camera: Camera,
        size: [f64; 2],
        start: [f64; 2],
        current: [f64; 2],
    ) -> Option<Camera> {
        let a = self.ground_at(camera, size, start)?;
        let b = self.ground_at(camera, size, current)?;
        Some(Camera {
            center: [
                (camera.center[0] + a[0] - b[0] + WORLD / 2.).rem_euclid(WORLD) - WORLD / 2.,
                (camera.center[1] + a[1] - b[1]).clamp(-WORLD / 2., WORLD / 2.),
            ],
            ..camera
        })
    }

    #[cfg(test)]
    pub fn tiles(self, camera: Camera, size: [f64; 2]) -> Vec<TileId> {
        Selector::default().select(self, camera, size)
    }
    /// View-space depths where terrain starts and finishes fading into the
    /// background: the last 40% of reach, so the far cutoff reads as haze.
    pub fn fog(self, camera: Camera, size: [f64; 2]) -> [f64; 2] {
        let pose = self.pose(camera, size);
        [pose.distance + 0.6 * pose.reach, pose.distance + pose.reach]
    }
    pub fn projected_tile_size(self, camera: Camera, size: [f64; 2], tile: TileId) -> f64 {
        self.pose(camera, size)
            .projected_size(camera, tile, size[1], ground_scale(camera))
    }
    /// Largest on screen first: coarse fallbacks, then detail nearest the eye.
    /// Distance from the orbit focus would rank near-eye and horizon tiles alike.
    pub fn screen_first(self, camera: Camera, size: [f64; 2], tiles: &mut [TileId]) {
        let pose = self.pose(camera, size);
        let scale = ground_scale(camera);
        // One sort; ties fall back to `Camera::center_first` order.
        tiles.sort_by_cached_key(|&tile| {
            let (edge, center) = camera.center_key(tile);
            (
                std::cmp::Reverse(pose.projected_size(camera, tile, size[1], scale).to_bits()),
                edge.to_bits(),
                center.to_bits(),
                tile,
            )
        });
    }
}

/// Split order is by on-screen size, so a full budget always drops the
/// smallest (farthest) detail. Remembering last frame's splits gives a
/// split/merge deadband without making the result depend on older history.
#[derive(Default)]
pub struct Selector {
    expanded: std::collections::BTreeSet<TileId>,
}
impl Selector {
    pub fn select(&mut self, orbit: Orbit, camera: Camera, size: [f64; 2]) -> Vec<TileId> {
        if Camera::geometry(size).is_none() {
            return Vec::new();
        }
        let pose = orbit.pose(camera, size);
        let scale = ground_scale(camera);
        let max_zoom = (camera.zoom.ceil() as u8)
            .saturating_add(1)
            .min(super::tiles::MAX_TILE_ZOOM);
        let visible = |id| pose.visible(camera, id, scale, 6000. * f64::from(orbit.exaggeration));
        let score = |id| pose.projected_size(camera, id, size[1], scale);
        let children = |id: TileId| {
            (0..2).flat_map(move |y| {
                (0..2).map(move |x| TileId {
                    z: id.z + 1,
                    x: id.x * 2 + x,
                    y: id.y * 2 + y,
                })
            })
        };
        let mut leaves = (-1..=1)
            .map(|x| TileId { z: 0, x, y: 0 })
            .filter(|&id| visible(id))
            .map(|id| (id, score(id)))
            .collect::<Vec<_>>();
        let mut expanded = std::collections::BTreeSet::new();
        // Largest on screen first; a split that would exceed the budget is
        // dropped for this frame, as smaller tiles may still fit.
        let splittable = |id: TileId, pixels: f64| {
            let threshold = if self.expanded.contains(&id) {
                MERGE_PIXELS
            } else {
                SPLIT_PIXELS
            };
            id.z < max_zoom && pixels > threshold
        };
        let mut queue = leaves
            .iter()
            .filter(|&&(id, pixels)| splittable(id, pixels))
            .map(|&(id, pixels)| (pixels.to_bits(), id))
            .collect::<std::collections::BinaryHeap<_>>();
        while let Some((_, id)) = queue.pop() {
            let kids = children(id).filter(|&id| visible(id)).collect::<Vec<_>>();
            if leaves.len() - 1 + kids.len() > MAX_TERRAIN_TILES {
                continue;
            }
            let index = leaves.iter().position(|leaf| leaf.0 == id).unwrap();
            leaves.swap_remove(index);
            for kid in kids {
                let pixels = score(kid);
                leaves.push((kid, pixels));
                if splittable(kid, pixels) {
                    queue.push((pixels.to_bits(), kid));
                }
            }
            expanded.insert(id);
        }
        self.expanded = expanded;
        let mut leaves = leaves.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        camera.center_first(&mut leaves);
        leaves
    }
}

struct Pose {
    reach: f64,
    distance: f64,
    eye: [f64; 3],
    forward: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    near: f64,
    far: f64,
    tan_x: f64,
    tan_y: f64,
}
impl Pose {
    fn tile_box(camera: Camera, id: TileId, scale: f64) -> ([f64; 3], f64) {
        let span = WORLD / f64::from(1_u32 << id.z);
        (
            [
                (-WORLD / 2. + (id.x as f64 + 0.5) * span - camera.center[0]) * scale,
                0.,
                (camera.center[1] - WORLD / 2. + (f64::from(id.y) + 0.5) * span) * scale,
            ],
            span * scale / 2.,
        )
    }
    fn visible(&self, camera: Camera, id: TileId, scale: f64, height: f64) -> bool {
        let (center, half) = Self::tile_box(camera, id, scale);
        let relative = std::array::from_fn(|i| center[i] - self.eye[i]);
        let plane = |normal: [f64; 3], offset: f64| {
            dot(normal, relative)
                + half * (normal[0].abs() + normal[2].abs())
                + height * normal[1].abs()
                >= offset
        };
        plane(self.forward, self.near)
            && plane(self.forward.map(|v| -v), -self.far)
            && [-1., 1.].into_iter().all(|sign| {
                plane(
                    std::array::from_fn(|i| self.forward[i] * self.tan_x + sign * self.right[i]),
                    0.,
                ) && plane(
                    std::array::from_fn(|i| self.forward[i] * self.tan_y + sign * self.up[i]),
                    0.,
                )
            })
    }
    fn projected_size(&self, camera: Camera, id: TileId, height: f64, scale: f64) -> f64 {
        // Detail follows the look target (orbit focus), not the eye: at the
        // focus this equals the true eye distance, and it grows radially.
        let (center, half) = Self::tile_box(camera, id, scale);
        let dx = (center[0].abs() - half).max(0.);
        let dz = (center[2].abs() - half).max(0.);
        let distance = (dx * dx + dz * dz + dot(self.eye, self.eye)).sqrt().max(1.);
        2. * half * height / (2. * self.tan_y * distance)
    }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    v.map(|v| v / length)
}
fn multiply(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    out
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn partition_clips_fallback_and_selects_raster_and_dem_independently() {
        let root = TileId { z: 0, x: 0, y: 0 };
        let child = TileId { z: 1, x: 0, y: 0 };
        let detail = TileId { z: 2, x: 0, y: 0 };
        let base = tile(root, vec![0.; 4]);
        let mut height = tile(child, vec![10.; 4]);
        height.raster_id = root;
        let mut imagery = tile(detail, vec![0.; 4]);
        imagery.dem_id = root;
        let leaves = Scene::partition(
            vec![base, height, imagery],
            Camera::default(),
            Orbit::default(),
            [1000., 700.],
        );
        assert_eq!(leaves.len(), 7);
        for (index, a) in leaves.iter().enumerate() {
            for b in &leaves[index + 1..] {
                assert!(!contains(a.id, b.id) && !contains(b.id, a.id));
            }
        }
        let best = leaves.iter().find(|tile| tile.id == detail).unwrap();
        assert_eq!(best.raster_id, detail);
        assert_eq!(best.dem_id, child);
        assert_eq!(
            leaves
                .iter()
                .map(|tile| 1_u64 << (2 * (2 - tile.id.z)))
                .sum::<u64>(),
            16
        );
    }
    #[test]
    fn local_altitude_origin_is_fixed_while_global_altitude_is_absolute() {
        let origin = [0., 0.];
        let path = super::super::flight3::Path3 {
            id: 1,
            origin,
            points: Arc::from([[0., 0., 0.], [100., 100., 0.]]),
            times: Arc::from([0., 1.]),
            color: [250, 40, 180, 255],
            width: 2.,
            drawing: Default::default(),
            radius: 3.,
            marker: plot::plot_data::MarkerShape::Circle,
            local: true,
            arrows: false,
            modes: std::sync::Arc::from([]),
        };
        let mut scene = Scene {
            paths: Arc::from([path.clone()]),
            ..Default::default()
        };
        scene
            .path_bases
            .insert((1, 0_f64.to_bits(), 0_f64.to_bits()), 1200.);
        let camera = Camera {
            center: origin,
            zoom: 12.,
        };
        let vertices =
            super::super::flight3::vertices(&scene, camera, Orbit::default(), [800., 600.]);
        assert_eq!(vertices[0][1], 1200.5);
        assert_eq!(vertices[1][1], 1200.5);
        let upgraded = Scene::assembled(
            vec![tile(TileId { z: 0, x: 0, y: 0 }, vec![2500.; 4])],
            &scene,
            camera,
            1,
        );
        assert_eq!(upgraded.path_base(&path), Some(1200.));
        let vertices =
            super::super::flight3::vertices(&upgraded, camera, Orbit::default(), [800., 600.]);
        assert!((vertices[0][1] + upgraded.center_height(camera) - 1200.5).abs() < 0.001);
        let mut global = path;
        global.local = false;
        global.points = Arc::from([[0., 0., 1800.], [100., 100., 1900.]]);
        scene.paths = Arc::from([global]);
        let vertices =
            super::super::flight3::vertices(&scene, camera, Orbit::default(), [800., 600.]);
        assert_eq!(vertices[0][1], 1800.5);
        assert_eq!(vertices[1][1], 1900.5);
    }
    fn tile(id: TileId, heights: Vec<f32>) -> Tile {
        Tile {
            id,
            dem_id: id,
            raster_id: id,
            provider: Provider::OpenStreetMap,
            raster: Some(Arc::new(Payload::Raster(super::super::tiles::Raster {
                version: 1,
                encoded: vec![],
                rgba: vec![255; 16],
                width: 2,
                height: 2,
            }))),
            elevation: Some(Arc::new(Payload::Elevation(
                super::super::tiles::Elevation {
                    version: 1,
                    heights,
                    width: 2,
                    height: 2,
                },
            ))),
        }
    }
    #[test]
    fn refinement_waits_for_all_children_and_merge_waits_for_parent() {
        let parent = TileId { z: 0, x: 0, y: 0 };
        let ids = (0..2)
            .flat_map(|y| (0..2).map(move |x| TileId { z: 1, x, y }))
            .collect::<Vec<_>>();
        let old = Scene {
            tiles: vec![tile(parent, vec![0.; 4])],
            ..Scene::default()
        };
        let mut ready = BTreeMap::from([(parent, old.tiles[0].clone())]);
        for &id in &ids[..3] {
            ready.insert(id, tile(id, vec![1.; 4]));
        }
        let mixed = Scene::handover(
            &ids,
            &ready,
            &old,
            Provider::OpenStreetMap,
            Camera::default(),
            Orbit::default(),
            [1000., 700.],
        );
        assert_eq!(mixed.len(), 4);
        assert!(mixed.iter().any(|tile| tile.raster_id == parent));
        assert!(mixed.iter().all(|tile| tile.id.z == parent.z + 1));
        ready.insert(ids[3], tile(ids[3], vec![1.; 4]));
        let refined = Scene {
            tiles: Scene::handover(
                &ids,
                &ready,
                &old,
                Provider::OpenStreetMap,
                Camera::default(),
                Orbit::default(),
                [1000., 700.],
            ),
            ..Scene::default()
        };
        assert_eq!(refined.tiles.len(), 4);
        ready.remove(&parent);
        let partial = Scene {
            tiles: vec![refined.tiles[0].clone()],
            ..Scene::default()
        };
        assert_eq!(
            Scene::handover(
                &[parent],
                &ready,
                &partial,
                Provider::OpenStreetMap,
                Camera::default(),
                Orbit::default(),
                [1000., 700.]
            )
            .len(),
            4,
            "Fill cached siblings around the previous frame's partial coverage"
        );
        assert_eq!(
            Scene::handover(
                &[parent],
                &ready,
                &Scene::default(),
                Provider::OpenStreetMap,
                Camera::default(),
                Orbit::default(),
                [1000., 700.]
            )
            .len(),
            4,
            "Cached children cover revisits even after leaving the previous scene"
        );
        assert_eq!(
            Scene::handover(
                &[parent],
                &ready,
                &refined,
                Provider::OpenStreetMap,
                Camera::default(),
                Orbit::default(),
                [1000., 700.]
            )
            .len(),
            4
        );
        ready.insert(parent, old.tiles[0].clone());
        assert_eq!(
            Scene::handover(
                &[parent],
                &ready,
                &refined,
                Provider::OpenStreetMap,
                Camera::default(),
                Orbit::default(),
                [1000., 700.]
            )
            .len(),
            4
        );
    }
    #[test]
    fn bilinear_focus_height_survives_lod_replacement() {
        let camera = Camera {
            center: [0., 0.],
            zoom: 2.,
        };
        let old = Scene::assembled(
            vec![tile(TileId { z: 0, x: 0, y: 0 }, vec![0., 10., 20., 30.])],
            &Scene::default(),
            camera,
            1,
        );
        assert_eq!(old.center_height(camera), 15.);
        let next = Scene::assembled(
            vec![tile(TileId { z: 1, x: 1, y: 1 }, vec![200.; 4])],
            &old,
            camera,
            2,
        );
        assert_eq!(
            next.center_height(camera),
            15.,
            "LOD alone must not move the entire world"
        );
    }
    #[test]
    fn retained_selection_has_deadband_and_stays_bounded() {
        let camera = Camera {
            center: super::super::camera::project(-111.7, 40.2).unwrap(),
            zoom: 12.,
        };
        let size = [1000., 700.];
        let mut selector = Selector::default();
        let orbit = Orbit {
            view_distance_km: 160.,
            ..Orbit::default()
        };
        let original = selector.select(orbit, camera, size);
        let repeated = selector.select(orbit, camera, size);
        assert_eq!(original, repeated);
        let mut last = original
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let mut retained_changes = 0;
        let mut fresh_changes = 0;
        let mut fresh_last = last.clone();
        for step in (0..60).chain((0..60).rev()) {
            let pose = Orbit {
                bearing: orbit.bearing + step as f32 * 0.1,
                ..orbit
            };
            let ids = selector
                .select(pose, camera, size)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            assert!(ids.len() <= MAX_TERRAIN_TILES);
            retained_changes += ids.symmetric_difference(&last).count();
            last = ids;
            let fresh = pose
                .tiles(camera, size)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            fresh_changes += fresh.symmetric_difference(&fresh_last).count();
            fresh_last = fresh;
        }
        eprintln!(
            "LOD movement trace: retained={retained_changes}, stateless={fresh_changes} tile changes"
        );
        assert!(retained_changes <= fresh_changes);
        // Sweeping the orbit must not leave history-made coarse patches.
        let camera = Camera {
            center: [-12517719.28, 4965521.78],
            zoom: 17.45,
        };
        let size = [1600., 900.];
        for step in 0..48 {
            let pose = Orbit {
                bearing: step as f32 * 7.5,
                pitch: 30.,
                ..orbit
            };
            let coarsest = |ids: Vec<TileId>| ids.into_iter().map(|id| id.z).min().unwrap();
            let retained = coarsest(selector.select(pose, camera, size));
            let fresh = coarsest(pose.tiles(camera, size));
            assert!(
                retained + 1 >= fresh,
                "step {step}: z{retained} vs fresh z{fresh}"
            );
        }
        let wide = selector.select(orbit, Camera { zoom: 0., ..camera }, size);
        assert!(wide.iter().all(|id| id.z <= 1));
    }
}

#[cfg(test)]
mod camera_invariant_tests {
    use super::*;
    #[test]
    fn resize_keeps_camera_position_and_zoom_keeps_ground_in_view() {
        let camera = Camera {
            center: super::super::camera::project(7.6586, 45.9763).unwrap(),
            zoom: 12.,
        };
        let orbit = Orbit::default();
        let a = orbit.pose(camera, [800., 600.]);
        for size in [[400., 300.], [1200., 900.], [1600., 400.]] {
            let b = orbit.pose(camera, size);
            assert_eq!(a.eye, b.eye);
            assert_eq!(a.far, b.far);
            assert_eq!(a.tan_y, b.tan_y);
            let center = orbit
                .ground_at(camera, size, [size[0] / 2., size[1] / 2.])
                .unwrap();
            assert!(
                center
                    .iter()
                    .zip(camera.center)
                    .all(|(a, b)| (a - b).abs() < 1e-7)
            );
        }
        for zoom in [8., 12., 16., 19.] {
            let p = orbit.pose(Camera { zoom, ..camera }, [800., 600.]);
            let distance = dot(p.eye, p.eye).sqrt();
            assert!(p.far - distance >= orbit.visibility_metres() - 1e-6);
            if zoom >= 12. {
                assert!((p.far - distance - orbit.visibility_metres()).abs() < 1e-6);
            }
        }
        // Lowering the view past the angle where the horizon enters (22.5
        // degrees with a 45 degree FOV) must not collapse far into a wall.
        for zoom in [9., 12., 15., 18.] {
            let camera = Camera { zoom, ..camera };
            let far = |pitch| Orbit { pitch, ..orbit }.pose(camera, [800., 600.]).far;
            let (above, below) = (far(22.6), far(22.4));
            assert!(below >= above * 0.99, "zoom {zoom}: far {above} -> {below}");
        }
        for zoom in [3., 5., 8., 12., 19.] {
            let camera = Camera { zoom, ..camera };
            for pitch in [25., 30., 45., 85.] {
                let orbit = Orbit { pitch, ..orbit };
                for point in [[0., 0.], [800., 0.], [400., 300.], [0., 600.]] {
                    assert!(
                        orbit.ground_at(camera, [800., 600.], point).is_some(),
                        "Ground below horizon must survive far clipping at zoom {zoom}, pitch {pitch}, pixel {point:?}"
                    );
                }
            }
        }
    }
}
