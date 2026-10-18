use super::{Operation, Profile, Scalar, Shape, Source};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ProfileKey {
    pub path: PathBuf,
    pub flight: usize,
}
impl ProfileKey {
    pub fn new(path: PathBuf, flight: usize) -> Self {
        Self { path, flight }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum Item {
    View(String),
    Field(String),
    Geometry(String),
}
impl Item {
    pub fn name(&self) -> &str {
        match self {
            Self::View(n) | Self::Field(n) | Self::Geometry(n) => n,
        }
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Drag {
    pub profile: ProfileKey,
    pub item: Item,
}
impl Drag {
    pub fn source(&self) -> String {
        format!(
            "@profile/{}",
            serde_json::to_string(self).expect("profile drag serializes")
        )
    }
    pub fn parse(source: &str) -> Option<Self> {
        serde_json::from_str(source.strip_prefix("@profile/")?).ok()
    }
}
#[derive(Clone, Debug, Default)]
pub struct ResourceIds {
    pub fields: BTreeMap<String, u64>,
    pub geometry: BTreeMap<String, u64>,
}

impl Profile {
    /// Profiles use flight 1 as template scope; explicit other flights stay explicit.
    pub fn for_flight(mut self, flight: usize) -> Self {
        fn field(value: &mut String, flight: usize) {
            if let Some(tail) = value.strip_prefix("blackbox/1/") {
                *value = format!("blackbox/{flight}/{tail}");
            }
        }
        fn setting(value: &mut super::Setting, flight: usize) {
            if value.flight == 1 {
                value.flight = flight;
            }
        }
        fn source(value: &mut Source, flight: usize) {
            match value {
                Source::Field(id) => field(id, flight),
                Source::Setting { setting: value } => setting(value, flight),
            }
        }
        fn scalar(value: &mut Scalar, flight: usize) {
            match value {
                Scalar::Setting { setting: value } => setting(value, flight),
                Scalar::Ratio { ratio } => {
                    scalar(&mut ratio[0], flight);
                    scalar(&mut ratio[1], flight);
                }
                _ => (),
            }
        }
        for condition in &mut self.requires_settings {
            setting(&mut condition.setting, flight);
        }
        for id in &mut self.requires {
            field(id, flight);
        }
        for signal in &mut self.signals {
            source(&mut signal.source, flight);
        }
        for group in &mut self.field_groups {
            field(&mut group.source, flight);
        }
        for filter in &mut self.filters {
            for input in &mut filter.inputs {
                source(input, flight);
            }
            match &mut filter.operation {
                Operation::Affine { gain, offset } => {
                    scalar(gain, flight);
                    scalar(offset, flight);
                }
                Operation::LowPass { hz }
                | Operation::HighPass { hz }
                | Operation::ZeroPhaseLowPass { hz }
                | Operation::ZeroPhaseHighPass { hz } => scalar(hz, flight),
                Operation::Integral { initial } => scalar(initial, flight),
                _ => (),
            }
        }
        for geometry in &mut self.geometry {
            match &mut geometry.shape {
                Shape::Line { coordinates } | Shape::Rectangle { coordinates } => {
                    for value in coordinates {
                        scalar(value, flight);
                    }
                }
                Shape::Band { start, end } => {
                    scalar(start, flight);
                    scalar(end, flight);
                }
                Shape::States { source: value, .. } | Shape::Flags { source: value, .. } => {
                    source(value, flight)
                }
            }
        }
        for view in &mut self.views {
            for value in &mut view.fields {
                source(value, flight);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_template_scopes_raw_sources_settings_and_nested_ratios() {
        let profile:Profile=serde_json::from_value(serde_json::json!({
            "version":1,"name":"Scoped",
            "requires":["blackbox/1/gyroADC[0]","blackbox/3/gyroADC[0]"],
            "requires_settings":[{"setting":{"name":"Firmware revision"},"starts_with":"Betaflight"}],
            "signals":[{"id":"gyro","source":"blackbox/1/gyroADC[0]","metadata":{"name":"Roll"}}],
            "filters":[{"id":"scaled","metadata":{"name":"Scaled"},"inputs":["$gyro"],"operation":{"kind":"affine","gain":{"ratio":[200,{"setting":{"name":"motor_poles"}}]}}}],
            "geometry":[{"id":"mode","name":"Modes","shape":{"kind":"flags","source":"blackbox/1/flightModeFlags","rules":[{"mask":1,"name":"Example","color":[1,2,3]}]}}],
            "views":[{"name":"Scope","fields":["blackbox/1/gyroADC[0]",{"setting":{"name":"motor_poles"}}]}]
        })).unwrap();
        let profile = profile.for_flight(2);
        assert_eq!(
            profile.requires,
            ["blackbox/2/gyroADC[0]", "blackbox/3/gyroADC[0]"]
        );
        assert_eq!(profile.requires_settings[0].setting.flight, 2);
        let Operation::Affine {
            gain: Scalar::Ratio { ratio },
            ..
        } = &profile.filters[0].operation
        else {
            panic!("ratio")
        };
        let Scalar::Setting { setting } = &ratio[1] else {
            panic!("setting")
        };
        assert_eq!(setting.flight, 2);
        let Shape::Flags {
            source: Source::Field(source),
            ..
        } = &profile.geometry[0].shape
        else {
            panic!("flags")
        };
        assert_eq!(source, "blackbox/2/flightModeFlags");
        let Source::Setting { setting } = &profile.views[0].fields[1] else {
            panic!("setting")
        };
        assert_eq!(setting.flight, 2);
        let drag = Drag {
            profile: ProfileKey::new("/tmp/folder/recipe.json".into(), 2),
            item: Item::View("Roll / PID".into()),
        };
        let decoded = Drag::parse(&drag.source()).unwrap();
        assert_eq!(decoded.profile, drag.profile);
        assert_eq!(decoded.item, drag.item);
    }
}
