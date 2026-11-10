//! Optional ordered, translucent state colors. Definitions and labels remain untouched.
use crate::{
    state::{Pane, Workbench},
    state_bands::StateBands,
};
use std::sync::Arc;
use zgui::prelude::*;
#[derive(Clone, PartialEq)]
pub struct Tint {
    pub states: Arc<StateBands>,
    pub opacity: f64,
}
pub fn bands(workbench: Workbench, pane: Pane) -> Vec<Tint> {
    if !pane.state_tint.get() {
        return Vec::new();
    }
    let hidden = pane.hidden_geometry.get();
    workbench
        .ordered_geometry(pane)
        .into_iter()
        .filter(|item| !hidden.contains(&item.id) && item.opacity > 0.)
        .filter_map(|item| {
            item.states
                .filter(|s| s.error.is_none())
                .map(|states| Tint {
                    states,
                    opacity: item.opacity,
                })
        })
        .collect()
}
pub fn at(tints: &[Tint], time: f64, mut base: [u8; 4]) -> [u8; 4] {
    if !time.is_finite() {
        return base;
    }
    for tint in tints {
        let intervals = &tint.states.intervals;
        let index = intervals.partition_point(|interval| interval.end <= time);
        if let Some(interval) = intervals
            .get(index)
            .filter(|interval| interval.start <= time && time < interval.end)
            && let Some(rule) = tint
                .states
                .rules
                .iter()
                .find(|rule| rule.value == interval.value)
        {
            for (channel, tint_channel) in base[..3].iter_mut().zip(rule.color) {
                *channel = (f64::from(*channel) * (1. - tint.opacity)
                    + f64::from(tint_channel) * tint.opacity)
                    .round() as u8;
            }
        }
    }
    base
}
pub fn cuts(tints: &[Tint], start: f64, end: f64) -> Vec<f64> {
    let mut cuts = vec![0., 1.];
    if !start.is_finite() || !end.is_finite() || end <= start {
        return cuts;
    }
    for tint in tints {
        let first = tint
            .states
            .intervals
            .partition_point(|interval| interval.end <= start);
        for interval in tint
            .states
            .intervals
            .iter()
            .skip(first)
            .take_while(|interval| interval.start < end)
        {
            for time in [interval.start, interval.end] {
                if time > start && time < end {
                    cuts.push((time - start) / (end - start));
                }
            }
        }
    }
    cuts.sort_unstable_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    cuts
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> Tint {
        let mut states = StateBands::build(
            "mode".into(),
            plot::time_series::SeriesData::new(vec![
                plot::PlotPoint { x: 0., y: 0. },
                plot::PlotPoint { x: 1., y: 1. },
                plot::PlotPoint { x: 2., y: f64::NAN },
                plot::PlotPoint { x: 3., y: 0. },
            ])
            .unwrap(),
            4.,
        )
        .unwrap();
        states.rules[0].name = "Disarmed".into();
        states.rules[0].color = [255, 0, 0];
        states.rules[1].name = "Armed".into();
        states.rules[1].color = [0, 255, 0];
        Tint {
            states: Arc::new(states),
            opacity: 0.25,
        }
    }
    #[test]
    fn tint_keeps_base_alpha_gaps_and_state_names() {
        let tint = fixture();
        let base = [0, 0, 200, 128];
        let tints = [tint.clone()];
        assert_eq!(at(&tints, 0.5, base), [64, 0, 150, 128]);
        assert_eq!(at(&tints, 1., base), [0, 64, 150, 128]);
        for time in [-1., 2., 2.5, 4., f64::NAN] {
            assert_eq!(at(&tints, time, base), base);
        }
        assert_eq!(at(&[], 0.5, base), base);
        assert_eq!(
            tint.states
                .rules
                .iter()
                .map(|rule| rule.name.as_str())
                .collect::<Vec<_>>(),
            ["Disarmed", "Armed"]
        );
        assert_eq!(cuts(&tints, 0., 4.), [0., 0.25, 0.5, 0.75, 1.]);
        assert_eq!(cuts(&tints, 0.5, 1.5), [0., 0.5, 1.]);
        assert_eq!(
            cuts(&[tint.clone(), tint], 0., 4.),
            [0., 0.25, 0.5, 0.75, 1.]
        );
    }
    #[test]
    fn overlapping_tints_follow_band_order() {
        let red = fixture();
        let mut green = red.clone();
        Arc::make_mut(&mut green.states).rules[0].color = [0, 255, 0];
        assert_eq!(
            at(&[red.clone(), green.clone()], 0.5, [0, 0, 200, 255]),
            [48, 64, 113, 255]
        );
        assert_eq!(at(&[green, red], 0.5, [0, 0, 200, 255]), [64, 48, 113, 255]);
    }
}
