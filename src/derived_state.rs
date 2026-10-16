use crate::{
    derived::{self, DerivedField, FilterSpec, RpyAxis},
    filter_mode::FilterMode,
    state::Workbench,
};
use zgui::prelude::*;

impl Workbench {
    pub fn finish_filter_editing(self) {
        self.cancel_filter_input_drag();
        self.filter_mode.set(FilterMode::New);
        self.script_output_ids.set(Vec::new());
        self.filter_inputs.set(Vec::new());
        self.filter_outputs.set(Vec::new());
        self.filter_error.set(String::new());
        self.reset_filter_names();
    }
    pub fn toggle_filter_editing(self, id: u64) {
        if self
            .filter_mode
            .with_untracked(|mode| mode.ids().contains(&id))
        {
            self.finish_filter_editing();
        } else {
            self.edit_filter(Some(id), None);
        }
    }
    pub fn edit_created_filters(self) {
        let ids = self.filter_outputs.get_untracked();
        let fields: Vec<_> = self.derived.with_untracked(|fields| {
            ids.iter()
                .filter_map(|id| fields.iter().find(|field| field.id == *id).cloned())
                .collect()
        });
        let Some(first) = fields.first() else {
            return;
        };
        self.edit_filter(Some(first.id), None);
        if fields.len() > 1
            && fields
                .iter()
                .all(|field| field.sources.len() == 1 && field.filter == first.filter)
        {
            self.filter_inputs.set(
                fields
                    .iter()
                    .map(|field| crate::derived::FilterInput {
                        source: field.sources[0].clone(),
                        name: field.name.clone(),
                    })
                    .collect(),
            );
            self.filter_mode.set(FilterMode::Edit {
                ids: ids.clone(),
                selected: first.id,
            });
            self.filter_outputs.set(ids);
        }
    }
    pub fn remove_saved_filter(self, id: u64) {
        let mode = self.filter_mode.get_untracked();
        match self.delete_filter(id) {
            Ok(()) if mode.ids().contains(&id) => {
                if let Some(&next) = mode.ids().iter().find(|&&other| other != id) {
                    self.edit_filter(Some(next), None);
                } else {
                    self.finish_filter_editing();
                }
            }
            Ok(()) => {}
            Err(error) => self.filter_error.set(error),
        }
    }
    pub fn filter_legend(self, pane: crate::state::Pane) {
        let sources = pane
            .lines
            .with_untracked(|lines| lines.iter().map(|line| line.source.clone()).collect());
        self.edit_filter(None, None);
        if let Err(error) = self.accept_filter_sources(sources) {
            self.filter_error.set(error);
        }
    }
    pub fn field_name(self, source: &str) -> String {
        self.derived.with(|fields| {
            fields
                .iter()
                .find(|field| field.key() == source)
                .map_or_else(|| source.to_owned(), |field| field.name.clone())
        })
    }
    pub fn edit_filter(self, id: Option<u64>, source: Option<String>) {
        self.cancel_filter_input_drag();
        self.reset_filter_names();
        self.filter_mode.set(FilterMode::New);
        self.script_output_ids.set(Vec::new());
        self.filter_outputs.set(id.into_iter().collect());
        if let Some(field) = self
            .derived
            .with_untracked(|fields| fields.iter().find(|field| Some(field.id) == id).cloned())
        {
            self.filter_inputs.set(
                field
                    .sources
                    .iter()
                    .map(|source| crate::derived::FilterInput {
                        source: source.clone(),
                        name: if field.sources.len() == 1 {
                            field.name.clone()
                        } else {
                            format!("{} filtered", self.field_name(source))
                        },
                    })
                    .collect(),
            );
            match &field.filter {
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
                FilterSpec::Rust(script, _) => {
                    self.filter_kind.set("rust".into());
                    self.script_source.set(script.source.clone());
                    let mut group: Vec<_> = self.derived.with_untracked(|fields| {
                        fields
                            .iter()
                            .filter(|node| {
                                node.group == field.group
                                    && matches!(node.filter, FilterSpec::Rust(..))
                            })
                            .cloned()
                            .collect()
                    });
                    group.sort_by_key(|node| match node.filter {
                        FilterSpec::Rust(_, index) => index,
                        _ => usize::MAX,
                    });
                    self.script_output_ids
                        .set(group.iter().map(|node| Some(node.id)).collect());
                    self.script_names.set(
                        group
                            .iter()
                            .map(|node| node.name.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    self.filter_outputs
                        .set(group.iter().map(|node| node.id).collect());
                }
                FilterSpec::Magnitude => {
                    self.filter_kind.set("magnitude".into());
                    self.filter_names
                        .update(|names| names[0] = field.name.clone());
                }
                FilterSpec::Quaternion(_, unit) => {
                    self.filter_kind.set("rpy".into());
                    self.filter_angle_unit.set(unit.symbol().into());
                    let group: Vec<_> = self.derived.with_untracked(|fields| {
                        fields
                            .iter()
                            .filter(|node| {
                                node.group == field.group
                                    && matches!(node.filter, FilterSpec::Quaternion(..))
                            })
                            .cloned()
                            .collect()
                    });
                    self.filter_outputs
                        .set(group.iter().map(|node| node.id).collect());
                    self.filter_names.update(|names| {
                        for node in group {
                            if let FilterSpec::Quaternion(axis, _) = node.filter {
                                let index = RpyAxis::ALL
                                    .iter()
                                    .position(|&candidate| candidate == axis)
                                    .unwrap();
                                names[index + 1] = node.name;
                            }
                        }
                    });
                }
                FilterSpec::Affine { gain, offset } => {
                    self.filter_kind.set("affine".into());
                    self.filter_gain.set(gain.to_string());
                    self.filter_offset.set(offset.to_string());
                }
            }
            self.filter_mode.set(FilterMode::Edit {
                ids: self.filter_outputs.get_untracked(),
                selected: field.id,
            });
        } else {
            self.filter_inputs.set(Vec::new());
            self.filter_kind.set("low".into());
            self.filter_angle_unit.set("rad".into());
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
        self.filter_mode.set(FilterMode::New);
        self.script_output_ids.set(Vec::new());
        self.filter_outputs.set(Vec::new());
        self.filter_inputs.set(Vec::new());
        self.cancel_filter_input_drag();
        self.reset_filter_names();
        if self.filter_kind.get_untracked() == "rpy" {
            self.filter_kind.set("low".into());
        }
        if let Err(error) = self.accept_filter_sources(sources) {
            self.filter_error.set(error);
        }
    }
    pub fn combined_filter(self) -> bool {
        self.filter_kind
            .with(|kind| matches!(kind.as_str(), "magnitude" | "rpy" | "rust"))
    }
    pub fn combined_filter_untracked(self) -> bool {
        self.filter_kind
            .with_untracked(|kind| matches!(kind.as_str(), "magnitude" | "rpy" | "rust"))
    }
    pub fn save_filter(self) {
        if self.script_busy.get_untracked() || self.derived_busy.get_untracked() {
            return;
        }
        if self.filter_kind.get_untracked() == "rust" {
            self.run_script(false);
            return;
        }
        if self.filter_inputs.with_untracked(Vec::is_empty) {
            return;
        }
        let result = if self.combined_filter_untracked() {
            self.apply_combined_filter()
        } else {
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
            spec.and_then(|spec| {
                let inputs = self.filter_inputs.get_untracked();
                let mode = self.filter_mode.get_untracked();
                let editing = mode
                    .selected()
                    .filter(|_| mode.ids().len() == 1 && inputs.len() == 1);
                let outputs = mode.ids();
                let fields = self.derived.get_untracked();
                if outputs.len() > 1
                    && fields.iter().any(|field| {
                        outputs.contains(&field.id) && matches!(field.filter, FilterSpec::Rust(..))
                    })
                {
                    return Err(
                        "Finish editing to turn Rust outputs into a built-in filter batch".into(),
                    );
                }
                if !mode.is_new() && outputs.len() == 1 && inputs.len() != 1 {
                    return Err("Finish editing to create a batch from multiple inputs".into());
                }
                if !mode.is_new()
                    && fields
                        .iter()
                        .any(|field| outputs.contains(&field.id) && field.sources.len() != 1)
                {
                    return Err("Finish editing to turn combined inputs into a batch filter".into());
                }
                let edits = inputs
                    .into_iter()
                    .map(|input| {
                        let id = editing.or_else(|| {
                            outputs
                                .iter()
                                .find(|id| {
                                    fields.iter().any(|field| {
                                        field.id == **id && field.sources == [input.source.clone()]
                                    })
                                })
                                .copied()
                        });
                        (id, input)
                    })
                    .collect();
                self.apply_named_filters(edits, spec)
            })
        };
        match result {
            Ok(ids) => {
                if self.filter_mode.get_untracked().is_new() {
                    self.filter_inputs.set(Vec::new());
                    self.reset_filter_names();
                } else {
                    let selected = self
                        .filter_mode
                        .get_untracked()
                        .selected()
                        .filter(|id| ids.contains(id))
                        .unwrap_or(ids[0]);
                    self.filter_mode.set(FilterMode::Edit {
                        ids: ids.clone(),
                        selected,
                    });
                }
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
    pub(crate) fn recompute_filters(self, fields: Vec<DerivedField>) {
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        self.cancel_filter_run();
        let cancel = self.filter_cancel.get_untracked();
        self.filter_generation.update(|generation| *generation += 1);
        let revision = self.filter_generation.get_untracked();
        let cache = self.cache.get_untracked();
        self.derived_busy.set(true);
        self.task(async move {
            let result =
                blocking(move || derived::compute_cancelled(fields, &log, cache, cancel)).await;
            if self.filter_generation.get_untracked() != revision {
                return;
            }
            self.derived_busy.set(false);
            match result {
                Ok(fields) => {
                    self.publish_filter_fields(fields);
                }
                Err(error) => {
                    self.filter_error.set(error.clone());
                    self.status.set(error);
                }
            }
        });
    }
    pub(crate) fn publish_filter_fields(self, fields: Vec<DerivedField>) {
        for pane in self.panes.get_untracked() {
            pane.lines.update(|lines| {
                for line in lines {
                    if let Some(field) = fields.iter().find(|field| field.key() == line.source) {
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
    pub fn delete_filter(self, id: u64) -> Result<(), String> {
        if self.derived.with_untracked(|fields| {
            fields
                .iter()
                .any(|field| field.sources.contains(&derived::key(id)))
        }) {
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
        if self
            .filter_mode
            .with_untracked(|mode| mode.ids().contains(&id))
        {
            self.filter_mode.set(FilterMode::New);
            self.script_output_ids.set(Vec::new());
        }
        self.recompute_filters(self.derived.get_untracked());
        self.filter_error.set(String::new());
        Ok(())
    }
}
