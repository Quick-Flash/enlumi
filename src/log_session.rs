//! Refresh a live session against another log without rebuilding its layout.
use crate::{
    analysis_state::AnalysisResult,
    derived::{self, DerivedField, EvaluationCache},
    geometry::Geometry,
    log::LogStore,
    profile::{MetadataMap, identity::ProfileKey},
    profile_instance::AppliedProfile,
    state::{Line, Workbench},
};
use plot::time_series::{SeriesData, TimeSeries, TimeSeriesSet};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, atomic::AtomicBool},
};
use zgui::prelude::*;

struct Plot {
    id: u64,
    lines: Vec<Line>,
    analysis: Option<AnalysisResult>,
}

pub(crate) struct Session {
    ready_workspaces: BTreeSet<u64>,
    old_log: Arc<LogStore>,
    fields: Vec<DerivedField>,
    geometry: Vec<Geometry>,
    profiles: BTreeMap<ProfileKey, AppliedProfile>,
    plots: Vec<Plot>,
}

pub(crate) struct Refreshed {
    ready_workspaces: BTreeSet<u64>,
    fields: Vec<DerivedField>,
    geometry: Vec<Geometry>,
    profiles: BTreeMap<ProfileKey, AppliedProfile>,
    plots: Vec<Plot>,
    cache: BTreeMap<String, SeriesData>,
    mapping: BTreeMap<String, Option<String>>,
    warnings: Vec<String>,
}

struct Data {
    old_log: Arc<LogStore>,
    log: Arc<LogStore>,
    mapping: BTreeMap<String, Option<String>>,
    evaluation: EvaluationCache,
}

impl Data {
    fn source(&mut self, source: &str) -> Result<String, String> {
        if let Some(mapped) = self.mapping.get(source) {
            return mapped
                .clone()
                .ok_or_else(|| format!("Parameter {source} unavailable"));
        }
        let mapped = if source.starts_with("@setting/") {
            // Parameter indices are file-local. Match name and occurrence instead.
            let parts = source.split('/').collect::<Vec<_>>();
            let flight = parts
                .get(1)
                .and_then(|p| p.parse::<usize>().ok())
                .ok_or("Invalid parameter source")?;
            let index = parts
                .get(2)
                .and_then(|p| p.parse::<usize>().ok())
                .ok_or("Invalid parameter source")?;
            let old = self
                .old_log
                .settings
                .get(&flight)
                .ok_or("Parameter flight unavailable")?;
            let setting = old.get(index).ok_or("Parameter unavailable")?;
            let occurrence = old[..index]
                .iter()
                .filter(|entry| entry.name == setting.name)
                .count();
            let flight = if self.log.settings.contains_key(&flight) {
                flight
            } else {
                *self
                    .log
                    .settings
                    .keys()
                    .next()
                    .ok_or("Parameters unavailable")?
            };
            let index = self.log.settings[&flight]
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name == setting.name)
                .nth(occurrence)
                .map(|(index, _)| index)
                .ok_or_else(|| format!("Parameter {} unavailable", setting.name))?;
            let element = parts
                .get(3)
                .map_or(String::new(), |part| format!("/{part}"));
            format!("@setting/{flight}/{index}{element}")
        } else {
            source.to_owned()
        };
        self.mapping.insert(source.to_owned(), Some(mapped.clone()));
        Ok(mapped)
    }

    fn current(&mut self, source: &str) -> Result<SeriesData, String> {
        if let Some(data) = self.evaluation.series().get(source) {
            return Ok(data.clone());
        }
        let data = self.log.prepare(source)?;
        self.evaluation
            .insert_series(source.to_owned(), data.clone());
        Ok(data)
    }

    fn read(&mut self, source: &str) -> Result<(String, SeriesData), String> {
        let source = self.source(source)?;
        let data = self.current(&source)?;
        Ok((source, data))
    }
}

impl Session {
    pub(crate) fn capture(workbench: Workbench, unfinished: Option<u64>) -> Option<Self> {
        Some(Self {
            ready_workspaces: workbench.workspaces.items.with_untracked(|items| {
                items
                    .iter()
                    .filter(|workspace| {
                        Some(workspace.id) != unfinished
                            && (workspace.source.with_untracked(Option::is_none)
                                || workspace.generation
                                    == workbench.loaded_generation.get_untracked())
                    })
                    .map(|workspace| workspace.id)
                    .collect()
            }),
            old_log: workbench.log.get_untracked()?,
            fields: workbench.derived.get_untracked(),
            geometry: workbench.geometry.items.get_untracked(),
            profiles: workbench.profiles.instances.get_untracked(),
            plots: workbench
                .all_panes()
                .into_iter()
                .map(|pane| Plot {
                    id: pane.id,
                    lines: pane.lines.get_untracked(),
                    analysis: pane.analysis.get_untracked(),
                })
                .collect(),
        })
    }

