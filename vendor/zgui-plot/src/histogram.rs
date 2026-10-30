//! Equal-width histogram binning and bar geometry.

use kurbo::BezPath;

use crate::{PlotGeometry, Range, append_rect, append_top_rounded_rect};

/// One histogram bin. Bins cover `start..end`; the last bin also includes `end`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bin {
    pub start: f64,
    pub end: f64,
    pub count: usize,
}

impl Bin {
    /// Bin covering `start..end` holding `count` values.
    pub const fn new(start: f64, end: f64, count: usize) -> Self {
        Self { start, end, count }
    }
}

/// Auto count axis fits tallest bin, with nonzero range for empty data.
pub fn scale(bins: &[Bin]) -> Range {
    Range::new(
        0.0,
        bins.iter().map(|bin| bin.count).max().unwrap_or(0).max(1) as f64,
    )
}

/// Empty bins covering `range`, or none for an invalid range or zero count.
pub fn empty_bins(count: usize, range: impl Into<Range>) -> Vec<Bin> {
    let Range { min, max } = range.into();
    if count == 0 || !min.is_finite() || !max.is_finite() || min >= max {
        return Vec::new();
    }
    let width = (max - min) / count as f64;
    (0..count)
        .map(|index| {
            let end = if index + 1 == count {
                max
            } else {
                min + (index + 1) as f64 * width
            };
            Bin::new(min + index as f64 * width, end, 0)
        })
        .collect()
}

/// Count `values` into `count` equal bins over `range`; values outside are ignored.
pub fn bins_in_range(
    values: impl IntoIterator<Item = f64>,
    count: usize,
    range: impl Into<Range>,
) -> Vec<Bin> {
    let range = range.into();
    let mut bins = empty_bins(count, range);
    if bins.is_empty() {
        return bins;
    }
    let width = range.span() / count as f64;
    for value in values {
        if range.contains(value) {
            bins[bin_index(value, range.min, width, count)].count += 1;
        }
    }
    bins
}

/// Same result as [`bins_in_range`] for ascending `sorted` values, found by
/// binary search so boundary rounding matches the linear scan exactly.
pub fn bins_sorted(sorted: &[f64], count: usize, range: impl Into<Range>) -> Vec<Bin> {
    let range = range.into();
    let mut bins = empty_bins(count, range);
    if bins.is_empty() {
        return bins;
    }
    let width = range.span() / count as f64;
    let start = sorted.partition_point(|value| *value < range.min);
    let end = sorted.partition_point(|value| *value <= range.max);
    let visible = &sorted[start..end];
    let mut previous = 0;
    for (index, bin) in bins.iter_mut().enumerate() {
        let next = if index + 1 == count {
            visible.len()
        } else {
            visible.partition_point(|value| bin_index(*value, range.min, width, count) <= index)
        };
        bin.count = next - previous;
        previous = next;
    }
    bins
}

