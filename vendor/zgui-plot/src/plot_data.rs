//! Backend-neutral series, styling, annotations, and prepared scatter/histogram data.
use crate::{PlotPoint, Range, Ranges, time_series::TimeSeriesSet};
use std::{collections::BTreeSet, sync::Arc};

#[path = "plot_data/fit.rs"]
mod fit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerShape {
    Circle,
    Square,
    Triangle,
    Diamond,
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HistogramCurve {
    Cdf,
    Ccdf,
    NormalFit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesStyle {
    pub color: [u8; 4],
    pub width: f64,
    pub radius: f64,
    pub marker: MarkerShape,
}
impl SeriesStyle {
    pub fn for_id(id: &str) -> Self {
        let hash = id.bytes().fold(2166136261_u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16777619)
        });
        let palette = crate::palette::DEFAULT_PALETTE;
        Self {
            color: palette[hash as usize % palette.len()],
            width: 1.5,
            radius: 3.0,
            marker: MarkerShape::Circle,
        }
    }
    pub fn css_color(self) -> String {
        let [r, g, b, a] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[derive(Clone, Debug)]
pub struct ScatterSeries {
    id: String,
    label: String,
    samples: Arc<[PlotPoint]>,
    vertices: Arc<[[f32; 2]]>,
    origin: PlotPoint,
    bounds: Option<Ranges>,
    x_sorted: Arc<[usize]>,
}
impl ScatterSeries {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        samples: impl Into<Arc<[PlotPoint]>>,
    ) -> Result<Self, String> {
        let samples = samples.into();
        let origin = samples
            .iter()
            .find(|point| point.x.is_finite() && point.y.is_finite())
            .copied()
            .unwrap_or(PlotPoint { x: 0.0, y: 0.0 });
        let mut bounds: Option<Ranges> = None;
        let mut vertices = Vec::with_capacity(samples.len());
        for (index, point) in samples.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() {
                vertices.push([f32::NAN; 2]);
                continue;
            }
            let vertex = [(point.x - origin.x) as f32, (point.y - origin.y) as f32];
            if !vertex.iter().all(|value| value.is_finite()) {
                return Err(format!(
                    "Sample {index}: origin-relative coordinates exceed f32 range"
                ));
            }
            vertices.push(vertex);
            let extent = bounds.get_or_insert(Ranges::new((point.x, point.x), (point.y, point.y)));
            extent.x.min = extent.x.min.min(point.x);
            extent.x.max = extent.x.max.max(point.x);
            extent.y.min = extent.y.min.min(point.y);
            extent.y.max = extent.y.max.max(point.y);
        }
        let mut x_sorted = samples
            .iter()
            .enumerate()
            .filter(|(_, point)| point.x.is_finite() && point.y.is_finite())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        x_sorted.sort_by(|&a, &b| samples[a].x.total_cmp(&samples[b].x));
        Ok(Self {
            id: id.into(),
            label: label.into(),
            samples,
            vertices: vertices.into(),
            origin,
            bounds,
            x_sorted: x_sorted.into(),
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn samples(&self) -> &[PlotPoint] {
        &self.samples
    }
    pub fn vertices(&self) -> Arc<[[f32; 2]]> {
        self.vertices.clone()
    }
    pub fn origin(&self) -> PlotPoint {
        self.origin
    }
    /// X-window search over prepared index; result retains original sample identity.
    pub fn nearest(
        &self,
        target: PlotPoint,
        ranges: Ranges,
        pixels: (f64, f64),
        radius: f64,
    ) -> Option<usize> {
        if !(pixels.0 > 0.0 && pixels.1 > 0.0) {
            return None;
        }
        let reach = radius / pixels.0 * ranges.x.span();
        let start = self
            .x_sorted
            .partition_point(|&index| self.samples[index].x < target.x - reach);
        let end = start
            + self.x_sorted[start..]
                .partition_point(|&index| self.samples[index].x <= target.x + reach);
        self.x_sorted[start..end]
            .iter()
            .copied()
            .filter(|&index| {
                ranges.x.contains(self.samples[index].x) && ranges.y.contains(self.samples[index].y)
            })
            .map(|index| {
                let point = self.samples[index];
                (
                    index,
                    ((point.x - target.x) / ranges.x.span() * pixels.0).powi(2)
                        + ((point.y - target.y) / ranges.y.span() * pixels.1).powi(2),
                )
            })
            .filter(|(_, distance)| *distance <= radius * radius)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

fn validate_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.is_empty() || !seen.insert(id) {
            return Err(format!("Series ID must be nonempty and unique: {id:?}"));
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Default)]
pub struct ScatterSet(Arc<[ScatterSeries]>);
impl ScatterSet {
    pub fn new(series: Vec<ScatterSeries>) -> Result<Self, String> {
        validate_ids(series.iter().map(ScatterSeries::id))?;
        Ok(Self(series.into()))
    }
    pub fn series(&self) -> &[ScatterSeries] {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct HistogramSeries {
    id: String,
    label: String,
    sorted: Arc<[f64]>,
    mean: f64,
    m2: f64,
}
impl HistogramSeries {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        values: impl IntoIterator<Item = f64>,
    ) -> Self {
        let mut sorted = values
            .into_iter()
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let (mut mean, mut m2) = (0.0, 0.0);
        for (index, value) in sorted.iter().enumerate() {
            let delta = value - mean;
            mean += delta / (index + 1) as f64;
            m2 += delta * (value - mean);
        }
        Self {
            id: id.into(),
            label: label.into(),
            sorted: sorted.into(),
            mean,
            m2,
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn values(&self) -> &[f64] {
        &self.sorted
    }
}
#[derive(Clone, Debug, Default)]
pub struct HistogramSet(Arc<[HistogramSeries]>);
impl HistogramSet {
    /// Combined moments without copying or sorting selected groups.
    pub fn moments(&self, hidden: &BTreeSet<String>) -> (usize, f64, f64) {
        let (mut count, mut mean, mut m2) = (0_usize, 0.0, 0.0);
        for series in self
            .0
            .iter()
            .filter(|series| !hidden.contains(series.id()) && !series.sorted.is_empty())
        {
            let n = series.sorted.len();
            let next = count + n;
            let delta = series.mean - mean;
            m2 += series.m2 + delta * delta * count as f64 * n as f64 / next as f64;
            mean += delta * n as f64 / next as f64;
            count = next;
        }
        (
            count,
            mean,
            if count > 0 {
                (m2 / count as f64).sqrt()
            } else {
                0.0
            },
        )
    }
    pub fn new(series: Vec<HistogramSeries>) -> Result<Self, String> {
        validate_ids(series.iter().map(HistogramSeries::id))?;
        Ok(Self(series.into()))
    }
    pub fn series(&self) -> &[HistogramSeries] {
        &self.0
    }
    pub fn grouped_bins(
        &self,
        hidden: &BTreeSet<String>,
        count: usize,
        range: Range,
    ) -> Vec<(String, Vec<crate::Bin>)> {
        self.0
            .iter()
            .filter(|series| !hidden.contains(series.id()))
            .map(|series| {
                (
                    series.id.clone(),
                    crate::histogram::bins_sorted(&series.sorted, count.clamp(1, 4096), range),
                )
            })
            .collect()
    }
    pub fn visible_values(&self, hidden: &BTreeSet<String>) -> Vec<f64> {
        let mut values = self
            .0
            .iter()
            .filter(|series| !hidden.contains(series.id()))
            .flat_map(|series| series.values().iter().copied())
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        values
    }
}

#[derive(Clone, Debug)]
pub enum PlotData {
    Time(TimeSeriesSet),
    Scatter(ScatterSet),
    Histogram(HistogramSet),
}

#[derive(Clone, Debug)]
pub struct SeriesLabel {
    pub id: String,
    pub label: String,
}
impl SeriesLabel {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
}
impl PlotData {
    pub fn legend_marker(&self) -> crate::LegendMarker {
        if matches!(self, Self::Time(_)) {
            crate::LegendMarker::Line
        } else {
            crate::LegendMarker::Dot
        }
    }
    pub fn labels(&self) -> Vec<SeriesLabel> {
        match self {
            Self::Time(set) => set
                .series()
                .iter()
                .map(|series| SeriesLabel {
                    id: series.id().into(),
                    label: series.label().into(),
                })
                .collect(),
            Self::Scatter(set) => set
                .series()
                .iter()
                .map(|series| SeriesLabel {
                    id: series.id().into(),
                    label: series.label().into(),
                })
                .collect(),
            Self::Histogram(set) => set
                .series()
                .iter()
                .map(|series| SeriesLabel {
                    id: series.id().into(),
                    label: series.label().into(),
                })
                .collect(),
        }
    }
    pub fn fitted_ranges(&self, hidden: &BTreeSet<String>, bins: usize) -> Ranges {
        let extent = match self {
            Self::Time(set) => return set.fitted_ranges(hidden),
            Self::Scatter(set) => set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
                .filter_map(|series| series.bounds)
                .reduce(union),
            Self::Histogram(set) => {
                let extent = set
                    .series()
                    .iter()
                    .filter(|series| !hidden.contains(series.id()))
                    .filter_map(|series| {
                        series
                            .values()
                            .first()
                            .zip(series.values().last())
                            .map(|(&min, &max)| (min, max))
                    })
                    .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
                let Some((min, max)) = extent else {
                    return Ranges::new((0.0, 1.0), (0.0, 1.0));
                };
                let x = padded(Range::new(min, max));
                let groups = set.grouped_bins(hidden, bins, x);
                let max = (0..bins.clamp(1, 4096))
                    .map(|index| {
                        groups
                            .iter()
                            .map(|(_, bins)| bins[index].count)
                            .sum::<usize>()
                    })
                    .max()
                    .unwrap_or(0);
                return Ranges::new(x, (0.0, (max as f64).max(1.0)));
            }
        };
        extent.map_or(Ranges::new((0.0, 1.0), (0.0, 1.0)), |extent| {
            Ranges::new(padded(extent.x), padded(extent.y))
        })
    }
}
fn union(a: Ranges, b: Ranges) -> Ranges {
    Ranges::new(
        (a.x.min.min(b.x.min), a.x.max.max(b.x.max)),
        (a.y.min.min(b.y.min), a.y.max.max(b.y.max)),
    )
}
fn padded(range: Range) -> Range {
    crate::range::valid(range.expand(0.05, (range.min.abs() * 1e-6).max(0.5)))
        .unwrap_or(Range::new(0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnnotationGeometry {
    XThreshold(f64),
    YThreshold(f64),
    XBand(Range),
    YBand(Range),
    Region(Ranges),
    FitLine { slope: f64, intercept: f64 },
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlotAnnotation {
    pub label: String,
    pub geometry: AnnotationGeometry,
    pub color: [u8; 4],
    pub width: f64,
    pub dashed: bool,
}
