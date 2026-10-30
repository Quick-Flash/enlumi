use crate::spectral::Heatmap;
use kurbo::{BezPath, Rect, Shape as _};
use plot::Ranges;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zgui::canvas::{Brush, Shape, ShapeBuilder, zgui_color::Color};

/// Half-open cell bounds. Geometry lives in cell coordinates, independent of
/// viewport size and range, so pans/zooms can reuse its allocations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Tile {
    pub left: usize,
    pub right: usize,
    pub bottom: usize,
    pub top: usize,
}
impl Tile {
    pub fn visible(data: &Heatmap, ranges: Ranges) -> Option<Self> {
        if data.columns == 0
            || data.rows == 0
            || data.columns.checked_mul(data.rows)? != data.values.len()
            || ![
                data.x_low,
                data.x_high,
                data.y_low,
                data.y_high,
                ranges.x.min,
                ranges.x.max,
                ranges.y.min,
                ranges.y.max,
            ]
            .into_iter()
            .all(f64::is_finite)
            || data.x_high <= data.x_low
            || data.y_high <= data.y_low
        {
            return None;
        }
        let cell_x = |x: f64| (x - data.x_low) / (data.x_high - data.x_low) * data.columns as f64;
        let cell_y = |y: f64| (y - data.y_low) / (data.y_high - data.y_low) * data.rows as f64;
        let tile = Self {
            left: cell_x(ranges.x.min).floor().clamp(0., data.columns as f64) as usize,
            right: cell_x(ranges.x.max).ceil().clamp(0., data.columns as f64) as usize,
            bottom: cell_y(ranges.y.min).floor().clamp(0., data.rows as f64) as usize,
            top: cell_y(ranges.y.max).ceil().clamp(0., data.rows as f64) as usize,
        };
        (tile.left < tile.right && tile.bottom < tile.top).then_some(tile)
    }
    pub fn contains(self, other: Self) -> bool {
        self.left <= other.left
            && self.right >= other.right
            && self.bottom <= other.bottom
            && self.top >= other.top
    }
    pub fn padded(self, data: &Heatmap) -> Self {
        let mut x = (self.right - self.left).saturating_mul(3).div_ceil(4);
        let mut y = (self.top - self.bottom).saturating_mul(3).div_ceil(4);
        // Keep overscan from multiplying large heatmaps' path memory. Preserve
        // all visible cells; views above this budget get no additional cells.
        let budget = self.cells().max(262_144);
        loop {
            let padded = Self {
                left: self.left.saturating_sub(x),
                right: self.right.saturating_add(x).min(data.columns),
                bottom: self.bottom.saturating_sub(y),
                top: self.top.saturating_add(y).min(data.rows),
            };
            if padded.cells() <= budget {
                return padded;
            }
            x /= 2;
            y /= 2;
        }
    }
    pub fn cells(self) -> usize {
        (self.right - self.left).saturating_mul(self.top - self.bottom)
    }
}

pub(super) struct Prepared {
    pub tile: Tile,
    pub shapes: Arc<[Shape]>,
}

fn color(value: usize) -> Color {
    let t = value as f32 / 255.;
    Color::srgb(
        (1.5 * t - 0.5).clamp(0., 1.),
        (1.5 - (2. * t - 1.).abs() * 1.5).clamp(0., 1.),
        (1. - 1.5 * t).clamp(0., 1.),
        1.,
    )
}

/// Merge adjacent cells with the same color, preserving missing-cell gaps.
/// Check cancellation before allocation and throughout large rows.
pub(super) fn prepare(data: &Heatmap, tile: Tile, cancelled: &AtomicBool) -> Option<Prepared> {
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let mut paths = vec![BezPath::new(); 256];
    for row in tile.bottom..tile.top {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        let mut run = None::<(usize, usize)>;
        for column in tile.left..=tile.right {
            if column % 4096 == 0 && cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let level = (column < tile.right)
                .then(|| data.values[row * data.columns + column])
                .flatten()
                .map(|db| {
                    (((db - data.min_db) / (data.max_db - data.min_db).max(1e-12) * 255.) as usize)
                        .min(255)
                });
            if run.map(|(level, _)| level) != level {
                if let Some((level, start)) = run.take() {
                    paths[level].extend(
                        Rect::new(start as f64, row as f64, column as f64, (row + 1) as f64)
                            .to_path(0.1)
                            .elements()
                            .iter()
                            .copied(),
                    );
                }
                run = level.map(|level| (level, column));
            }
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let shapes = paths
        .into_iter()
        .enumerate()
        .filter(|(_, path)| !path.is_empty())
        .map(|(level, path)| {
            let color = color(level);
            ShapeBuilder::new(path)
                .fill(Brush::Linear {
                    start: (0., 0.).into(),
                    end: (1., 0.).into(),
                    stops: vec![(0., color), (1., color)],
                    repeating: false,
                })
                .build()
        })
        .collect::<Vec<_>>()
        .into();
    Some(Prepared { tile, shapes })
}
