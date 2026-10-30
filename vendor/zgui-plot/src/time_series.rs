//! Prepared, X-sorted samples shared by native time plots and data adapters.
use crate::{PlotPoint, Range, Ranges};
use std::{collections::BTreeSet, sync::Arc};

/// Original f64 samples plus reusable origin-relative renderer vertices.
/// Nonfinite Y values break lines; X values must be finite and nondecreasing.
#[derive(Clone, Debug)]
pub struct SeriesData {
    samples: Arc<[PlotPoint]>,
    vertices: Arc<[[f32; 2]]>,
    origin: PlotPoint,
    bounds: Option<Ranges>,
}

impl SeriesData {
    pub fn new(samples: impl Into<Arc<[PlotPoint]>>) -> Result<Self, String> {
        let samples = samples.into();
        let origin = samples
            .iter()
            .find(|point| point.y.is_finite())
            .copied()
            .unwrap_or(PlotPoint {
                x: samples.first().map_or(0.0, |point| point.x),
                y: 0.0,
            });
        let mut previous = f64::NEG_INFINITY;
        let mut bounds: Option<Ranges> = None;
        let mut vertices = Vec::with_capacity(samples.len());
        for (index, point) in samples.iter().enumerate() {
            if !point.x.is_finite() || point.x < previous {
                return Err(format!(
                    "Sample {index}: X must be finite and nondecreasing"
                ));
            }
            previous = point.x;
            let x = (point.x - origin.x) as f32;
            let y = if point.y.is_finite() {
                (point.y - origin.y) as f32
            } else {
                f32::NAN
            };
            if !x.is_finite() || (point.y.is_finite() && !y.is_finite()) {
                return Err(format!(
                    "Sample {index}: origin-relative coordinates exceed f32 range"
                ));
            }
            vertices.push([x, y]);
            if point.y.is_finite() {
                let range =
                    bounds.get_or_insert(Ranges::new((point.x, point.x), (point.y, point.y)));
                range.x.min = range.x.min.min(point.x);
                range.x.max = range.x.max.max(point.x);
                range.y.min = range.y.min.min(point.y);
                range.y.max = range.y.max.max(point.y);
            }
        }
        Ok(Self {
            samples,
            vertices: vertices.into(),
            origin,
            bounds,
        })
    }

    pub fn samples(&self) -> &[PlotPoint] {
        &self.samples
    }
    pub fn vertices(&self) -> Arc<[[f32; 2]]> {
        Arc::clone(&self.vertices)
    }
    pub fn origin(&self) -> PlotPoint {
        self.origin
    }
    pub fn bounds(&self) -> Option<Ranges> {
        self.bounds
    }

    /// Visible nearest sample, preserving original index across line gaps.
    pub fn nearest(
        &self,
        target: PlotPoint,
        ranges: Ranges,
        pixels: (f64, f64),
        radius: f64,
    ) -> Option<usize> {
        crate::nearest_sorted(&self.samples, target, ranges, pixels, radius)
    }
}

/// Stable identity and display label. Labels may repeat; IDs must not.
#[derive(Clone, Debug)]
pub struct TimeSeries {
    id: String,
    label: String,
    data: SeriesData,
}
impl TimeSeries {
    pub fn new(id: impl Into<String>, label: impl Into<String>, data: SeriesData) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            data,
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn data(&self) -> &SeriesData {
        &self.data
    }
}

/// Immutable collection validates identity before keyed UI rows are mounted.
#[derive(Clone, Debug, Default)]
pub struct TimeSeriesSet(Arc<[TimeSeries]>);
impl TimeSeriesSet {
    pub fn new(series: Vec<TimeSeries>) -> Result<Self, String> {
        let mut ids = BTreeSet::new();
        for item in &series {
            if item.id.is_empty() || !ids.insert(item.id.as_str()) {
                return Err(format!(
                    "Series ID must be nonempty and unique: {:?}",
                    item.id
                ));
            }
        }
        Ok(Self(series.into()))
    }
    pub fn series(&self) -> &[TimeSeries] {
        &self.0
    }

    pub fn fitted_ranges(&self, hidden: &BTreeSet<String>) -> Ranges {
        let bounds = self
            .0
            .iter()
            .filter(|series| !hidden.contains(series.id()))
            .filter_map(|series| series.data.bounds())
            .reduce(|a, b| {
                Ranges::new(
                    (a.x.min.min(b.x.min), a.x.max.max(b.x.max)),
                    (a.y.min.min(b.y.min), a.y.max.max(b.y.max)),
                )
            });
        let Some(bounds) = bounds else {
            return Ranges::new((0.0, 1.0), (0.0, 1.0));
        };
        fn padded(range: Range, fraction: f64, flat: f64) -> Range {
            crate::range::valid(range.expand(fraction, flat)).unwrap_or(Range::new(0.0, 1.0))
        }
        Ranges::new(
            padded(
                bounds.x,
                0.01,
                (bounds.x.min.abs() * f64::EPSILON * 8.0).max(1.0),
            ),
            padded(bounds.y, 0.05, (bounds.y.min.abs() * 1e-6).max(1e-9)),
        )
    }
}
