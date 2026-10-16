use crate::{geometry_inspector::change, state::Workbench};
use zgui::prelude::*;
impl Workbench {
    pub fn new_state_mapping(self) {
        let id = self.id();
        let mut item = crate::geometry::Geometry::new(
            id,
            crate::geometry::GeometryKind::Band,
            kurbo::Point::ZERO,
        );
        item.name = "New state mapping".into();
        item.states = Some(std::sync::Arc::new(crate::state_bands::StateBands {
            source: String::new(),
            input: plot::time_series::SeriesData::new(vec![]).unwrap(),
            intervals: std::sync::Arc::from([]),
            rules: vec![],
            error: None,
        }));
        self.geometry.items.update(|items| items.push(item));
        self.cancel_band_source();
        self.edit_geometry_on(self.pane(), id);
    }

    pub fn edit_mapping_rule(
        self,
        id: u64,
        previous: Option<i64>,
        value: &str,
        name: &str,
    ) -> bool {
        let mut result = Err("State layer no longer exists".to_string());
        change(self, id, |item| {
            if let Some(states) = item.states.as_mut() {
                result = std::sync::Arc::make_mut(states).set_rule(previous, value, name);
            }
        });
        match result {
            Ok(()) => {
                self.geometry.error.set(String::new());
                true
            }
            Err(error) => {
                self.geometry.error.set(error);
                false
            }
        }
    }
    pub fn remove_mapping_rule(self, id: u64, value: i64) {
        change(self, id, |item| {
            if let Some(states) = item.states.as_mut() {
                std::sync::Arc::make_mut(states)
                    .rules
                    .retain(|rule| rule.value != value);
            }
        });
        self.geometry.error.set(String::new());
    }
}
