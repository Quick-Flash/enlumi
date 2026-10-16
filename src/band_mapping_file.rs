//! Portable mappings contain no field paths or firmware-specific code.
use crate::{geometry_inspector::change, state::Workbench, state_bands::StateRule};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Read};
use zgui::prelude::*;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingFile {
    version: u32,
    states: Vec<StateRule>,
}
impl MappingFile {
    fn parse(text: &str) -> Result<Vec<StateRule>, String> {
        let file: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if file.version != 1 {
            return Err(format!(
                "Unsupported mapping version {}; expected 1",
                file.version
            ));
        }
        if file.states.is_empty() {
            return Err("Mapping needs at least one state".into());
        }
        let mut values = BTreeSet::new();
        for rule in &file.states {
            if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&rule.value) {
                return Err("State values must be exactly representable integers".into());
            }
            if !values.insert(rule.value) {
                return Err(format!("Duplicate state value {}", rule.value));
            }
            if rule.name.trim().is_empty() || rule.name.len() > 256 {
                return Err("State names need 1 to 256 bytes of text".into());
            }
        }
        Ok(file.states)
    }
    fn encode(states: Vec<StateRule>) -> Result<String, String> {
        let text = serde_json::to_string_pretty(&Self { version: 1, states })
            .map_err(|error| error.to_string())?;
        if text.len() + 1 > 1_048_576 {
            return Err("Mapping file exceeds 1 MiB".into());
        }
        Self::parse(&text)?;
        Ok(text + "\n")
    }
}
impl Workbench {
    pub fn mapping_file(self, id: u64, export: bool) {
        let Some(states) = self.geometry_item(id).and_then(|item| item.states) else {
            return;
        };
        self.task(async move {
            let result = blocking(move || -> Result<Option<Vec<StateRule>>, String> {
                let dialog = rfd::FileDialog::new().add_filter("State mapping (JSON)", &["json"]);
                if export {
                    let text = MappingFile::encode(states.rules.clone())?;
                    let Some(path) = dialog.set_file_name("state-mapping.json").save_file() else {
                        return Ok(None);
                    };
                    std::fs::write(path, text).map_err(|error| error.to_string())?;
                    Ok(None)
                } else {
                    let Some(path) = dialog.pick_file() else {
                        return Ok(None);
                    };
                    let mut text = String::new();
                    std::fs::File::open(path)
                        .map_err(|error| error.to_string())?
                        .take(1_048_577)
                        .read_to_string(&mut text)
                        .map_err(|error| error.to_string())?;
                    if text.len() > 1_048_576 {
                        return Err("Mapping file exceeds 1 MiB".into());
                    }
                    MappingFile::parse(&text).map(Some)
                }
            })
            .await;
            match result {
                Ok(Some(rules)) => {
                    change(self, id, |item| {
                        if let Some(states) = item.states.as_mut() {
                            std::sync::Arc::make_mut(states).apply_rules(rules);
                        }
                    });
                    self.geometry.error.set(String::new());
                }
                Ok(None) => {}
                Err(error) => self.geometry.error.set(format!("Mapping: {error}")),
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_rules_replace_defaults_keep_absent_states_and_survive_refresh() {
        use crate::state_bands::StateBands;
        use plot::{PlotPoint, time_series::SeriesData};
        let data = |value| SeriesData::new(vec![PlotPoint { x: 0., y: value }]).unwrap();
        let mut bands =
            StateBands::build("vehicle_status/0/nav_state".into(), data(3.), 1.).unwrap();
        assert_eq!(bands.rules[0].name, "State 3");
        let rules = MappingFile::parse(r#"{"version":1,"states":[{"value":3,"name":"Fork hover","color":[1,2,3]},{"value":42,"name":"Future state","color":[4,5,6]}]}"#).unwrap();
        bands.apply_rules(rules);
        assert_eq!(bands.rules[0].name, "Fork hover");
        let mut refreshed = StateBands::build("custom".into(), data(7.), 1.).unwrap();
        refreshed.preserve_rules(&bands);
        assert_eq!(
            refreshed
                .rules
                .iter()
                .map(|rule| rule.value)
                .collect::<Vec<_>>(),
            vec![3, 7, 42]
        );
        assert_eq!(refreshed.rules[1].name, "State 7");
        assert_eq!(
            MappingFile::parse(&MappingFile::encode(refreshed.rules.clone()).unwrap()).unwrap(),
            refreshed.rules
        );
    }
    #[test]
    fn portable_mapping_round_trip_and_rejection() {
        let rules = vec![StateRule {
            value: 42,
            name: "Custom hover".into(),
            color: [3, 5, 7],
        }];
        assert_eq!(
            MappingFile::parse(&MappingFile::encode(rules.clone()).unwrap()).unwrap(),
            rules
        );
        for text in [
            r#"{"version":2,"states":[]}"#,
            r#"{"version":1,"states":[]}"#,
            r#"{"version":1,"states":[{"value":0,"name":"","color":[0,0,0]}]}"#,
            r#"{"version":1,"states":[{"value":0,"name":"x","color":[256,0,0]}]}"#,
            r#"{"version":1,"states":[{"value":0,"name":"x","color":[0,0,0]},{"value":0,"name":"y","color":[0,0,0]}]}"#,
        ] {
            assert!(MappingFile::parse(text).is_err(), "{text}");
        }
        let example = MappingFile::parse(include_str!(
            "../examples/state-mappings/px4-nav-state.json"
        ))
        .unwrap();
        assert!(
            example
                .iter()
                .any(|rule| rule.value == 3 && rule.name == "Mission")
        );
    }
}
