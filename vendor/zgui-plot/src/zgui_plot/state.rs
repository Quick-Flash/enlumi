use crate::plot_data::{HistogramCurve, PlotAnnotation, PlotData, SeriesStyle};
use crate::{PlotPoint, Ranges, interaction::RangeUpdate};
use std::collections::{BTreeMap, BTreeSet};
use zgui::{canvas::Lod, prelude::*};

/// Sample identity and full-precision data coordinates for consumer readouts.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesHit {
    pub series_id: String,
    pub sample_index: usize,
    pub point: PlotPoint,
}

/// Caller-owned state survives tab switches and can be independently duplicated.
#[derive(Clone, Copy)]
pub struct PlotState {
    pub ranges: RwSignal<Ranges, LocalStorage>,
    pub hidden: RwSignal<BTreeSet<String>, LocalStorage>,
    pub hover: RwSignal<Option<SeriesHit>, LocalStorage>,
    pub lod: RwSignal<Lod, LocalStorage>,
    /// Fit visible series on source/membership changes until viewport is manually changed.
    pub styles: RwSignal<BTreeMap<String, SeriesStyle>, LocalStorage>,
    pub emphasized: RwSignal<Option<String>, LocalStorage>,
    pub annotations: RwSignal<Vec<PlotAnnotation>, LocalStorage>,
    pub show_annotations: RwSignal<bool, LocalStorage>,
    pub histogram: RwSignal<HistogramOptions, LocalStorage>,
    pub bin_hover: RwSignal<Option<HistogramHit>, LocalStorage>,
    pub curve_hover: RwSignal<Option<HistogramCurve>, LocalStorage>,
    pub auto_fit: RwSignal<bool, LocalStorage>,
}
impl Default for PlotState {
    fn default() -> Self {
        Self {
            ranges: RwSignal::new_local(Ranges::new((0.0, 1.0), (0.0, 1.0))),
            hidden: RwSignal::new_local(BTreeSet::new()),
            hover: RwSignal::new_local(None),
            lod: RwSignal::new_local(Lod::Columns),
            styles: RwSignal::new_local(BTreeMap::new()),
            emphasized: RwSignal::new_local(None),
            annotations: RwSignal::new_local(Vec::new()),
            show_annotations: RwSignal::new_local(true),
            histogram: RwSignal::new_local(HistogramOptions::default()),
            bin_hover: RwSignal::new_local(None),
            curve_hover: RwSignal::new_local(None),
            auto_fit: RwSignal::new_local(true),
        }
    }
}
impl PlotState {
    pub(super) fn set_hover(self, hit: Option<SeriesHit>) {
        if self.hover.with_untracked(|current| current != &hit) {
            self.hover.set(hit);
        }
    }
    pub fn set_view(self, ranges: Ranges) -> Result<(), String> {
        if crate::range::valid(ranges.x).is_none() || crate::range::valid(ranges.y).is_none() {
            return Err("Both axes need finite, increasing bounds".into());
        }
        if self.auto_fit.get_untracked() {
            self.auto_fit.set(false);
        }
        if self.ranges.get_untracked() != ranges {
            self.ranges.set(ranges);
        }
        self.set_hover(None);
        if self.bin_hover.with_untracked(Option::is_some) {
            self.bin_hover.set(None);
        }
        Ok(())
    }
    /// Fit only requested axes once; keep other axis and disable continuous auto-fit.
    /// Y-only fit uses current X window. Empty/hidden windows keep current view.
    pub fn fit_axes(self, data: &PlotData, axes: crate::interaction::Axes) {
        self.fit_axes_with_drawing(data, axes, &BTreeMap::new());
    }
    pub(super) fn fit_axes_with_drawing(
        self,
        data: &PlotData,
        axes: crate::interaction::Axes,
        drawing: &BTreeMap<String, crate::line_draw::LineDraw>,
    ) {
        if !axes.x && !axes.y {
            return;
        }
        let hidden = self.hidden.get_untracked();
        if data
            .labels()
            .iter()
            .all(|label| hidden.contains(label.id()))
        {
            return;
        }
        let current = self.ranges.get_untracked();
        let bins = self.histogram.get_untracked().bins;
        let fitted = if axes.y && !axes.x {
            let Some(y) = data.fitted_y_range(&hidden, current.x, bins, drawing) else {
                return;
            };
            Ranges { x: current.x, y }
        } else {
            data.fitted_ranges(&hidden, bins)
        };
        let _ = self.set_view(Ranges {
            x: if axes.x { fitted.x } else { current.x },
            y: if axes.y { fitted.y } else { current.y },
        });
    }
    /// Fit one axis once without changing other bounds.
    pub fn fit_axis(self, data: &PlotData, axis: crate::labels::Axis) {
        self.fit_axes(
            data,
            crate::interaction::Axes {
                x: axis == crate::labels::Axis::X,
                y: axis == crate::labels::Axis::Y,
            },
        );
    }
    pub fn fit_to(self, series: &crate::time_series::TimeSeriesSet) {
        self.fit_data(&PlotData::Time(series.clone()));
    }
    pub fn fit_data(self, data: &PlotData) {
        self.auto_fit.set(true);
        self.ranges.set(data.fitted_ranges(
            &self.hidden.get_untracked(),
            self.histogram.get_untracked().bins,
        ));
        self.set_hover(None);
        if self.bin_hover.with_untracked(Option::is_some) {
            self.bin_hover.set(None);
        }
    }
    pub fn style(self, id: &str) -> SeriesStyle {
        self.styles.with(|styles| {
            styles
                .get(id)
                .copied()
                .unwrap_or_else(|| SeriesStyle::for_id(id))
        })
    }
    pub fn apply(self, update: RangeUpdate) {
        let next = update.apply(self.ranges.get_untracked());
        if crate::range::valid(next.x).is_some() && crate::range::valid(next.y).is_some() {
            let _ = self.set_view(next);
        }
    }
    pub fn duplicate(self) -> Self {
        Self {
            ranges: RwSignal::new_local(self.ranges.get_untracked()),
            hidden: RwSignal::new_local(self.hidden.get_untracked()),
            hover: RwSignal::new_local(None),
            lod: RwSignal::new_local(self.lod.get_untracked()),
            styles: RwSignal::new_local(self.styles.get_untracked()),
            emphasized: RwSignal::new_local(None),
            annotations: RwSignal::new_local(self.annotations.get_untracked()),
            show_annotations: RwSignal::new_local(self.show_annotations.get_untracked()),
            histogram: RwSignal::new_local(self.histogram.get_untracked()),
            bin_hover: RwSignal::new_local(None),
            curve_hover: RwSignal::new_local(None),
            auto_fit: RwSignal::new_local(self.auto_fit.get_untracked()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistogramOptions {
    pub bins: usize,
    pub cdf: bool,
    pub ccdf: bool,
    pub normal_fit: bool,
}
impl Default for HistogramOptions {
    fn default() -> Self {
        Self {
            bins: 80,
            cdf: false,
            ccdf: false,
            normal_fit: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramHit {
    pub bin: crate::Bin,
    pub sample_count: usize,
    pub percent: f64,
}

#[cfg(test)]
mod fit_tests {
    use super::*;
    use crate::{
        interaction::Axes,
        labels::Axis,
        time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    };

    #[test]
    fn fitting_axis_preserves_other_axis_and_hidden_data_is_excluded() {
        let owner = zgui::reactive::Owner::new();
        owner.with(|| {
            let state = PlotState::default();
            let data = |x, y| {
                SeriesData::new(vec![
                    crate::PlotPoint { x, y },
                    crate::PlotPoint {
                        x: x + 1.0,
                        y: y + 1.0,
                    },
                ])
                .unwrap()
            };
            let series = TimeSeriesSet::new(vec![
                TimeSeries::new("a", "a", data(0.0, 0.0)),
                TimeSeries::new("b", "b", data(100.0, 200.0)),
            ])
            .unwrap();
            let plot = PlotData::Time(series.clone());
            state.hidden.update(|hidden| {
                hidden.insert("b".into());
            });
            let fitted = series.fitted_ranges(&state.hidden.get_untracked());
            let zoom = Ranges::new((10.0, 20.0), (30.0, 40.0));
            state.set_view(zoom).unwrap();
            state.fit_axis(&plot, Axis::X);
            assert_eq!(
                state.ranges.get_untracked(),
                Ranges {
                    x: fitted.x,
                    y: zoom.y
                }
            );
            assert!(!state.auto_fit.get_untracked());
            state.set_view(zoom).unwrap();
            state.fit_axis(&plot, Axis::Y);
            assert_eq!(
                state.ranges.get_untracked(),
                Ranges {
                    x: zoom.x,
                    y: zoom.y
                }
            );
            state.hidden.update(|hidden| {
                hidden.insert("a".into());
            });
            let before = state.ranges.get_untracked();
            state.fit_axes(&plot, Axes::BOTH);
            assert_eq!(state.ranges.get_untracked(), before);
        });
    }
}
