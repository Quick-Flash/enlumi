//! Decimal tick levels shared by grid lines and axis labels.

use std::cmp::Ordering;

use crate::Range;

/// One decimal tick level whose multiples are drawn at a common opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickLevel {
    /// 0 is the finest level; each level is ten times coarser.
    pub level: u8,
    pub step: f64,
    /// Screen distance between adjacent ticks of this level.
    pub spacing: f64,
}

impl TickLevel {
    /// Tick values of this level inside `range`. Finer levels omit multiples of
    /// ten so shared positions are only emitted once, by the coarsest level.
    pub fn values(self, range: impl Into<Range>) -> impl Iterator<Item = f64> {
        let range = range.into();
        let start = (range.min / self.step).ceil();
        (0..512)
            .map(move |index| {
                let multiple = start + f64::from(index);
                (multiple, multiple * self.step)
            })
            .take_while(move |(_, value)| value.partial_cmp(&range.max) != Some(Ordering::Greater))
            .filter(move |(multiple, value)| {
                *value >= range.min
                    && value.is_finite()
                    && !(self.level < 2 && multiple.rem_euclid(10.0) == 0.0)
            })
            .map(|(_, value)| value)
    }
}

/// Three decimal tick levels, starting at the first power of ten whose ticks
/// are at least `min_pixel_spacing` apart (egui_plot `log_grid_spacer(10)`).
pub fn tick_levels(
    range: impl Into<Range>,
    pixels: f64,
    min_pixel_spacing: f64,
    min_step: f64,
) -> impl Iterator<Item = TickLevel> {
    let range = range.into();
    let span = range.span();
    let valid = range.min.is_finite()
        && span.is_finite()
        && span > 0.0
        && pixels.is_finite()
        && pixels > 0.0;
    let base = 10.0_f64
        .powf((span * min_pixel_spacing / pixels).log10().ceil())
        .max(min_step);
    (0..3u8)
        .filter(move |_| valid)
        .map(move |level| {
            let step = base * 10.0_f64.powi(i32::from(level));
            TickLevel {
                level,
                step,
                spacing: step / span * pixels,
            }
        })
        .filter(move |level| level.step.is_finite() && level.step > 0.0 && level.step >= min_step)
}

/// Fade-in thresholds for axis labels, in pixels between adjacent labels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelOptions {
    pub min_step: f64,
    pub min_pixel_spacing: f64,
    pub fade_start: f64,
    pub fade_end: f64,
}

impl LabelOptions {
    /// egui_plot defaults for labels along a horizontal axis.
    pub fn horizontal(min_step: f64) -> Self {
        Self {
            min_step,
            min_pixel_spacing: 8.0,
            fade_start: 60.0,
            fade_end: 80.0,
        }
    }

    /// egui_plot defaults for labels along a vertical axis.
    pub fn vertical(min_step: f64) -> Self {
        Self {
            fade_start: 20.0,
            fade_end: 30.0,
            ..Self::horizontal(min_step)
        }
    }
}

/// Label opacity for a level whose labels are `spacing` pixels apart.
pub fn label_opacity(spacing: f64, fade_start: f64, fade_end: f64) -> f64 {
    ((spacing - fade_start) / (fade_end - fade_start))
        .clamp(0.0, 1.0)
        .sqrt()
}

/// Visible label values with their opacity, sorted by value. Each mark fades
/// according to the pixel spacing of its own level.
pub fn label_marks(range: impl Into<Range>, pixels: f64, options: LabelOptions) -> Vec<(f64, f64)> {
    let range = range.into();
    let mut marks = Vec::new();
    for level in tick_levels(range, pixels, options.min_pixel_spacing, options.min_step) {
        let opacity = label_opacity(level.spacing, options.fade_start, options.fade_end);
        if opacity > 0.0 {
            marks.extend(level.values(range).map(|value| (value, opacity)));
        }
    }
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    marks
}

/// Compact numeric label: up to four decimals, scientific for tiny or huge values.
pub fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "N/A".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.abs() < 0.001 || value.abs() >= 100_000.0 {
        return format!("{value:.4e}");
    }
    format!("{value:.4}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finer_levels_skip_values_owned_by_coarser_levels() {
        let levels: Vec<_> = tick_levels((0.0, 100.0), 1000.0, 8.0, 0.0).collect();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0].step, 1.0);
        assert!(!levels[0].values((0.0, 100.0)).any(|value| value == 10.0));
        assert!(levels[2].values((0.0, 100.0)).any(|value| value == 100.0));
    }

    #[test]
    fn invalid_ranges_produce_no_levels() {
        for range in [(1.0, 1.0), (f64::NAN, 1.0), (0.0, f64::INFINITY)] {
            assert_eq!(tick_levels(range, 500.0, 8.0, 0.0).count(), 0);
        }
        assert_eq!(tick_levels((0.0, 1.0), 0.0, 8.0, 0.0).count(), 0);
    }

    #[test]
    fn min_step_keeps_integer_axes_integral() {
        let marks = label_marks((0.0, 3.0), 1000.0, LabelOptions::vertical(1.0));
        assert!(marks.iter().all(|(value, _)| value.fract() == 0.0));
    }

    #[test]
    fn each_mark_fades_by_its_own_level_on_both_axes() {
        for (options, low, high) in [
            (LabelOptions::horizontal(0.0), 60.0, 80.0),
            (LabelOptions::vertical(0.0), 20.0, 30.0),
        ] {
            let opacity = |pixels| {
                label_marks((0.0, 10.0), pixels, options)
                    .into_iter()
                    .find(|mark| mark.0 == 1.0)
                    .map_or(0.0, |mark| mark.1)
            };
            assert_eq!(opacity(low * 10.0), 0.0);
            assert!(opacity((low + 0.001) * 10.0) < 0.02);
            assert!((opacity((low + high) * 5.0) - 0.5_f64.sqrt()).abs() < 1e-10);
            assert_eq!(opacity(high * 10.0), 1.0);
        }
    }

    #[test]
    fn decimal_rollover_keeps_visible_coarse_marks() {
        for pixels in [799.999, 800.001] {
            let marks = label_marks((0.0, 10.0), pixels, LabelOptions::horizontal(0.0));
            let ones = marks
                .iter()
                .filter(|mark| mark.0 == 1.0)
                .collect::<Vec<_>>();
            assert_eq!(ones.len(), 1);
            assert!((ones[0].1 - 1.0).abs() < 0.00001);
        }
    }

    #[test]
    fn label_opacity_fades_between_thresholds() {
        assert_eq!(label_opacity(60.0, 60.0, 80.0), 0.0);
        assert_eq!(label_opacity(80.0, 60.0, 80.0), 1.0);
        assert!((label_opacity(70.0, 60.0, 80.0) - 0.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn numbers_trim_and_switch_to_scientific() {
        assert_eq!(format_number(1.25), "1.25");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(0.0001), "1.0000e-4");
        assert_eq!(format_number(f64::NAN), "N/A");
    }
}
