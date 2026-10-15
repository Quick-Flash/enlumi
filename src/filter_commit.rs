use crate::{
    derived::{self, AngleUnit, DerivedField, FilterInput, FilterSpec, RpyAxis},
    state::Workbench,
};
use zgui::prelude::*;

impl Workbench {
    pub fn reset_filter_names(self) {
        let fields = self.derived.get_untracked();
        self.filter_names.set(
            ["Magnitude", "Roll", "Pitch", "Yaw"]
                .map(|stem| {
                    let mut name = stem.to_owned();
                    let mut suffix = 2;
                    while fields
                        .iter()
                        .any(|field| field.name.eq_ignore_ascii_case(&name))
                    {
                        name = format!("{stem} {suffix}");
                        suffix += 1;
                    }
                    name
                })
                .to_vec(),
        );
    }
    pub fn move_filter_input(self, source: &str, offset: isize) {
        if self.filter_kind.get_untracked() != "rpy" {
            return;
        }
        self.cancel_filter_input_drag();
        self.filter_inputs.update(|inputs| {
            if let Some(index) = inputs.iter().position(|input| input.source == source)
                && let Some(next) = index
                    .checked_add_signed(offset)
                    .filter(|&next| next < inputs.len())
            {
                inputs.swap(index, next);
            }
        });
    }
    pub(crate) fn apply_named_filters(
        self,
        edits: Vec<(Option<u64>, FilterInput)>,
        filter: FilterSpec,
    ) -> Result<Vec<u64>, String> {
        let fields = self.derived.get_untracked();
        if edits.iter().any(|(editing, _)| {
            editing.is_some_and(|id| !fields.iter().any(|field| field.id == id))
        }) {
            return Err("Derived field no longer exists".into());
        }
        let edits = edits
            .into_iter()
            .map(|(editing, input)| {
                let id = editing.unwrap_or_else(|| self.id());
                DerivedField {
                    id,
                    name: input.name.trim().into(),
                    sources: vec![input.source],
                    group: id,
                    filter,
                    data: None,
                }
            })
            .collect();
        self.commit_filter_fields(edits)
    }
    pub(crate) fn apply_combined_filter(self) -> Result<Vec<u64>, String> {
        let sources: Vec<_> = self
            .filter_inputs
            .get_untracked()
            .into_iter()
            .map(|input| input.source)
            .collect();
        let rpy = self.filter_kind.get_untracked() == "rpy";
        if rpy && (sources.len() != 4 || sources.iter().any(String::is_empty)) {
            return Err("Quaternion needs exactly four inputs: fill W, X, Y, Z".into());
        }
        let names = self.filter_names.get_untracked();
        let specs: Vec<_> = if rpy {
            let unit = if self.filter_angle_unit.get_untracked() == "deg" {
                AngleUnit::Degrees
            } else {
                AngleUnit::Radians
            };
            RpyAxis::ALL
                .into_iter()
                .map(|axis| FilterSpec::Quaternion(axis, unit))
                .collect()
        } else {
            vec![FilterSpec::Magnitude]
        };
        let fields = self.derived.get_untracked();
        let mode = self.filter_mode.get_untracked();
        let outputs = mode.ids();
        let group = outputs.iter().find_map(|id| {
            fields
                .iter()
                .find(|field| {
                    field.id == *id
                        && matches!(
                            (rpy, field.filter),
                            (true, FilterSpec::Quaternion(..)) | (false, FilterSpec::Magnitude)
                        )
                })
                .map(|field| field.group)
        });
        let group = if !rpy && outputs.len() == 1 {
            fields
                .iter()
                .find(|field| field.id == outputs[0])
                .map(|field| field.group)
        } else {
            group
        };
        if !mode.is_new() && group.is_none() {
            return Err("Finish editing to create a different output group".into());
        }
        let mut edits: Vec<_> = specs
            .into_iter()
            .enumerate()
            .map(|(index, filter)| {
                let id = fields
                    .iter()
                    .find(|field| {
                        Some(field.group) == group
                            && (!rpy && outputs.contains(&field.id)
                                || match (field.filter, filter) {
                                    (
                                        FilterSpec::Quaternion(a, _),
                                        FilterSpec::Quaternion(b, _),
                                    ) => a == b,
                                    (a, b) => a == b,
                                })
                    })
                    .map_or_else(|| self.id(), |field| field.id);
                DerivedField {
                    id,
                    name: names[if rpy { index + 1 } else { 0 }].trim().into(),
                    sources: sources.clone(),
                    group: group.unwrap_or(0),
                    filter,
                    data: None,
                }
            })
            .collect();
        let group = group.unwrap_or(edits[0].id);
        for field in &mut edits {
            field.group = group;
        }
        self.commit_filter_fields(edits)
    }
    fn commit_filter_fields(self, edits: Vec<DerivedField>) -> Result<Vec<u64>, String> {
        if self.loading.get_untracked() {
            return Err("Wait for log to load".into());
        }
        if edits.is_empty() {
            return Err("Drag source fields into source area first".into());
        }
        let log = self.log.get_untracked().ok_or("Open a log first")?;
        let mut fields = self.derived.get_untracked();
        let ids = edits.iter().map(|field| field.id).collect();
        for mut field in edits {
            if let Some(existing) = fields.iter_mut().find(|existing| existing.id == field.id) {
                if existing.sources == field.sources && existing.filter == field.filter {
                    field.data = existing.data.clone();
                }
                *existing = field;
            } else {
                fields.push(field);
            }
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
                field.sources.iter().any(|source| {
                    changed
                        .iter()
                        .any(|id| derived::depends_on(&fields, source, *id))
                })
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
}
