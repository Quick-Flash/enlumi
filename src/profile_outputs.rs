use crate::{profile::identity::ProfileKey, state::Workbench};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub struct ProfileFieldData {
    pub counts: Memo<std::collections::BTreeMap<String, usize>>,
}
impl ProfileFieldData {
    pub fn new(workbench: Workbench) -> Self {
        Self {
            counts: Memo::new(move |_| {
                let mut counts = workbench.log.with(|log| {
                    log.as_ref()
                        .map_or_else(std::collections::BTreeMap::new, |log| {
                            log.catalogue
                                .iter()
                                .map(|field| (field.id.clone(), field.count))
                                .collect()
                        })
                });
                workbench.derived.with(|fields| {
                    counts.extend(fields.iter().filter_map(|field| {
                        field
                            .data
                            .as_ref()
                            .map(|data| (field.key(), data.samples().len()))
                    }));
                });
                counts
            }),
        }
    }
}

impl Workbench {
    pub fn profile_field_search_text(self, key: &ProfileKey, token: &str) -> String {
        self.profiles
            .instances
            .with(|instances| {
                instances
                    .get(key)
                    .and_then(|instance| instance.resolve(token))
            })
            .map_or_else(String::new, |source| {
                let name = self.field_name(&source);
                self.profiles.metadata.with(|metadata| {
                    let extra = metadata.get(&source).map_or_else(String::new, |meta| {
                        format!("{} {} {}", meta.name, meta.axis, meta.unit)
                    });
                    format!("{source} {name} {extra}").to_lowercase()
                })
            })
    }
}
