//! Lazy terrain grids and coherent tile lookup for dense ground tracks.
use super::{
    camera::{Camera, WORLD},
    terrain::Tile,
};
use std::{cell::Cell, sync::OnceLock};

struct Mesh {
    tile: Tile,
    span: f64,
    samples: OnceLock<Vec<f32>>,
}
impl Mesh {
    fn local(&self, p: [f64; 2]) -> [f64; 2] {
        [
            (p[0] + WORLD / 2.) / self.span - self.tile.id.x as f64,
            (WORLD / 2. - p[1]) / self.span - f64::from(self.tile.id.y),
        ]
    }
    fn contains(&self, p: [f64; 2]) -> bool {
        self.local(p)
            .iter()
            .all(|v| (-1e-8..=1. + 1e-8).contains(v))
    }
    fn height(&self, p: [f64; 2]) -> Option<f32> {
        let [u, v] = self.local(p);
        if !(-1e-8..=1. + 1e-8).contains(&u) || !(-1e-8..=1. + 1e-8).contains(&v) {
            return None;
        }
        let samples = self.samples.get_or_init(|| {
            (0..49 * 49)
                .map(|i| {
                    self.tile
                        .height_at(Camera {
                            center: [
                                -WORLD / 2.
                                    + (self.tile.id.x as f64 + (i % 49) as f64 / 48.) * self.span,
                                WORLD / 2.
                                    - (f64::from(self.tile.id.y) + (i / 49) as f64 / 48.)
                                        * self.span,
                            ],
                            zoom: 0.,
                        })
                        .unwrap_or(f32::NAN)
                })
                .collect()
        });
        let (x, y) = (
            (u * 48.).clamp(0., 48. - 1e-9),
            (v * 48.).clamp(0., 48. - 1e-9),
        );
        let index = y.floor() as usize * 49 + x.floor() as usize;
        let (a, b, c, d) = (
            samples[index],
            samples[index + 1],
            samples[index + 49],
            samples[index + 50],
        );
        let (fx, fy) = (x.fract() as f32, y.fract() as f32);
        let height = if fx + fy <= 1. {
            a + (b - a) * fx + (c - a) * fy
        } else {
            d + (c - d) * (1. - fx) + (b - d) * (1. - fy)
        };
        height.is_finite().then_some(height)
    }
}
pub struct Sampler {
    meshes: Vec<Mesh>,
    last: Cell<Option<usize>>,
}
impl Sampler {
    pub fn new(tiles: &[Tile]) -> Self {
        Self {
            meshes: tiles
                .iter()
                .filter(|tile| tile.elevation.is_some())
                .map(|tile| Mesh {
                    tile: tile.clone(),
                    span: WORLD / f64::from(1_u32 << tile.id.z),
                    samples: OnceLock::new(),
                })
                .collect(),
            last: Cell::new(None),
        }
    }
    fn tile(&self, p: [f64; 2]) -> Option<usize> {
        if let Some(index) = self.last.get()
            && self.meshes[index].height(p).is_some()
        {
            return Some(index);
        }
        let index = self.meshes.iter().position(|mesh| mesh.height(p).is_some());
        self.last.set(index);
        index
    }
    pub fn cuts(&self, a: [f64; 2], b: [f64; 2], edges: &mut Vec<f64>) {
        let middle = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
        if let Some(index) = self.tile(middle)
            && self.meshes[index].contains(a)
            && self.meshes[index].contains(b)
        {
            super::ground_path::cuts(&self.meshes[index].tile, a, b, edges);
            return;
        }
        for mesh in &self.meshes {
            super::ground_path::cuts(&mesh.tile, a, b, edges);
        }
    }
    pub fn heights(&self, a: [f64; 2], b: [f64; 2]) -> Option<[f32; 2]> {
        let index = self.tile([(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.])?;
        Some([self.meshes[index].height(a)?, self.meshes[index].height(b)?])
    }
}
