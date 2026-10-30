//! Common histogram backdrop layers. Caller chooses logical thresholds/bands.
use crate::{GridOptions, PlotGeometry, Range, Ranges, grid_paths, overlay};
use kurbo::BezPath;
#[derive(Clone)]
pub struct HistogramLayers {
    pub grid: Vec<(BezPath, f64)>,
    pub axis: Option<BezPath>,
    pub fail_regions: Option<BezPath>,
    pub cdf: Option<BezPath>,
    pub ccdf: Option<BezPath>,
    pub limits: Option<BezPath>,
}
pub struct HistogramLayerOptions<'a> {
    pub sorted: &'a [f64],
    pub thresholds: &'a [f64],
    pub shaded: &'a [Range],
    pub domain: Range,
    pub y: Range,
    pub geometry: PlotGeometry,
    pub cdf: bool,
    pub ccdf: bool,
    pub limits: bool,
    pub bands: bool,
}
pub fn build(options: HistogramLayerOptions<'_>) -> HistogramLayers {
    let HistogramLayerOptions {
        sorted,
        thresholds,
        shaded,
        domain,
        y,
        geometry,
        cdf,
        ccdf,
        limits,
        bands,
    } = options;
    let regions = bands.then(|| {
        let mut path = BezPath::new();
        for range in shaded {
            crate::append_rect(
                &mut path,
                geometry.x(range.min, domain),
                geometry.top,
                geometry.x(range.max, domain),
                geometry.bottom,
            );
        }
        path
    });
    HistogramLayers {
        grid: grid_paths(
            Ranges::new(domain, y),
            geometry,
            GridOptions {
                y_min_step: 1.0,
                ..GridOptions::default()
            },
        ),
        axis: (cdf || ccdf)
            .then(|| overlay::right_axis(geometry, &[0.0, 0.25, 0.5, 0.75, 1.0], 4.0)),
        fail_regions: regions,
        cdf: cdf.then(|| crate::histogram::cumulative_path(sorted, domain, geometry, false)),
        ccdf: ccdf.then(|| crate::histogram::cumulative_path(sorted, domain, geometry, true)),
        limits: limits
            .then(|| overlay::vertical_value_lines(thresholds.iter().copied(), domain, geometry)),
    }
}
