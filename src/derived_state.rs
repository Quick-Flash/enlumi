use crate::{
    derived::{self, DerivedField, FilterSpec},
    state::Workbench,
};
use zgui::prelude::*;

impl Workbench {
    pub fn field_name(self, source: &str) -> String {
        self.derived.with(|fields| {
            fields
                .iter()
                .find(|field| field.key() == source)
                .map_or_else(|| source.to_owned(), |field| field.name.clone())
        })
    }
    pub fn edit_filter(self, id: Option<u64>, source: Option<String>) {
        self.filter_edit.set(id);
        self.filter_outputs.set(id.into_iter().collect());
        if let Some(field) = self
            .derived
            .with_untracked(|fields| fields.iter().find(|field| Some(field.id) == id).cloned())
        {
            self.filter_inputs.set(vec![crate::derived::FilterInput {
                source: field.source,
                name: field.name,
            }]);
            match field.filter {
                FilterSpec::LowPass(hz) | FilterSpec::HighPass(hz) => {
                    self.filter_kind.set(
                        if matches!(field.filter, FilterSpec::LowPass(_)) {
                            "low"
                        } else {
                            "high"
                        }
                        .into(),
                    );
                    self.filter_cutoff.set(hz.to_string());
                }
                FilterSpec::Affine { gain, offset } => {
                    self.filter_kind.set("affine".into());
                    self.filter_gain.set(gain.to_string());
                    self.filter_offset.set(offset.to_string());
                }
            }
        } else {
            self.filter_inputs.set(Vec::new());
            self.filter_kind.set("low".into());
            self.filter_cutoff.set("5".into());
            self.filter_gain.set("1".into());
            self.filter_offset.set("0".into());
            if let Some(source) = source
                && let Err(error) = self.accept_filter_sources(vec![source])
            {
                self.filter_error.set(error);
            }
        }
        self.filter_error.set(String::new());
        self.filters_open.set(true);
    }
    pub fn next_filters(self) {
        let sources = self
            .filter_outputs
            .get_untracked()
            .into_iter()
            .map(derived::key)
            .collect();
        self.filter_edit.set(None);
        self.filter_outputs.set(Vec::new());
        self.filter_inputs.set(Vec::new());
        if let Err(error) = self.accept_filter_sources(sources) {
            self.filter_error.set(error);
        }
    }
    pub fn save_filter(self) {
        let parse = |value: String| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| "Filter parameters must be numbers".to_owned())
        };
        let spec = match self.filter_kind.get_untracked().as_str() {
            "high" => parse(self.filter_cutoff.get_untracked()).map(FilterSpec::HighPass),
            "affine" => parse(self.filter_gain.get_untracked()).and_then(|gain| {
                parse(self.filter_offset.get_untracked())
                    .map(|offset| FilterSpec::Affine { gain, offset })
            }),
            _ => parse(self.filter_cutoff.get_untracked()).map(FilterSpec::LowPass),
        };
        match spec.and_then(|spec| {
            let editing = self.filter_edit.get_untracked();
            let outputs = self.filter_outputs.get_untracked();
            let fields = self.derived.get_untracked();
            let edits = self
                .filter_inputs
                .get_untracked()
                .into_iter()
                .map(|input| {
                    let id = editing.or_else(|| {
                        outputs
                            .iter()
                            .find(|id| {
                                fields
                                    .iter()
                                    .any(|field| field.id == **id && field.source == input.source)
                            })
                            .copied()
                    });
                    (id, input)
                })
                .collect();
            self.apply_named_filters(edits, spec)
        }) {
            Ok(ids) => {
                self.filter_edit.set(if ids.len() == 1 {
                    ids.first().copied()
                } else {
                    None
                });
                self.filter_outputs.set(ids);
                self.filter_error.set(String::new());
            }
            Err(error) => self.filter_error.set(error),
        }
    }
    #[cfg(test)]
    pub fn upsert_filter(
        self,
        editing: Option<u64>,
        name: String,
        source: String,
        filter: FilterSpec,
    ) -> Result<u64, String> {
        self.apply_filter_batch(
            editing,
            vec![crate::derived::FilterInput { source, name }],
            filter,
        )
        .map(|ids| ids[0])
    }
    #[cfg(test)]
    pub fn apply_filter_batch(
        self,
        editing: Option<u64>,
        inputs: Vec<crate::derived::FilterInput>,
        filter: FilterSpec,
    ) -> Result<Vec<u64>, String> {
        if editing.is_some() && inputs.len() != 1 {
            return Err("Edit one field at a time, or start a new batch".into());
        }
        self.apply_named_filters(
            inputs.into_iter().map(|input| (editing, input)).collect(),
            filter,
        )
    }
    fn apply_named_filters(
        self,
        edits: Vec<(Option<u64>, crate::derived::FilterInput)>,
        filter: FilterSpec,
    ) -> Result<Vec<u64>, String> {
        if self.loading.get_untracked() {
            return Err("Wait for log to load".into());
        }
        if edits.is_empty() {
            return Err("Drag source fields into source area first".into());
        }
        let log = self.log.get_untracked().ok_or("Open a log first")?;
        let mut fields = self.derived.get_untracked();
        let mut ids = Vec::new();
        for (editing, input) in edits {
            let id = editing.unwrap_or_else(|| self.id());
            if editing.is_some() && !fields.iter().any(|field| field.id == id) {
                return Err("Derived field no longer exists".into());
            }
            let mut field = DerivedField {
                id,
                name: input.name.trim().into(),
                source: input.source,
                filter,
                data: None,
            };
            if let Some(existing) = fields.iter_mut().find(|field| field.id == id) {
                if existing.source == field.source && existing.filter == field.filter {
                    field.data = existing.data.clone();
                }
                *existing = field;
            } else {
                fields.push(field);
            }
            ids.push(id);
        }
        derived::validate(&fields, &log)?;
        let changed: Vec<_> = fields
            .iter()
            .filter(|field| field.data.is_none())
            .map(|field| field.id)
            .collect();
        let invalid: Vec<_> = fields
            .iter()
            .filter(|field| {
                changed
                    .iter()
                    .any(|id| derived::depends_on(&fields, &field.source, *id))
            })
            .map(|field| field.id)
            .collect();
        for field in &mut fields {
            if invalid.contains(&field.id) {
                field.data = None;
            }
        }
        self.derived.set(fields.clone());
        self.recompute_filters(fields);
        self.expanded.update(|expanded| {
            expanded.insert("@derived".into());
        });
        Ok(ids)
    }
    fn recompute_filters(self, fields: Vec<DerivedField>) {
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        self.filter_generation.update(|generation| *generation += 1);
        let revision = self.filter_generation.get_untracked();
        let cache = self.cache.get_untracked();
        self.derived_busy.set(true);
        self.task(async move {
            let result = blocking(move || derived::compute(fields, &log, cache)).await;
            if self.filter_generation.get_untracked() != revision {
                return;
            }
            self.derived_busy.set(false);
            match result {
                Ok(fields) => {
                    for pane in self.panes.get_untracked() {
                        pane.lines.update(|lines| {
                            for line in lines {
                                if let Some(field) =
                                    fields.iter().find(|field| field.key() == line.source)
                                {
                                    let data = field.data.as_ref().expect("computed field");
                                    if line.name != field.name
                                        || line.data.samples().as_ptr() != data.samples().as_ptr()
                                    {
                                        line.name = field.name.clone();
                                        line.data = data.clone();
                                        line.revision += 1;
                                    }
                                }
                            }
                        });
                        pane.plot.hover.set(None);
                    }
                    self.derived.set(fields);
                    if !self.loading.get_untracked() {
                        self.status.set("Derived fields updated".into());
                    }
                }
                Err(error) => {
                    self.filter_error.set(error.clone());
                    self.status.set(error);
                }
            }
        });
    }
    pub fn delete_filter(self, id: u64) -> Result<(), String> {
        if self
            .derived
            .with_untracked(|fields| fields.iter().any(|field| field.source == derived::key(id)))
        {
            return Err("Remove dependent fields first".into());
        }
        self.derived
            .update(|fields| fields.retain(|field| field.id != id));
        self.selected_fields.update(|fields| {
            fields.remove(&derived::key(id));
        });
        self.filter_outputs
            .update(|outputs| outputs.retain(|output| *output != id));
        for pane in self.panes.get_untracked() {
            let ids: Vec<_> = pane.lines.with_untracked(|lines| {
                lines
                    .iter()
                    .filter(|line| line.source == derived::key(id))
                    .map(|line| line.id)
                    .collect()
            });
            for id in ids {
                pane.remove_line(id);
            }
        }
        if self.selected_field.get_untracked().as_deref() == Some(&derived::key(id)) {
            self.selected_field.set(None);
        }
        self.filter_edit.set(None);
        self.recompute_filters(self.derived.get_untracked());
        Ok(())
    }
}
