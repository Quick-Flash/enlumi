//! Categorical sample-and-hold geometry. Missing values leave unshaded gaps.
use plot::time_series::SeriesData;
use std::{collections::BTreeSet, sync::Arc};
#[derive(Clone, Debug, PartialEq)]
pub struct StateInterval {
    pub start: f64,
    pub end: f64,
    pub value: i64,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub value: i64,
    pub name: String,
    pub color: [u8; 3],
}
#[derive(Clone, Debug)]
pub struct StateBands {
    pub source: String,
    pub input: SeriesData,
    pub intervals: Arc<[StateInterval]>,
    pub rules: Vec<StateRule>,
    pub error: Option<String>,
}
impl PartialEq for StateBands {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && std::ptr::eq(self.input.samples(), other.input.samples())
            && self.intervals == other.intervals
            && self.rules == other.rules
            && self.error == other.error
    }
}
impl StateBands {
    pub fn build(source: String, input: SeriesData, end: f64) -> Result<Self, String> {
        if !end.is_finite() || end < 0. {
            return Err("Log end must be finite and nonnegative".into());
        }
        let mut intervals: Vec<StateInterval> = Vec::new();
        let mut values = BTreeSet::new();
        let points = input.samples();
        let mut index = 0;
        while index < points.len() {
            let start = points[index].x;
            let mut last = index;
            while last + 1 < points.len() && points[last + 1].x == start {
                last += 1;
            }
            let point = points[last];
            let stop = points.get(last + 1).map_or(end, |p| p.x).min(end);
            index = last + 1;
            if start < 0. || !start.is_finite() {
                return Err("State timestamps must be finite and nonnegative".into());
            }
            if !point.y.is_finite() || start >= end || stop <= start {
                continue;
            }
            if point.y.fract() != 0.
                || !(-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&point.y)
            {
                return Err(
                    "State bands require exact integer values; use a discrete field".into(),
                );
            }
            let value = point.y as i64;
            values.insert(value);
            if values.len() > 256 {
                return Err("State bands support at most 256 distinct values".into());
            }
            if let Some(previous) = intervals.last_mut()
                && previous.end == start
                && previous.value == value
            {
                previous.end = stop;
            } else {
                intervals.push(StateInterval {
                    start,
                    end: stop,
                    value,
                });
            }
            if intervals.len() > 100_000 {
                return Err("State source changes too often (over 100000 intervals)".into());
            }
        }
        if intervals.is_empty() {
            return Err("Source has no finite state intervals within log".into());
        }
        let rules = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| StateRule {
                value,
                name: format!("State {value}"),
                color: {
                    let color = plot::palette::DEFAULT_PALETTE
                        [index % plot::palette::DEFAULT_PALETTE.len()];
                    [color[0], color[1], color[2]]
                },
            })
            .collect();
        Ok(Self {
            source,
            input,
            intervals: intervals.into(),
            rules,
            error: None,
        })
    }
    pub fn set_rule(
        &mut self,
        previous: Option<i64>,
        value: &str,
        name: &str,
    ) -> Result<(), String> {
        let value = value
            .trim()
            .parse::<i64>()
            .map_err(|_| "State value must be an integer".to_string())?;
        if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) {
            return Err("State value exceeds exact integer range".into());
        }
        if name.trim().is_empty() || name.len() > 256 {
            return Err("State name needs 1 to 256 bytes of text".into());
        }
        if self
            .rules
            .iter()
            .any(|rule| rule.value == value && Some(rule.value) != previous)
        {
            return Err(format!("State {value} already exists"));
        }
        if let Some(previous) = previous {
            let rule = self
                .rules
                .iter_mut()
                .find(|rule| rule.value == previous)
                .ok_or("State no longer exists")?;
            rule.value = value;
            rule.name = name.into();
        } else {
            let palette = plot::palette::DEFAULT_PALETTE;
            let color = palette
                .iter()
                .find(|color| {
                    !self
                        .rules
                        .iter()
                        .any(|rule| rule.color == [color[0], color[1], color[2]])
                })
                .unwrap_or(&palette[self.rules.len() % palette.len()]);
            self.rules.push(StateRule {
                value,
                name: name.into(),
                color: [color[0], color[1], color[2]],
            });
        }
        self.rules.sort_by_key(|rule| rule.value);
        Ok(())
    }
    pub fn preserve_rules(&mut self, old: &Self) {
        self.apply_rules(old.rules.clone());
    }
    pub fn apply_rules(&mut self, mut rules: Vec<StateRule>) {
        for value in self
            .intervals
            .iter()
            .map(|interval| interval.value)
            .collect::<BTreeSet<_>>()
        {
            if !rules.iter().any(|rule| rule.value == value) {
                // Unmapped values retain numeric labels, never firmware assumptions.
                let color = plot::palette::DEFAULT_PALETTE
                    [rules.len() % plot::palette::DEFAULT_PALETTE.len()];
                rules.push(StateRule {
                    value,
                    name: format!("State {value}"),
                    color: [color[0], color[1], color[2]],
                });
            }
        }
        rules.sort_by_key(|rule| rule.value);
        self.rules = rules;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plot::PlotPoint;
    fn data(points: &[(f64, f64)]) -> SeriesData {
        SeriesData::new(
            points
                .iter()
                .map(|&(x, y)| PlotPoint { x, y })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
    #[test]
    fn custom_mapping_edits_validate_before_mutation_and_keep_colors() {
        let mut bands = StateBands::build("custom".into(), data(&[(0., 0.)]), 1.).unwrap();
        bands.set_rule(None, "42", "Custom mode").unwrap();
        let color = bands.rules[1].color;
        let before = bands.rules.clone();
        for (value, name) in [
            ("42", "Duplicate"),
            ("1.5", "Float"),
            ("9007199254740992", "Too large"),
            ("-2", " "),
        ] {
            assert!(bands.set_rule(None, value, name).is_err());
            assert_eq!(bands.rules, before);
        }
        bands.set_rule(Some(42), "-2", "Renamed").unwrap();
        assert_eq!(
            bands.rules[0],
            StateRule {
                value: -2,
                name: "Renamed".into(),
                color
            }
        );
        assert!(bands.set_rule(Some(-2), "0", "Collision").is_err());
        assert_eq!(bands.rules[0].value, -2);
    }
    #[test]
    fn holds_states_merges_repeats_last_duplicate_wins_and_missing_values_break() {
        let bands = StateBands::build(
            "vehicle_status/0/nav_state".into(),
            data(&[
                (0., 0.),
                (1., 1.),
                (1., 3.),
                (2., 3.),
                (3., f64::NAN),
                (4., 3.),
                (5., 99.),
            ]),
            6.,
        )
        .unwrap();
        assert_eq!(
            bands.intervals.as_ref(),
            &[
                StateInterval {
                    start: 0.,
                    end: 1.,
                    value: 0
                },
                StateInterval {
                    start: 1.,
                    end: 3.,
                    value: 3
                },
                StateInterval {
                    start: 4.,
                    end: 5.,
                    value: 3
                },
                StateInterval {
                    start: 5.,
                    end: 6.,
                    value: 99
                }
            ]
        );
        assert_eq!(
            bands.rules.iter().find(|v| v.value == 3).unwrap().name,
            "State 3"
        );
        assert_eq!(
            bands.rules.iter().find(|v| v.value == 99).unwrap().name,
            "State 99"
        );
        let trimmed = StateBands::build("custom".into(), data(&[(1., 0.), (3., 1.)]), 2.).unwrap();
        assert_eq!(
            trimmed.intervals.as_ref(),
            &[StateInterval {
                start: 1.,
                end: 2.,
                value: 0
            }]
        );
    }
    #[test]
    fn rejects_continuous_empty_negative_time_and_excessive_cardinality_sources() {
        for (points, end) in [
            (vec![(0., 0.5)], 1.),
            (vec![(0., f64::NAN)], 1.),
            (vec![], 1.),
            (vec![(-1., 0.)], 1.),
            (vec![(0., 0.)], f64::INFINITY),
            (vec![(0., 0.)], -1.),
            (vec![(1., 0.)], 1.),
        ] {
            assert!(StateBands::build("test".into(), data(&points), end).is_err());
        }
        let points = (0..257).map(|n| (n as f64, n as f64)).collect::<Vec<_>>();
        assert!(
            StateBands::build("test".into(), data(&points), 257.)
                .unwrap_err()
                .contains("256")
        );
        let points = (0..100_002)
            .map(|n| (n as f64, (n % 2) as f64))
            .collect::<Vec<_>>();
        assert!(
            StateBands::build("test".into(), data(&points), 100_002.)
                .unwrap_err()
                .contains("100000")
        );
    }
    #[test]
    fn refreshed_source_preserves_named_color_mapping_without_inventing_old_modes() {
        let mut old = StateBands::build("custom".into(), data(&[(0., 0.), (1., 1.)]), 2.).unwrap();
        old.rules[0].name = "Pilot".into();
        old.rules[0].color = [1, 2, 3];
        let mut new = StateBands::build("custom".into(), data(&[(0., 0.), (1., 2.)]), 2.).unwrap();
        new.preserve_rules(&old);
        assert_eq!(new.rules[0], old.rules[0]);
        assert_eq!(new.rules[1].value, 1);
        assert_eq!(new.rules[2].value, 2);
        let unknown =
            StateBands::build("vehicle_status/0/nav_state".into(), data(&[(0., 6.)]), 1.).unwrap();
        assert_eq!(
            unknown.rules[0].name, "State 6",
            "Version-dependent enum slots stay numeric"
        );
    }
}
