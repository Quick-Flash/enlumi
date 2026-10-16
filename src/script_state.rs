use crate::{
    derived::{self, DerivedField, FilterSpec},
    filter_mode::FilterMode,
    script::Script,
    state::Workbench,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zgui::prelude::*;

pub fn output_names(text: &str) -> Result<Vec<String>, String> {
    let names: Vec<String> = text
        .split('\n')
        .map(|line| line.trim().to_owned())
        .collect();
    if names.is_empty() || names.len() > 16 || names.iter().any(String::is_empty) {
        return Err("Give each output row a name (1–16 outputs)".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    if names.iter().any(|name| !seen.insert(name.to_lowercase())) {
        return Err("Output names must be unique".into());
    }
    Ok(names)
}

impl Workbench {
    fn forget_script_output(self, id: u64) {
        let key = derived::key(id);
        self.selected_fields.update(|fields| {
            fields.remove(&key);
        });
        if self.selected_field.get_untracked().as_deref() == Some(&key) {
            self.selected_field.set(None);
        }
        for pane in self.panes.get_untracked() {
            let ids: Vec<_> = pane.lines.with_untracked(|lines| {
                lines
                    .iter()
                    .filter(|line| line.source == key)
                    .map(|line| line.id)
                    .collect()
            });
            for id in ids {
                pane.remove_line(id);
            }
        }
    }
    pub fn cancel_filter_run(self) {
        self.filter_cancel
            .get_untracked()
            .store(true, Ordering::Relaxed);
        self.filter_cancel.set(Arc::new(AtomicBool::new(false)));
        self.filter_generation.update(|revision| *revision += 1);
        self.derived_busy.set(false);
        self.script_busy.set(false);
    }
    pub fn cancel_script(self) {
        self.cancel_filter_run();
        self.filter_error.set("Filter run cancelled".into());
    }
    pub fn run_script(self, check_only: bool) {
        if self.loading.get_untracked()
            || self.derived_busy.get_untracked()
            || self.script_busy.get_untracked()
        {
            return;
        }
        let names_text = self.script_names.get_untracked();
        let names = match output_names(&names_text) {
            Ok(names) => names,
            Err(error) => {
                self.filter_error.set(error);
                return;
            }
        };
        let source = self.script_source.get_untracked();
        let script = Arc::new(Script {
            source: source.clone(),
            outputs: names.len(),
        });
        let inputs = self.filter_inputs.get_untracked();
        if let Err(error) = script.validate(if check_only {
            inputs.len().max(1)
        } else {
            inputs.len()
        }) {
            self.filter_error.set(error);
            return;
        }
        let row_ids = self.script_output_ids.get_untracked();
        let mode = self.filter_mode.get_untracked();
        let log = self.log.get_untracked();
        let mut fields = self.derived.get_untracked();
        let ids = if check_only {
            Vec::new()
        } else {
            if log.is_none() {
                self.filter_error.set("Open a log first".into());
                return;
            }
            if !mode.is_new()
                && mode.ids().iter().any(|id| {
                    !fields.iter().any(|field| {
                        field.id == *id && matches!(field.filter, FilterSpec::Rust(..))
                    })
                })
            {
                self.filter_error
                    .set("Finish editing to convert a built-in filter".into());
                return;
            }
            let rows = self.script_output_ids.get_untracked();
            let ids: Vec<_> = (0..names.len())
                .map(|index| {
                    if mode.is_new() {
                        self.id()
                    } else {
                        rows.get(index)
                            .copied()
                            .flatten()
                            .unwrap_or_else(|| self.id())
                    }
                })
                .collect();
            let removed: Vec<_> = mode
                .ids()
                .iter()
                .filter(|id| !ids.contains(id))
                .copied()
                .collect();
            if let Some(dependent) = fields.iter().find(|field| {
                !removed.contains(&field.id)
                    && field
                        .sources
                        .iter()
                        .any(|source| removed.iter().any(|id| source == &derived::key(*id)))
            }) {
                self.filter_error.set(format!("Cannot remove output: {} depends on it. Remove or change dependent field first.", dependent.name));
                return;
            }
            fields.retain(|field| !removed.contains(&field.id));
            for (index, (name, id)) in names.into_iter().zip(&ids).enumerate() {
                let candidate = DerivedField {
                    id: *id,
                    name,
                    sources: inputs.iter().map(|input| input.source.clone()).collect(),
                    group: ids[0],
                    filter: FilterSpec::Rust(script.clone(), index),
                    data: None,
                };
                if let Some(field) = fields.iter_mut().find(|field| field.id == *id) {
                    *field = candidate;
                } else {
                    fields.push(candidate);
                }
            }
            let invalid: Vec<_> = fields
                .iter()
                .filter(|field| {
                    ids.iter()
                        .any(|id| derived::depends_on(&fields, &field.key(), *id))
                })
                .map(|field| field.id)
                .collect();
            for field in &mut fields {
                if invalid.contains(&field.id) {
                    field.data = None;
                }
            }
            if let Err(error) = derived::validate(&fields, log.as_ref().expect("checked log")) {
                self.filter_error.set(error);
                return;
            }
            ids
        };
        self.cancel_filter_run();
        let revision = self.filter_generation.get_untracked();
        let cancel = self.filter_cancel.get_untracked();
        let cache = self.cache.get_untracked();
        self.script_busy.set(true);
        self.derived_busy.set(!check_only);
        self.filter_error.set(String::new());
        self.status.set(
            if check_only {
                "Checking Rust filter…"
            } else {
                "Compiling / running Rust filter…"
            }
            .into(),
        );
        self.task(async move {
            let result = blocking(move || {
                if check_only {
                    crate::script_compile::compile(&script.source, &cancel).map(|_| Vec::new())
                } else {
                    derived::compute_cancelled(fields, &log.expect("checked log"), cache, cancel)
                }
            })
            .await;
            if self.filter_generation.get_untracked() != revision {
                return;
            }
            self.script_busy.set(false);
            self.derived_busy.set(false);
            if self.script_source.get_untracked() != source
                || self.script_names.get_untracked() != names_text
                || self.filter_inputs.get_untracked() != inputs
                || self.script_output_ids.get_untracked() != row_ids
                || self.filter_mode.get_untracked() != mode
                || self.filter_kind.get_untracked() != "rust"
            {
                self.filter_error
                    .set("Editor changed during run; result discarded. Run again.".into());
                self.status.set("Rust result discarded".into());
                return;
            }
            match result {
                Ok(_) if check_only => {
                    self.filter_error.set(String::new());
                    self.status.set("Rust filter compiled successfully".into());
                }
                Ok(fields) => {
                    for id in mode.ids().iter().filter(|id| !ids.contains(id)) {
                        self.forget_script_output(*id);
                    }
                    self.publish_filter_fields(fields);
                    if mode.is_new() {
                        self.filter_inputs.set(Vec::new());
                    } else {
                        self.filter_mode.set(FilterMode::Edit {
                            selected: mode
                                .selected()
                                .filter(|id| ids.contains(id))
                                .unwrap_or(ids[0]),
                            ids: ids.clone(),
                        });
                    }
                    self.script_output_ids
                        .set(ids.iter().map(|id| Some(*id)).collect());
                    self.filter_outputs.set(ids);
                    self.expanded.update(|expanded| {
                        expanded.insert("@derived".into());
                    });
                    self.filter_error.set(String::new());
                }
                Err(error) => {
                    self.filter_error.set(error.clone());
                    self.status.set(error);
                }
            }
        });
    }
}