/// Bins spanning the data extent. Constant data gets a unit-wide domain
/// centred on the value (egui_plot behaviour) and keeps the requested count.
pub fn auto_bins(values: &[f64], count: usize) -> Vec<Bin> {
    if values.is_empty() || count == 0 {
        return Vec::new();
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = if min == max {
        (min - 0.5, max + 0.5)
    } else {
        (min, max)
    };
    bins_in_range(values.iter().copied(), count, range)
}

/// Bin under a pointer at `fraction` (0..=1) of the histogram width.
pub fn bin_at(count: usize, fraction: f64) -> Option<usize> {
    if count == 0 || !(0.0..=1.0).contains(&fraction) {
        return None;
    }
    Some(((fraction * count as f64).floor() as usize).min(count - 1))
}

/// Fractions of ascending `sorted` samples at or below, and at or above, `value`.
pub fn cumulative_fractions(sorted: &[f64], value: f64) -> (f64, f64) {
    if sorted.is_empty() {
        return (0.0, 0.0);
    }
    let n = sorted.len() as f64;
    (
        sorted.partition_point(|sample| *sample <= value) as f64 / n,
        (sorted.len() - sorted.partition_point(|sample| *sample < value)) as f64 / n,
    )
}

/// Horizontal screen extent of bar `index` of `count`. Bars keep a gap of up
/// to 2px; a lone bar is capped at 56px wide and centred.
pub fn bar_extent(index: usize, count: usize, geometry: PlotGeometry) -> (f64, f64) {
    let width = geometry.data_width() / count.max(1) as f64;
    let gap = (width - 1.0).clamp(0.0, 2.0);
    let bar_width = if count == 1 {
        width.min(56.0)
    } else {
        width - gap
    };
    let x = geometry.left + index as f64 * width + (width - bar_width) / 2.0;
    (x, x + bar_width)
}

/// Rounded-top bars for non-empty bins, scaled so `max` reaches the top.
/// With `selected`, only that bin is drawn.
pub fn bar_path(
    bins: &[Bin],
    geometry: PlotGeometry,
    max: f64,
    selected: Option<usize>,
) -> BezPath {
    bar_path_in_range(bins, geometry, Range::new(0.0, max), selected)
}

/// Bars clipped to full Y viewport, including a nonzero lower bound.
pub fn bar_path_in_range(
    bins: &[Bin],
    geometry: PlotGeometry,
    range: Range,
    selected: Option<usize>,
) -> BezPath {
    let mut path = BezPath::new();
    if bins.is_empty() || crate::range::valid(range).is_none() {
        return path;
    }
    for (index, bin) in bins.iter().enumerate() {
        if bin.count == 0 || selected.is_some_and(|selected| index != selected) {
            continue;
        }
        let (x0, x1) = bar_extent(index, bins.len(), geometry);
        let lower = 0.0_f64.clamp(range.min, range.max);
        let upper = (bin.count as f64).clamp(range.min, range.max);
        if upper <= lower {
            continue;
        }
        let y = geometry.y(upper, range);
        let bottom = geometry.y(lower, range);
        let radius = 2.0_f64.min((x1 - x0) / 2.0).min((bottom - y) / 2.0);
        append_top_rounded_rect(&mut path, x0, y, x1, bottom, radius);
    }
    path
}

/// Stacked bars, one path per group (bottom group first), for groups whose
/// bins share one layout. Segments above `max` are clipped; only the topmost
/// unclipped segment gets rounded corners. Groups with no visible segment are omitted.
pub fn stacked_bar_paths(
    groups: &[&[Bin]],
    geometry: PlotGeometry,
    max: f64,
) -> Vec<(usize, BezPath)> {
    stacked_bar_paths_in_range(groups, geometry, Range::new(0.0, max))
}

/// Stacked bars clipped to both ends of Y viewport.
pub fn stacked_bar_paths_in_range(
    groups: &[&[Bin]],
    geometry: PlotGeometry,
    range: Range,
) -> Vec<(usize, BezPath)> {
    if groups.is_empty() || crate::range::valid(range).is_none() {
        return Vec::new();
    }
    let bin_count = groups[0].len();
    let mut paths = vec![BezPath::new(); groups.len()];
    for bin in 0..bin_count {
        let (x0, x1) = bar_extent(bin, bin_count, geometry);
        let mut lower = 0usize;
        for (group, bins) in groups.iter().enumerate() {
            let count = bins[bin].count;
            if count == 0 {
                continue;
            }
            let upper = lower.saturating_add(count);
            let visible_lower = (lower as f64).clamp(range.min, range.max);
            let visible_upper = (upper as f64).clamp(range.min, range.max);
            lower = upper;
            if visible_lower >= visible_upper {
                continue;
            }
            let bottom = geometry.y(visible_lower, range);
            let top = geometry.y(visible_upper, range);
            let last = groups[group + 1..].iter().all(|bins| bins[bin].count == 0);
            // A clipped top is a viewport cut, not the rounded end of a bar.
            if last && upper as f64 <= range.max {
                let radius = 2.0_f64.min((x1 - x0) / 2.0).min((bottom - top) / 2.0);
                append_top_rounded_rect(&mut paths[group], x0, top, x1, bottom, radius);
            } else {
                append_rect(&mut paths[group], x0, top, x1, bottom);
            }
        }
    }
    paths
        .into_iter()
        .enumerate()
        .filter(|(_, path)| !path.elements().is_empty())
        .collect()
}

/// Empirical cumulative step curve of ascending `sorted` samples over the
/// visible `domain`, scaled 0..=100% of the full sample count. `reverse` draws
/// the complementary (at-or-above) curve.
pub fn cumulative_path(
    sorted: &[f64],
    domain: impl Into<Range>,
    geometry: PlotGeometry,
    reverse: bool,
) -> BezPath {
    let Range { min, max } = domain.into();
    let mut path = BezPath::new();
    if sorted.is_empty() || min >= max {
        return path;
    }
    let y = |count: usize| geometry.y(count as f64, (0.0, sorted.len() as f64));
    let count = |index: usize| if reverse { sorted.len() - index } else { index };
    let mut index = sorted.partition_point(|value| *value < min);
    path.move_to((geometry.x(min, (min, max)), y(count(index))));
    while index < sorted.len() && sorted[index] <= max {
        let value = sorted[index];
        let end = index + sorted[index..].partition_point(|sample| *sample == value);
        let x = geometry.x(value, (min, max));
        path.line_to((x, y(count(index))));
        path.line_to((x, y(count(end))));
        index = end;
    }
    path.line_to((geometry.right, y(count(index))));
    path
}

fn bin_index(value: f64, min: f64, width: f64, count: usize) -> usize {
    (((value - min) / width).floor() as usize).min(count - 1)
}

/// CDF/CCDF for sorted groups, reduced to one step per CSS column.
/// Group counts use binary search; source buffers remain independent.
pub fn cumulative_columns(
    groups: &[&[f64]],
    domain: Range,
    geometry: PlotGeometry,
    reverse: bool,
) -> BezPath {
    let mut path = BezPath::new();
    let total = groups.iter().map(|group| group.len()).sum::<usize>();
    if total == 0 || crate::range::valid(domain).is_none() {
        return path;
    }
    let columns = geometry.data_width().ceil().clamp(1.0, 8192.0) as usize;
    let y = |x: f64| {
        let below = groups
            .iter()
            .map(|group| {
                group.partition_point(|value| if reverse { *value < x } else { *value <= x })
            })
            .sum::<usize>();
        geometry.y(
            if reverse {
                (total - below) as f64 / total as f64
            } else {
                below as f64 / total as f64
            },
            (0.0, 1.0),
        )
    };
    let mut previous = y(domain.min);
    path.move_to((geometry.left, previous));
    for column in 1..=columns {
        let value = domain.min + domain.span() * column as f64 / columns as f64;
        let x = geometry.x(value, domain);
        let next = y(value);
        path.line_to((x, previous));
        path.line_to((x, next));
        previous = next;
    }
    path
}

#[cfg(test)]
mod tests {
    #[test]
    fn bars_clip_at_nonzero_lower_bound() {
        let geometry =
            crate::PlotGeometry::with_bounds(0.0, 0.0, 100.0, 100.0, 100.0, 100.0).unwrap();
        let range = crate::Range::new(2.0, 6.0);
        let groups = [
            vec![super::Bin::new(0.0, 1.0, 1)],
            vec![super::Bin::new(0.0, 1.0, 4)],
            vec![super::Bin::new(0.0, 1.0, 10)],
        ];
        let refs = groups.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let paths = super::stacked_bar_paths_in_range(&refs, geometry, range);
        assert_eq!(
            paths.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            vec![1, 2]
        );
        for (_, path) in paths {
            let bounds = kurbo::Shape::bounding_box(&path);
            assert!(bounds.y0 >= geometry.top && bounds.y1 <= geometry.bottom);
        }
        assert!(super::bar_path_in_range(&groups[0], geometry, range, None).is_empty());
        let bounds = kurbo::Shape::bounding_box(&super::bar_path_in_range(
            &groups[1], geometry, range, None,
        ));
        assert_eq!(bounds.y0, geometry.y(4.0, range));
        assert_eq!(bounds.y1, geometry.bottom);
    }

    use super::*;

    #[test]
    fn sorted_bins_match_scan_at_boundaries() {
        let values = [0.0, 0.1, 1.0 / 3.0, 0.5, 2.0 / 3.0, 0.9999, 1.0];
        for count in [1, 3, 7, 80] {
            for range in [(0.0, 1.0), (0.25, 0.75), (1.0 / 3.0, 2.0 / 3.0)] {
                assert_eq!(
                    bins_sorted(&values, count, range),
                    bins_in_range(values, count, range)
                );
            }
        }
    }

    #[test]
    fn auto_bins_count_every_value_including_upper_endpoint() {
        let bins = auto_bins(&[0.0, 5.0, 10.0], 2);
        assert_eq!(bins.iter().map(|bin| bin.count).collect::<Vec<_>>(), [1, 2]);
        let constant = auto_bins(&[2.0, 2.0], 80);
        assert_eq!((constant[0].start, constant[79].end), (1.5, 2.5));
        assert!(auto_bins(&[], 10).is_empty());
    }

    fn geometry() -> PlotGeometry {
        PlotGeometry::with_bounds(0.0, 0.0, 100.0, 100.0, 100.0, 120.0).unwrap()
    }

    #[test]
    fn bars_skip_empty_bins_and_can_isolate_one() {
        let bins = [
            Bin::new(0.0, 1.0, 2),
            Bin::new(1.0, 2.0, 0),
            Bin::new(2.0, 3.0, 4),
        ];
        let all = kurbo::Shape::bounding_box(&bar_path(&bins, geometry(), 4.0, None));
        assert_eq!((all.y0, all.y1), (0.0, 100.0));
        let one = kurbo::Shape::bounding_box(&bar_path(&bins, geometry(), 4.0, Some(0)));
        assert_eq!(one.y0, 50.0);
        assert!(one.x1 < 34.0);
    }

    #[test]
    fn stacked_bars_clip_above_max_and_omit_hidden_groups() {
        let low: &[Bin] = &[Bin::new(0.0, 1.0, 3)];
        let high: &[Bin] = &[Bin::new(0.0, 1.0, 5)];
        assert_eq!(stacked_bar_paths(&[low, high], geometry(), 10.0).len(), 2);
        let clipped = stacked_bar_paths(&[low, high], geometry(), 2.0);
        assert_eq!(clipped.len(), 1);
        assert_eq!(clipped[0].0, 0);
    }

    #[test]
    fn cumulative_path_spans_domain_with_full_denominator() {
        let path = cumulative_path(&[1.0, 2.0, 2.0, 3.0], (1.5, 2.5), geometry(), false);
        let bounds = kurbo::Shape::bounding_box(&path);
        assert_eq!((bounds.x0, bounds.x1), (0.0, 100.0));
        assert_eq!((bounds.y0, bounds.y1), (25.0, 75.0));
    }

    #[test]
    fn bin_at_includes_right_edge() {
        assert_eq!(bin_at(10, 0.0), Some(0));
        assert_eq!(bin_at(10, 1.0), Some(9));
        assert_eq!(bin_at(0, 0.5), None);
        assert_eq!(bin_at(10, -0.1), None);
    }

    #[test]
    fn cumulative_fractions_include_duplicates() {
        assert_eq!(
            cumulative_fractions(&[1.0, 2.0, 2.0, 3.0], 2.0),
            (0.75, 0.75)
        );
        assert_eq!(cumulative_fractions(&[1.0, 2.0], 0.0), (0.0, 1.0));
        assert_eq!(cumulative_fractions(&[], 2.0), (0.0, 0.0));
    }
}