    pub(crate) fn refresh(self, log: Arc<LogStore>) -> Refreshed {
        let mut data = Data {
            old_log: self.old_log,
            log,
            mapping: BTreeMap::new(),
            evaluation: EvaluationCache::default(),
        };
        let mut warnings = Vec::new();
        let parameters = data
            .old_log
            .catalogue
            .iter()
            .filter(|field| field.id.starts_with("@setting/"))
            .map(|field| field.id.clone())
            .collect::<Vec<_>>();
        for source in parameters {
            if data.source(&source).is_err() {
                data.mapping.insert(source, None);
            }
        }
        let mut profiles = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let existing = self
            .fields
            .iter()
            .map(|field| field.id)
            .collect::<BTreeSet<_>>();
        let mut owned = BTreeMap::new();
        for old in self.profiles.values() {
            for field in &old.fields {
                owned.insert(field.id, field);
            }
            let base = old.key.path.parent().unwrap_or(std::path::Path::new("."));
            match crate::profile::prepare::prepare_with_ids(
                old.definition.clone(),
                &data.log,
                base,
                old.ids.clone(),
                Arc::new(AtomicBool::new(false)),
            ) {
                Ok(prepared) => {
                    let mut instance = AppliedProfile::from_prepared(
                        old.key.clone(),
                        old.definition.clone(),
                        prepared,
                    );
                    instance.placements = old.placements.clone();
                    for (token, source) in &old.symbols {
                        // Field identifiers remain stable; raw parameter identifiers may move.
                        if derived::id(source).is_none()
                            && let Some(mapped) = instance.symbols.get(token)
                        {
                            data.mapping.insert(source.clone(), Some(mapped.clone()));
                        }
                    }
                    for field in &instance.fields {
                        if existing.contains(&field.id) {
                            fields.insert(field.id, field.clone());
                        }
                    }
                    profiles.insert(instance.key.clone(), instance);
                }
                Err(error) => warnings.push(format!("{}: {error}", old.name)),
            }
        }
        for mut field in self.fields {
            if owned
                .get(&field.id)
                .is_some_and(|old| old.sources == field.sources && old.filter == field.filter)
            {
                if let Some(updated) = fields.get_mut(&field.id) {
                    updated.name = field.name;
                }
                continue;
            }
            fields.remove(&field.id);
            let sources = field
                .sources
                .iter()
                .map(|source| data.source(source))
                .collect::<Result<Vec<_>, _>>();
            match sources {
                Ok(sources) => {
                    field.sources = sources;
                    fields.insert(field.id, field);
                }
                Err(error) => warnings.push(format!("{}: {error}", field.name)),
            }
        }
        let mut pending = fields.into_values().collect::<Vec<_>>();
        for field in &mut pending {
            field.data = None;
        }
        let mut fields = Vec::new();
        while !pending.is_empty() {
            let waiting = pending
                .iter()
                .map(|field| field.id)
                .collect::<BTreeSet<_>>();
            let before = pending.len();
            let mut deferred = Vec::new();
            for mut field in pending {
                if field
                    .sources
                    .iter()
                    .filter_map(|source| derived::id(source))
                    .any(|id| waiting.contains(&id))
                {
                    deferred.push(field);
                    continue;
                }
                let result = field
                    .sources
                    .iter()
                    .try_for_each(|source| data.current(source).map(|_| ()))
                    .and_then(|()| {
                        derived::evaluate_field(
                            &mut field,
                            &mut data.evaluation,
                            Arc::new(AtomicBool::new(false)),
                        )
                    });
                match result {
                    Ok(()) => fields.push(field),
                    Err(error) => warnings.push(format!("{}: {error}", field.name)),
                }
            }
            if deferred.len() == before {
                warnings.extend(
                    deferred
                        .iter()
                        .map(|field| format!("{}: filter dependency unavailable", field.name)),
                );
                break;
            }
            pending = deferred;
        }
        let mut geometry = Vec::new();
        for mut item in self.geometry {
            if let Some(old) = &item.states {
                let refreshed = data.read(&old.source).and_then(|(source, input)| {
                    crate::state_bands::StateBands::build(source, input, data.log.duration)
                });
                match refreshed {
                    Ok(mut states) => {
                        states.preserve_rules(old);
                        item.states = Some(Arc::new(states));
                    }
                    Err(error) => {
                        warnings.push(format!("{}: {error}", item.name));
                        continue;
                    }
                }
            }
            geometry.push(item);
        }
        let mut plots = Vec::new();
        for mut plot in self.plots {
            if let Some(mut analysis) = plot.analysis.take() {
                let sources = if analysis.mode.windowed() {
                    analysis
                        .series
                        .series()
                        .iter()
                        .map(|series| (series.id().to_owned(), series.label().to_owned()))
                        .collect::<Vec<_>>()
                } else {
                    plot.lines
                        .iter()
                        .filter_map(|line| {
                            line.source
                                .strip_prefix("@spectrum/")
                                .map(|source| (source.to_owned(), line.name.clone()))
                        })
                        .collect()
                };
                let mut series = Vec::<TimeSeries>::new();
                let conditioning = if analysis.condition.is_empty() {
                    None
                } else {
                    data.read(&analysis.condition).ok().map(|(source, input)| {
                        analysis.condition = source;
                        input
                    })
                };
                analysis.options.start = None;
                analysis.options.end = None;
                let mut heatmap = None;
                for (source, name) in sources {
                    let result = data.read(&source).and_then(|(source, input)| {
                        let spectrum = crate::spectral::compute(
                            &input,
                            conditioning.as_ref(),
                            &analysis.options,
                            &AtomicBool::new(false),
                        )?;
                        heatmap = spectrum.heatmap;
                        Ok(TimeSeries::new(
                            source,
                            name.clone(),
                            SeriesData::new(spectrum.points)?,
                        ))
                    });
                    match result {
                        Ok(result) => {
                            if !series.iter().any(|existing| existing.id() == result.id()) {
                                series.push(result);
                            }
                        }
                        Err(error) => warnings.push(format!("{name}: {error}")),
                    }
                }
                let mut lines = Vec::new();
                if !analysis.mode.windowed() {
                    for series in &series {
                        if let Some(old) = plot.lines.iter().find(|line| {
                            line.source
                                .strip_prefix("@spectrum/")
                                .and_then(|source| data.source(source).ok())
                                .as_deref()
                                == Some(series.id())
                        }) {
                            let mut line = old.clone();
                            line.source = format!("@spectrum/{}", series.id());
                            line.data = series.data().clone();
                            line.revision += 1;
                            lines.push(line);
                        }
                    }
                }
                plot.lines = lines;
                analysis.series = TimeSeriesSet::new(series).expect("Unique analysis sources");
                if let (Some(old), Some(new)) = (&analysis.heatmap, &mut heatmap) {
                    new.x_label.clone_from(&old.x_label);
                    new.y_label.clone_from(&old.y_label);
                }
                analysis.heatmap = heatmap;
                analysis.summary = if analysis.series.series().is_empty() {
                    "No compatible inputs in this log"
                } else {
                    "Recomputed for the current log"
                }
                .into();
                plot.analysis = Some(analysis);
            } else {
                plot.lines = plot
                    .lines
                    .into_iter()
                    .filter_map(|mut line| match data.read(&line.source) {
                        Ok((source, input)) => {
                            if data.old_log.setting_label(&line.source).as_deref()
                                == Some(&line.name)
                                && let Some(label) = data.log.setting_label(&source)
                            {
                                line.name = label;
                            }
                            line.source = source;
                            line.data = input;
                            line.revision += 1;
                            Some(line)
                        }
                        Err(error) => {
                            warnings.push(format!("{}: {error}", line.name));
                            None
                        }
                    })
                    .collect();
            }
            plots.push(plot);
        }
        // Prepared profile caches must also refer exclusively to the new log.
        for instance in profiles.values_mut() {
            instance.fields = instance
                .fields
                .iter()
                .filter_map(|field| fields.iter().find(|new| new.id == field.id).cloned())
                .collect();
            for view in instance.views.values_mut() {
                view.fields
                    .retain_mut(|(source, input)| match data.current(source) {
                        Ok(new) => {
                            *input = new;
                            true
                        }
                        Err(_) => false,
                    });
            }
            instance.geometry.retain_mut(|item| {
                match geometry.iter().find(|new| new.id == item.id) {
                    Some(new) => {
                        *item = new.clone();
                        true
                    }
                    None => false,
                }
            });
        }
        Refreshed {
            ready_workspaces: self.ready_workspaces,
            fields,
            geometry,
            profiles,
            plots,
            cache: data.evaluation.into_series(),
            mapping: data.mapping,
            warnings,
        }
    }
}

