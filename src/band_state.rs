use crate::{
    geometry::{Geometry, GeometryKind},
    state::Workbench,
    state_bands::StateBands,
};
use std::{collections::BTreeMap, sync::Arc};
use zgui::prelude::*;
#[derive(Clone, Copy)]
pub struct BandUi {
    pub open: RwSignal<bool, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub error: RwSignal<String, LocalStorage>,
    pub drop: NodeRef,
    pub hover: RwSignal<bool, LocalStorage>,
    pub jobs: StoredValue<BTreeMap<u64, u64>, LocalStorage>,
}
impl BandUi {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new_local(false),
            busy: RwSignal::new_local(false),
            error: RwSignal::new_local(String::new()),
            drop: NodeRef::new(),
            hover: RwSignal::new_local(false),
            jobs: StoredValue::new_local(BTreeMap::new()),
        }
    }
}
impl Workbench {
    pub fn cancel_band_source(self) {
        self.bands.hover.set(false);
        self.bands.open.set(false);
        self.bands.busy.set(false);
        self.bands.jobs.update_value(|jobs| {
            jobs.remove(&0);
        });
        self.bands.error.set(String::new());
    }
    pub fn generate_bands(self, source: String, existing: Option<u64>) {
        let Some(log) = self.log.get_untracked() else {
            self.bands.error.set("Open a log first".into());
            return;
        };
        if self.loading.get_untracked() {
            self.bands.error.set("Wait for log loading".into());
            return;
        }
        let derived = self.derived.with_untracked(|fields| {
            fields
                .iter()
                .find(|field| field.key() == source)
                .and_then(|field| field.data.clone())
        });
        if crate::derived::id(&source).is_some()
            && (derived.is_none() || self.derived_busy.get_untracked())
        {
            self.bands
                .error
                .set("Wait for source filter to finish".into());
            return;
        }
        let cached = derived.or_else(|| {
            self.cache
                .with_untracked(|cache| cache.get(&source).cloned())
        });
        let generation = self.generation.get_untracked();
        let pane = self.pane();
        let token = self.id();
        let job = existing.unwrap_or(0);
        self.bands.jobs.update_value(|jobs| {
            jobs.insert(job, token);
        });
        self.bands.error.set(String::new());
        if existing.is_none() {
            self.bands.busy.set(true);
        }
        self.task(async move {
            let key = source.clone();
            let end = log.duration;
            let result = blocking(move || {
                let input = match cached {
                    Some(data) => data,
                    None => log.prepare(&key)?,
                };
                StateBands::build(key, input, end)
            })
            .await;
            if self.generation.get_untracked() != generation
                || !self
                    .bands
                    .jobs
                    .with_value(|jobs| jobs.get(&job) == Some(&token))
            {
                return;
            }
            self.bands.jobs.update_value(|jobs| {
                jobs.remove(&job);
            });
            if let Some(id) = existing {
                if let Some(mut item) = self.geometry_item(id)
                    && let Some(previous) = item.states.as_ref()
                {
                    item.states = Some(Arc::new(match result {
                        Ok(mut states) => {
                            states.preserve_rules(previous);
                            states
                        }
                        Err(error) => {
                            let mut states = (**previous).clone();
                            states.error = Some(error);
                            states
                        }
                    }));
                    self.update_geometry(item, false);
                }
            } else {
                self.bands.busy.set(false);
                match result {
                    Ok(states) if self.find(pane.id).is_some() => {
                        let id = self.id();
                        let mut item = Geometry::new(id, GeometryKind::Band, kurbo::Point::ZERO);
                        item.name = format!("{} states", self.field_name(&source));
                        item.coordinates = [0., 0., end, 1.];
                        item.states = Some(Arc::new(states));
                        self.geometry.items.update(|items| items.push(item));
                        self.attach_geometry(id, pane, true);
                        self.bands.open.set(false);
                        self.edit_geometry_on(pane, id);
                    }
                    Ok(_) => self.bands.error.set("Target plot no longer exists".into()),
                    Err(error) => self.bands.error.set(error),
                }
            }
        });
    }
    pub fn refresh_derived_bands(self) {
        let items = self.geometry.items.get_untracked();
        for item in items {
            if let Some(states) = item.states
                && crate::derived::id(&states.source).is_some()
            {
                let changed = self.derived.with_untracked(|fields| {
                    fields
                        .iter()
                        .find(|field| field.key() == states.source)
                        .and_then(|field| field.data.as_ref())
                        .is_some_and(|data| {
                            states.error.is_some()
                                || data.samples().as_ptr() != states.input.samples().as_ptr()
                        })
                });
                if changed {
                    self.generate_bands(states.source.clone(), Some(item.id));
                }
            }
        }
    }
    pub fn clear_generated_bands(self) {
        self.cancel_band_source();
        self.bands.jobs.update_value(BTreeMap::clear);
        let ids = self.geometry.items.with_untracked(|items| {
            items
                .iter()
                .filter(|item| item.states.is_some())
                .map(|item| item.id)
                .collect::<Vec<_>>()
        });
        for id in ids {
            self.delete_geometry(id);
        }
    }
}
