use crate::{profile::identity::ProfileKey, state::Workbench};
use zgui::prelude::*;

impl Workbench {
    pub fn profile_field_matches(self, key: &ProfileKey, token: &str, query: &str) -> bool {
        self.profiles
            .instances
            .with(|instances| {
                instances
                    .get(key)
                    .and_then(|instance| instance.resolve(token))
            })
            .is_some_and(|source| {
                source.to_lowercase().contains(query)
                    || self.field_name(&source).to_lowercase().contains(query)
                    || self.profiles.metadata.with(|metadata| {
                        metadata.get(&source).is_some_and(|meta| {
                            format!("{} {} {}", meta.name, meta.axis, meta.unit)
                                .to_lowercase()
                                .contains(query)
                        })
                    })
            })
    }
}