impl Workbench {
    pub(crate) fn restore_log_session(self, session: Refreshed) {
        self.profiles.cancel();
        self.profiles.error.set(String::new());
        self.profiles.failures.set(BTreeMap::new());
        self.profiles.instances.set(session.profiles);
        self.profiles.log_warnings.set(session.warnings);
        self.cache.set(session.cache);
        self.derived.set(session.fields);
        self.geometry.items.set(session.geometry);
        let valid = |source: &str| {
            self.cache
                .with_untracked(|cache| cache.contains_key(source))
                || self.log.with_untracked(|log| {
                    log.as_ref()
                        .is_some_and(|log| log.catalogue.iter().any(|field| field.id == source))
                })
        };
        let remap = |source: &str| {
            session
                .mapping
                .get(source)
                .cloned()
                .unwrap_or_else(|| Some(source.to_owned()))
                .unwrap_or_default()
        };
        let metadata = self
            .profiles
            .metadata
            .get_untracked()
            .into_iter()
            .filter_map(|(source, metadata)| {
                let source = remap(&source);
                valid(&source).then_some((source, metadata))
            })
            .collect::<MetadataMap>();
        self.profiles.overrides.update(|overrides| {
            *overrides = std::mem::take(overrides)
                .into_iter()
                .filter_map(|(source, metadata)| {
                    let source = remap(&source);
                    valid(&source).then_some((source, metadata))
                })
                .collect();
        });
        self.rebuild_profile_metadata();
        self.profiles.metadata.update(|current| {
            for (source, value) in metadata {
                current.entry(source).or_insert(value);
            }
            for (source, value) in self.profiles.overrides.get_untracked() {
                if value.is_none() {
                    current.remove(&source);
                }
            }
            current.retain(|source, _| valid(source));
        });
        self.selected_field.update(|source| {
            *source = source
                .as_ref()
                .map(|source| remap(source))
                .filter(|source| valid(source));
        });
        self.selection_anchor.update(|source| {
            *source = source
                .as_ref()
                .map(|source| remap(source))
                .filter(|source| valid(source));
        });
        self.selected_fields.update(|sources| {
            *sources = sources
                .iter()
                .map(|source| remap(source))
                .filter(|source| valid(source))
                .collect();
        });
        for plot in session.plots {
            let Some(pane) = self.find(plot.id) else {
                continue;
            };
            let current = pane
                .lines
                .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<BTreeSet<_>>());
            pane.lines.set(
                plot.lines
                    .into_iter()
                    .filter(|line| current.contains(&line.id))
                    .collect(),
            );
            pane.analysis.set(plot.analysis);
            pane.loading.set(None);
            pane.snapshot.set_value(None);
            pane.plot.hover.set(None);
            let ids = pane
                .lines
                .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<BTreeSet<_>>());
            if pane
                .selected
                .get_untracked()
                .is_some_and(|id| !ids.contains(&id))
            {
                pane.selected.set(None);
            }
            pane.drawing
                .update(|drawing| drawing.retain(|id, _| ids.contains(id)));
            pane.plot.styles.update(|styles| {
                styles.retain(|id, _| id.parse::<u64>().is_ok_and(|id| ids.contains(&id)))
            });
            pane.plot.hidden.update(|hidden| {
                hidden.retain(|id| id.parse::<u64>().is_ok_and(|id| ids.contains(&id)))
            });
            pane.geometry
                .update(|ids| ids.retain(|id| self.geometry_item(*id).is_some()));
            pane.hidden_geometry
                .update(|ids| ids.retain(|id| self.geometry_item(*id).is_some()));
            pane.legend_order.update(|order| {
                order.retain(|id| ids.contains(id) || self.geometry_item(*id).is_some())
            });
            pane.plot.auto_fit.set(true);
            pane.fit_axes(plot::interaction::Axes::BOTH);
        }
        for view in self.all_views() {
            let ui = view.analysis;
            let results = view.panes.with_untracked(|panes| {
                panes
                    .iter()
                    .filter_map(|pane| pane.analysis.get_untracked())
                    .collect::<Vec<_>>()
            });
            ui.results.set(results);
            ui.message.set(String::new());
            ui.sources.update(|sources| {
                *sources = std::mem::take(sources)
                    .into_iter()
                    .map(|source| remap(&source))
                    .filter(|source| valid(source))
                    .collect();
            });
            ui.disabled.update(|sources| {
                *sources = sources
                    .iter()
                    .map(|source| remap(source))
                    .filter(|source| valid(source))
                    .collect();
            });
            let condition = remap(&ui.condition.get_untracked());
            ui.condition.set(if valid(&condition) {
                condition
            } else {
                String::new()
            });
            ui.start.set(String::new());
            ui.end.set(String::new());
            ui.picking_range.set(false);
            ui.run_selection.set(false);
        }
        self.workspaces.items.update(|items| {
            for workspace in items {
                if session.ready_workspaces.contains(&workspace.id) {
                    workspace.generation = self.loaded_generation.get_untracked();
                }
                workspace.loading.set(None);
            }
        });
        if self
            .geometry
            .selected
            .get_untracked()
            .is_some_and(|id| self.geometry_item(id).is_none())
        {
            self.geometry.selected.set(None);
            self.geometry.editing.set(false);
        }
    }
}

#[cfg(test)]
#[path = "log_session_tests.rs"]
mod tests;
