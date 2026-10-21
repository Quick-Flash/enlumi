use crate::{
    derived,
    profile::{
        MetadataMap,
        identity::{Item, ProfileKey},
    },
    profile_instance::AppliedProfile,
    state::{Line, Workbench},
};
use std::collections::{BTreeMap, BTreeSet};
use zgui::prelude::*;
impl Workbench {
    pub(crate) fn commit_instance(
        self,
        mut instance: AppliedProfile,
        fields: Vec<derived::DerivedField>,
        geometry_updates: Vec<(crate::geometry::Geometry, crate::geometry::Geometry)>,
    ) -> Result<String, String> {
        let previous = self
            .profiles
            .instances
            .with_untracked(|all| all.get(&instance.key).cloned());
        if let Some(old) = &previous {
            // Keep valid resources borrowed by other profiles even if this recipe now skips them.
            for item in &old.geometry {
                if !instance
                    .geometry
                    .iter()
                    .any(|geometry| geometry.id == item.id)
                    && self.profiles.instances.with_untracked(|instances| {
                        instances.values().any(|other| {
                            other.key != instance.key
                                && other.ids.geometry.values().any(|id| *id == item.id)
                        })
                    })
                {
                    instance.geometry.push(item.clone());
                }
            }
            let removed_fields = old
                .field_ids()
                .difference(&instance.field_ids())
                .copied()
                .filter(|id| {
                    self.derived
                        .with_untracked(|fields| fields.iter().any(|field| field.id == *id))
                })
                .collect::<BTreeSet<_>>();
            let removed_geometry = old
                .geometry_ids()
                .difference(&instance.geometry_ids())
                .copied()
                .collect::<BTreeSet<_>>();
            if self.profiles.instances.with_untracked(|instances| {
                instances
                    .values()
                    .filter(|other| other.key != instance.key)
                    .any(|other| {
                        other
                            .ids
                            .fields
                            .values()
                            .any(|id| removed_fields.contains(id))
                            || other
                                .ids
                                .geometry
                                .values()
                                .any(|id| removed_geometry.contains(id))
                    })
            }) {
                return Err(
                    "Reload kept previous setup: another profile uses removed resources".into(),
                );
            }
        }
        let old_geometry = previous
            .as_ref()
            .map(AppliedProfile::geometry_ids)
            .unwrap_or_default();
        for other in self
            .profiles
            .instances
            .get_untracked()
            .values()
            .filter(|other| other.key != instance.key)
        {
            for (source, meta) in &mut instance.metadata {
                if let Some(old) = other.metadata.get(source)
                    && old != meta
                {
                    if instance.definition.version >= 2 {
                        // Saved plot recipes carry labels, not exclusive ownership of inputs.
                        meta.clone_from(old);
                        continue;
                    }
                    return Err(format!(
                        "Metadata conflict: {source} supplied by {}",
                        other.name
                    ));
                }
            }
            for (source, label) in &instance.groups {
                if other.groups.get(source).is_some_and(|old| old != label) {
                    return Err(format!(
                        "Group conflict: {source} supplied by {}",
                        other.name
                    ));
                }
            }
        }
        let mut geometry = self.geometry.items.get_untracked();
        for (original, updated) in geometry_updates {
            let Some(current) = geometry.iter_mut().find(|g| g.id == original.id) else {
                return Err(format!(
                    "Geometry {} changed during reload; retry",
                    original.name
                ));
            };
            if *current != original {
                return Err(format!(
                    "Geometry {} changed during reload; retry",
                    original.name
                ));
            }
            *current = updated;
        }
        geometry.retain(|g| !old_geometry.contains(&g.id));
        geometry.extend(instance.geometry.clone());
        if let Some(old) = &previous {
            instance.placements = old.placements.clone();
        }
        // View tokens preserve identity when aliases change raw sources.
        if let Some(old) = &previous {
            for placement in &old.placements {
                for (token, id) in &placement.lines {
                    let replacement = match &placement.item {
                        Item::View(name) => instance
                            .views
                            .get(name)
                            .and_then(|view| {
                                view.bindings.get(token).and_then(|source| {
                                    view.fields.iter().find(|(s, _)| s == source)
                                })
                            })
                            .cloned(),
                        Item::Field(_) => instance.resolve(token).and_then(|source| {
                            let data = if derived::id(&source).is_some() {
                                fields
                                    .iter()
                                    .find(|f| f.key() == source)
                                    .and_then(|f| f.data.clone())
                            } else {
                                self.log.with_untracked(|log| {
                                    log.as_ref().and_then(|log| log.prepare(&source).ok())
                                })
                            }?;
                            Some((source, data))
                        }),
                        Item::Geometry(_) | Item::Workspace(_) | Item::WorkspacePreset(_) => None,
                    };
                    for pane in self.all_panes() {
                        pane.lines.update(|lines| {
                            if let Some((source, data)) = &replacement {
                                if let Some(line) = lines.iter_mut().find(|line| line.id == *id) {
                                    line.source = source.clone();
                                    line.data = data.clone();
                                    line.revision += 1;
                                }
                            } else {
                                lines.retain(|line| line.id != *id);
                            }
                        });
                    }
                }
            }
        }
        let committed_key = instance.key.clone();
        let name = instance.name.clone();
        let removed = previous
            .as_ref()
            .map(|old| {
                old.field_ids()
                    .difference(&instance.field_ids())
                    .copied()
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        self.derived.set(fields);
        self.geometry.items.set(geometry);
        if let Some(old) = &previous {
            self.refresh_profile_geometry(old, &instance);
        }
        self.profiles.instances.update(|all| {
            all.insert(instance.key.clone(), instance);
        });
        self.rebuild_profile_metadata();
        for pane in self.all_panes() {
            pane.geometry.update(|ids| {
                ids.retain(|id| !old_geometry.contains(id) || self.geometry_item(*id).is_some())
            });
            pane.lines.update(|lines| {
                lines.retain(|line| {
                    !derived::id(&line.source).is_some_and(|id| removed.contains(&id))
                });
                for line in lines {
                    line.name = self.field_name(&line.source);
                    if let Some(data) = self.derived.with_untracked(|fields| {
                        fields
                            .iter()
                            .find(|f| f.key() == line.source)
                            .and_then(|f| f.data.clone())
                    }) {
                        line.data = data;
                        line.revision += 1;
                    }
                }
            });
        }
        let mut updated = self.profiles.instances.with_untracked(|all| {
            all.get(&committed_key)
                .cloned()
                .into_iter()
                .collect::<Vec<_>>()
        });
        for instance in &mut updated {
            for placement in &mut instance.placements {
                let Item::View(name) = &placement.item else {
                    continue;
                };
                let Some(view) = instance.views.get(name) else {
                    continue;
                };
                let Some(pane) = self.find(placement.pane) else {
                    continue;
                };
                pane.geometry
                    .update(|ids| ids.extend(view.geometry.iter().copied()));
                for (token, source) in &view.bindings {
                    // Existing tracked lines can move between plots. Never move them back.
                    if placement.lines.get(token).is_some_and(|id| {
                        self.all_panes().iter().any(|pane| {
                            pane.lines
                                .with_untracked(|lines| lines.iter().any(|line| line.id == *id))
                        })
                    }) {
                        continue;
                    }
                    if pane
                        .lines
                        .with_untracked(|lines| lines.iter().any(|line| line.source == *source))
                    {
                        continue;
                    }
                    let Some((_, data)) = view.fields.iter().find(|(s, _)| s == source) else {
                        continue;
                    };
                    let id = self.id();
                    self.assign_line_color(pane, id);
                    pane.lines.update(|lines| {
                        lines.push(Line {
                            id,
                            source: source.clone(),
                            name: self.field_name(source),
                            data: data.clone(),
                            revision: 0,
                        })
                    });
                    placement.lines.insert(token.clone(), id);
                }
            }
        }
        self.profiles.instances.update(|all| {
            for instance in updated {
                all.insert(instance.key.clone(), instance);
            }
        });
        self.finish_filter_editing();
        self.expanded.update(|expanded| {
            expanded.insert("@derived".into());
        });
        Ok(name)
    }

    pub fn rebuild_profile_metadata(self) {
        let mut metadata = MetadataMap::new();
        let mut groups = BTreeMap::new();
        let mut warnings = Vec::new();
        for instance in self.profiles.instances.get_untracked().values() {
            metadata.extend(instance.metadata.clone());
            groups.extend(instance.groups.clone());
            warnings.extend(instance.diagnostics.iter().map(|d| {
                format!(
                    "{} / {}: {}{}",
                    instance.name,
                    d.resource,
                    d.message,
                    if d.dependents.is_empty() {
                        String::new()
                    } else {
                        format!(" (also {})", d.dependents.join(", "))
                    }
                )
            }));
        }
        for (source, value) in self.profiles.overrides.get_untracked() {
            match value {
                Some(value) => {
                    metadata.insert(source, value);
                }
                None => {
                    metadata.remove(&source);
                }
            }
        }
        self.profiles.metadata.set(metadata);
        self.profiles.groups.set(groups);
        self.profiles.warnings.set(warnings);
    }

    pub fn profile_remove_blockers(self, key: &ProfileKey) -> Vec<String> {
        let Some(instance) = self
            .profiles
            .instances
            .with_untracked(|all| all.get(key).cloned())
        else {
            return vec![];
        };
        let owned = instance.field_ids();
        let geometry = instance.geometry_ids();
        let mut blockers = self.derived.with_untracked(|fields| {
            fields
                .iter()
                .filter(|f| {
                    !owned.contains(&f.id)
                        && f.sources
                            .iter()
                            .filter_map(|s| derived::id(s))
                            .any(|id| owned.contains(&id))
                })
                .map(|f| format!("Filter: {}", f.name))
                .collect::<Vec<_>>()
        });
        let created = instance.line_ids();
        for pane in self.all_panes() {
            blockers.extend(pane.lines.with_untracked(|lines| {
                lines
                    .iter()
                    .filter(|line| {
                        derived::id(&line.source).is_some_and(|id| owned.contains(&id))
                            && !created.contains(&line.id)
                    })
                    .map(|line| format!("Plot {}: {} (manually added)", pane.id, line.name))
                    .collect::<Vec<_>>()
            }));
        }
        blockers.extend(self.profiles.instances.with_untracked(|instances| {
            instances
                .values()
                .filter(|other| other.key != *key)
                .filter(|other| {
                    other.ids.fields.values().any(|id| owned.contains(id))
                        || other.ids.geometry.values().any(|id| geometry.contains(id))
                })
                .map(|other| format!("Profile: {} uses shared resources", other.name))
                .collect::<Vec<_>>()
        }));
        blockers.extend(self.geometry.items.with_untracked(|items| {
            items
                .iter()
                .filter(|g| {
                    !geometry.contains(&g.id)
                        && g.states.as_ref().is_some_and(|s| {
                            derived::id(&s.source).is_some_and(|id| owned.contains(&id))
                        })
                })
                .map(|g| format!("Geometry: {}", g.name))
                .collect::<Vec<_>>()
        }));
        blockers
    }

    pub fn remove_profile(self, key: ProfileKey) {
        if self.profiles.busy.get_untracked() || self.derived_busy.get_untracked() {
            self.profiles
                .error
                .set("Wait for profile and filters to finish".into());
            return;
        }
        let blockers = self.profile_remove_blockers(&key);
        if !blockers.is_empty() {
            self.profiles
                .error
                .set(format!("Remove blocked by {}", blockers.join(", ")));
            return;
        }
        let Some(instance) = self
            .profiles
            .instances
            .with_untracked(|all| all.get(&key).cloned())
        else {
            return;
        };
        let fields = instance.field_ids();
        let geometry = instance.geometry_ids();
        let lines = instance.line_ids();
        self.derived
            .update(|all| all.retain(|f| !fields.contains(&f.id)));
        self.geometry
            .items
            .update(|all| all.retain(|g| !geometry.contains(&g.id)));
        for pane in self.all_panes() {
            pane.geometry
                .update(|all| all.retain(|id| !geometry.contains(id)));
            pane.lines.update(|all| {
                all.retain(|line| {
                    !lines.contains(&line.id)
                        && !derived::id(&line.source).is_some_and(|id| fields.contains(&id))
                })
            });
        }
        self.profiles.instances.update(|all| {
            all.remove(&key);
        });
        self.profiles.removal.set(None);
        self.profiles.failures.update(|failures| {
            failures.remove(&key);
        });
        self.rebuild_profile_metadata();
        for pane in self.all_panes() {
            pane.lines.update(|lines| {
                for line in lines {
                    line.name = self.field_name(&line.source);
                }
            });
        }
        self.status
            .set(format!("Removed profile: {}", instance.name));
    }
}
