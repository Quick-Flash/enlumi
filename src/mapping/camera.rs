use plot::{PlotGeometry, Ranges};

pub const WORLD: f64 = std::f64::consts::TAU * 6_378_137.;
pub const LIMIT: f64 = 85.0511287798066;
pub const TILE: f64 = 256.;
/// Camera overzoom is used by saved flight views; raster tile requests cap at z19.
pub const MAX_ZOOM: f64 = 28.;
/// Shared interaction limit for 2D and 3D; source imagery still caps at z19.
pub const MAX_INTERACTIVE_ZOOM: f64 = 22.;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub center: [f64; 2],
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: project(0., 0.).expect("Valid origin"),
            zoom: 2.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    pub z: u8,
    // Display X keeps world copies distinct; requests use canonical_x().
    pub x: i64,
    pub y: u32,
}

impl TileId {
    pub fn canonical_x(self) -> u32 {
        self.x.rem_euclid(1_i64 << self.z) as u32
    }
}

impl Camera {
    pub fn scale(self) -> f64 {
        TILE * self.zoom.exp2() / WORLD
    }

    pub fn ranges(self, size: [f64; 2]) -> Ranges {
        let [dx, dy] = size.map(|v| v / self.scale() / 2.);
        Ranges::new(
            (self.center[0] - dx, self.center[0] + dx),
            (self.center[1] - dy, self.center[1] + dy),
        )
    }

    pub fn from_ranges(ranges: Ranges, size: [f64; 2]) -> Option<Self> {
        Self::geometry(size)?;
        let scale = (size[0] / ranges.x.span()).min(size[1] / ranges.y.span());
        let center = [
            (ranges.x.min + ranges.x.max) / 2.,
            (ranges.y.min + ranges.y.max) / 2.,
        ];
        if !scale.is_finite() || scale <= 0. || !center.iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(Self {
            center: [
                (center[0] + WORLD / 2.).rem_euclid(WORLD) - WORLD / 2.,
                center[1].clamp(-WORLD / 2., WORLD / 2.),
            ],
            zoom: (scale * WORLD / TILE).log2().clamp(0., MAX_ZOOM),
        })
    }

    /// A pan changes the center, never the scale. Reconstructing zoom from
    /// translated range spans can round an integer just below its tile level.
    pub fn pan_to(self, ranges: Ranges, size: [f64; 2]) -> Option<Self> {
        Self::from_ranges(ranges, size).map(|next| Self {
            zoom: self.zoom,
            ..next
        })
    }

    pub fn geometry(size: [f64; 2]) -> Option<PlotGeometry> {
        if size.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return None;
        }
        PlotGeometry::with_bounds(0., 0., size[0], size[1], size[0], size[1])
    }

    pub fn tiles(self, size: [f64; 2]) -> Vec<TileId> {
        self.visible_tiles(size, 256, 22)
    }

    pub fn map_tiles(self, size: [f64; 2]) -> Vec<TileId> {
        let mut tiles = self.visible_tiles(size, 64, 19);
        self.center_first(&mut tiles);
        tiles
    }

    /// Coarser visible layers for temporary coverage while detailed tiles load.
    pub fn map_fallback_tiles(self, size: [f64; 2]) -> Vec<TileId> {
        let Some(level) = self.map_tiles(size).first().map(|tile| tile.z) else {
            return Vec::new();
        };
        let ranges = self.ranges(size);
        let mut tiles = Vec::new();
        for z in 0..level {
            let n = 1_i64 << z;
            let span = WORLD / n as f64;
            let x0 = ((ranges.x.min + WORLD / 2.) / span).floor() as i64;
            let x1 = ((ranges.x.max + WORLD / 2.) / span).ceil() as i64;
            let y0 = (((WORLD / 2. - ranges.y.max) / span).floor() as i64).clamp(0, n);
            let y1 = (((WORLD / 2. - ranges.y.min) / span).ceil() as i64).clamp(0, n);
            if x1.saturating_sub(x0).saturating_mul(y1 - y0) <= 64 {
                tiles.extend(
                    (y0..y1).flat_map(|y| (x0..x1).map(move |x| TileId { z, x, y: y as u32 })),
                );
            }
        }
        tiles
    }

    /// Squared distance from focus to tile edge, then to tile center.
    pub fn center_key(self, tile: TileId) -> (f64, f64) {
        let span = WORLD / f64::from(1_u32 << tile.z);
        let dx = -WORLD / 2. + (tile.x as f64 + 0.5) * span - self.center[0];
        let dy = WORLD / 2. - (f64::from(tile.y) + 0.5) * span - self.center[1];
        let edge_x = (dx.abs() - span / 2.).max(0.);
        let edge_y = (dy.abs() - span / 2.).max(0.);
        (edge_x * edge_x + edge_y * edge_y, dx * dx + dy * dy)
    }
    pub fn center_first(self, tiles: &mut [TileId]) {
        tiles.sort_by(|a, b| {
            let a_score = self.center_key(*a);
            let b_score = self.center_key(*b);
            a_score
                .0
                .total_cmp(&b_score.0)
                .then_with(|| a_score.1.total_cmp(&b_score.1))
                .then_with(|| a.cmp(b))
        });
    }

    fn visible_tiles(self, size: [f64; 2], limit: i64, max_zoom: u8) -> Vec<TileId> {
        if Self::geometry(size).is_none() {
            return Vec::new();
        }
        let ranges = self.ranges(size);
        let mut z = (self.zoom.floor() as u8).min(max_zoom);
        loop {
            let n = 1_i64 << z;
            let span = WORLD / n as f64;
            let x0 = ((ranges.x.min + WORLD / 2.) / span).floor() as i64;
            let x1 = ((ranges.x.max + WORLD / 2.) / span).ceil() as i64;
            let y0 = (((WORLD / 2. - ranges.y.max) / span).floor() as i64).clamp(0, n);
            let y1 = (((WORLD / 2. - ranges.y.min) / span).ceil() as i64).clamp(0, n);
            let count = x1.saturating_sub(x0).saturating_mul(y1 - y0);
            if count <= limit || z == 0 {
                let mut tiles = (y0..y1)
                    .flat_map(|y| (x0..x1).map(move |x| TileId { z, x, y: y as u32 }))
                    .collect::<Vec<_>>();
                if tiles.len() as i64 > limit {
                    self.center_first(&mut tiles);
                    tiles.truncate(limit as usize);
                }
                return tiles;
            }
            z -= 1;
        }
    }

    pub fn place(self, tile: TileId, size: [f64; 2]) -> [f64; 3] {
        let span = WORLD / (1_u64 << tile.z) as f64;
        let left = -WORLD / 2. + tile.x as f64 * span;
        let top = WORLD / 2. - f64::from(tile.y) * span;
        [
            (left - self.center[0]) * self.scale() + size[0] / 2.,
            (self.center[1] - top) * self.scale() + size[1] / 2.,
            span * self.scale() / TILE,
        ]
    }

    pub fn uniforms(self, size: [f64; 2], dpi: f32) -> [f32; 8] {
        let z = self.zoom.floor();
        let unit = WORLD / (TILE * z.exp2());
        let range = self.ranges(size);
        [
            ((range.x.min + WORLD / 2.) / unit).rem_euclid(TILE) as f32,
            ((WORLD / 2. - range.y.max) / unit).rem_euclid(TILE) as f32,
            (self.zoom - z).exp2() as f32,
            dpi,
            ((range.y.max - WORLD / 2.) * self.scale()) as f32,
            ((range.y.max + WORLD / 2.) * self.scale()) as f32,
            0.,
            0.,
        ]
    }
}

pub fn project(lon: f64, lat: f64) -> Option<[f64; 2]> {
    if !lon.is_finite() || !lat.is_finite() || !(-90. ..=90.).contains(&lat) {
        return None;
    }
    Some([
        WORLD * lon / 360.,
        WORLD / std::f64::consts::TAU * lat.clamp(-LIMIT, LIMIT).to_radians().tan().asinh(),
    ])
}

pub fn geographic([x, y]: [f64; 2]) -> [f64; 2] {
    [
        x / WORLD * 360.,
        (y / WORLD * std::f64::consts::TAU)
            .sinh()
            .atan()
            .to_degrees(),
    ]
}
