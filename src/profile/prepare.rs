use super::{MetadataMap, Profile, Shape, Source, resolve};
use crate::{
    derived::{self, DerivedField},
    geometry::{Geometry, GeometryKind},
    log::LogStore,
    state_bands::StateBands,
};
use plot::time_series::SeriesData;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Prepared {
    pub name: String,
    #[cfg(test)]
    pub warnings: Vec<String>,
    pub diagnostics: Vec<crate::profile_instance::Diagnostic>,
    pub groups: BTreeMap<String, String>,
    pub symbols: BTreeMap<String, String>,
    pub ids: super::identity::ResourceIds,
    pub fields: Vec<DerivedField>,
    pub metadata: MetadataMap,
    pub geometry: Vec<Geometry>,
    pub views: Vec<PreparedView>,
}
#[derive(Clone)]
pub struct PreparedView {
    pub name: String,
    pub fields: Vec<(String, SeriesData)>,
    pub geometry: Vec<u64>,
    pub tab: bool,
    pub stacked: bool,
    pub bindings: BTreeMap<String, String>,
}

#[cfg(test)]
pub fn prepare(
    profile: Profile,
    log: &LogStore,
    base: &Path,
    first_id: u64,
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    let ids = super::identity::ResourceIds {
        fields: profile
            .filters
            .iter()
            .enumerate()
            .map(|(index, field)| (field.id.clone(), first_id + index as u64))
            .collect(),
        geometry: profile
            .geometry
            .iter()
            .enumerate()
            .map(|(index, geo)| {
                (
                    geo.id.clone(),
                    first_id + profile.filters.len() as u64 + index as u64,
                )
            })
            .collect(),
    };
    prepare_with_ids(profile, log, base, ids, cancel)
}

pub fn prepare_with_ids(
    mut profile: Profile,
    log: &LogStore,
    base: &Path,
    ids: super::identity::ResourceIds,
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    let mut diagnostics = Vec::new();
    let mut symbols = BTreeMap::new();
    let mut unavailable = BTreeMap::<String, String>::new();
    let mut metadata = MetadataMap::new();
    let mut declared_ids = BTreeSet::new();
    requirements(&profile, log)?;
    for field in &profile.requires {
        if let Err(error) =
            resolve_input(log, &Source::Field(field.clone()), &symbols, &unavailable)
        {
            report(
                &mut diagnostics,
                "Required field",
                Some(field.clone()),
                error,
            );
        }
    }
    for id in profile
        .signals
        .iter()
        .map(|s| &s.id)
        .chain(profile.filters.iter().map(|s| &s.id))
        .chain(profile.geometry.iter().map(|s| &s.id))
    {
        if id.is_empty() || id.starts_with('$') || !declared_ids.insert(id.clone()) {
            return Err(format!("Invalid or duplicate profile ID {id}"));
        }
    }
    for signal in &profile.signals {
        validate_metadata(&signal.metadata)?;
        let source = match resolve_input(log, &signal.source, &symbols, &unavailable) {
            Ok(source) => source,
            Err(error) => {
                unavailable.insert(signal.id.clone(), error.clone());
                report(
                    &mut diagnostics,
                    &signal.metadata.name,
                    Some(format!("{:?}", signal.source)),
                    error,
                );
                continue;
            }
        };
        if metadata
            .insert(source.clone(), signal.metadata.clone())
            .is_some()
        {
            return Err(format!("Multiple aliases for {source}"));
        }
        symbols.insert(signal.id.clone(), source);
    }
    let mut groups = BTreeMap::new();
    for group in &profile.field_groups {
        if group.name.trim().is_empty() || group.source.is_empty() || group.components.len() > 256 {
            return Err("Field groups need source, name, and at most 256 components".into());
        }
        let mut found = false;
        for (suffix, axis) in &group.components {
            if suffix.is_empty()
                || !(suffix.starts_with('[') || suffix.starts_with('.') || suffix.starts_with('/'))
                || axis.trim().is_empty()
            {
                return Err(
                    "Group components need exact [index], .member, or /field suffixes and labels"
                        .into(),
                );
            }
            let source = format!("{}{suffix}", group.source);
            if !log.catalogue.iter().any(|field| field.id == source) {
                report(
                    &mut diagnostics,
                    &format!("{} {axis}", group.name),
                    Some(source.clone()),
                    format!("Missing field {source}"),
                );
                continue;
            }
            found = true;
            let metadata_value = super::Metadata {
                name: format!("{} {axis}", group.name),
                unit: group.unit.clone(),
                axis: axis.clone(),
            };
            validate_metadata(&metadata_value)?;
            metadata.entry(source).or_insert(metadata_value);
        }
        if found {
            groups.insert(group.source.clone(), group.name.clone());
        }
    }
    for filter in &profile.filters {
        symbols.insert(filter.id.clone(), derived::key(ids.fields[&filter.id]));
    }
    let mut pending = Vec::new();
    for filter in &profile.filters {
        let id = ids.fields[&filter.id];
        validate_metadata(&filter.metadata)?;
        let candidate = (|| {
            Ok::<_, String>(DerivedField {
                id,
                group: id,
                name: filter.metadata.name.clone(),
                data: None,
                sources: filter
                    .inputs
                    .iter()
                    .map(|s| resolve_input(log, s, &symbols, &unavailable))
                    .collect::<Result<_, _>>()?,
                filter: resolve::operation(log, &filter.operation, base)?,
            })
        })();
        match candidate {
            Ok(field) => pending.push(field),
            Err(error) => {
                unavailable.insert(filter.id.clone(), error.clone());
                report(&mut diagnostics, &filter.metadata.name, None, error);
            }
        }
    }
    let mut fields: Vec<DerivedField> = Vec::new();
    while !pending.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Profile cancelled".into());
        }
        let pending_ids: BTreeSet<_> = pending.iter().map(|f| f.id).collect();
        let Some(index) = pending.iter().position(|f| {
            f.sources
                .iter()
                .filter_map(|s| derived::id(s))
                .all(|id| !pending_ids.contains(&id))
        }) else {
            for field in &pending {
                if let Some((local, _)) = ids.fields.iter().find(|(_, id)| **id == field.id) {
                    unavailable.insert(
                        local.clone(),
                        "Unresolved or cyclic filter dependencies".into(),
                    );
                }
                report(
                    &mut diagnostics,
                    &field.name,
                    None,
                    "Unresolved or cyclic filter dependencies".into(),
                );
            }
            break;
        };
        let candidate = pending.remove(index);
        let name = candidate.name.clone();
        let local = ids
            .fields
            .iter()
            .find(|(_, id)| **id == candidate.id)
            .unwrap()
            .0
            .clone();
        let root_cause = candidate
            .sources
            .iter()
            .filter_map(|s| derived::id(s))
            .find_map(|id| {
                ids.fields
                    .iter()
                    .find(|(_, value)| **value == id)
                    .and_then(|(local, _)| unavailable.get(local))
                    .cloned()
            });
        if let Some(error) = root_cause {
            unavailable.insert(local, error.clone());
            report(&mut diagnostics, &name, None, error);
            continue;
        }
        let mut branch = fields.clone();
        branch.push(candidate);
        match derived::compute_cancelled(branch, log, BTreeMap::new(), cancel.clone()) {
            Ok(computed) => fields = computed,
            Err(error) => {
                unavailable.insert(local, error.clone());
                report(&mut diagnostics, &name, None, error);
            }
        }
    }
    fields.sort_by_key(|field| field.id);
    for filter in &profile.filters {
        if let Some(key) = symbols.get(&filter.id)
            && fields.iter().any(|field| field.key() == *key)
        {
            metadata.insert(key.clone(), filter.metadata.clone());
        }
    }
    let mut data: BTreeMap<_, _> = fields
        .iter()
        .filter_map(|f| f.data.clone().map(|data| (f.key(), data)))
        .collect();
    let mut geometry = Vec::new();
    let mut geometry_ids = BTreeMap::new();
    for definition in &profile.geometry {
        if cancel.load(Ordering::Relaxed) {
            return Err("Profile cancelled".into());
        }
        if definition.name.trim().is_empty() {
            return Err("Geometry needs a name".into());
        }
        let result = (|| -> Result<Geometry, String> {
            let id = ids.geometry[&definition.id];
            let kind = match definition.shape {
                Shape::Line { .. } => GeometryKind::Line,
                Shape::Rectangle { .. } => GeometryKind::Rectangle,
                _ => GeometryKind::Band,
            };
            let mut item = Geometry::new(id, kind, kurbo::Point::ZERO);
            item.name = definition.name.clone();
            item.color = definition.color;
            item.opacity = definition.opacity;
            match &definition.shape {
                Shape::Line { coordinates } | Shape::Rectangle { coordinates } => {
                    for (out, value) in item.coordinates.iter_mut().zip(coordinates) {
                        *out = resolve::scalar(log, value)?;
                    }
                }
                Shape::Band { start, end } => {
                    item.coordinates = [
                        resolve::scalar(log, start)?,
                        0.0,
                        resolve::scalar(log, end)?,
                        1.0,
                    ]
                }
                Shape::Flags { source, rules } => {
                    let source = resolve_input(log, source, &symbols, &unavailable)?;
                    let input = get_data(&source, log, &mut data)?;
                    if rules.is_empty() || rules.len() > 64 {
                        return Err("Flags need 1 to 64 rules".into());
                    }
                    let mut masks = BTreeSet::new();
                    for rule in rules {
                        if rule.mask == 0
                            || rule.mask > 9_007_199_254_740_991
                            || rule.name.trim().is_empty()
                            || !masks.insert(rule.mask)
                        {
                            return Err("Flags need unique, nonzero exact masks and names".into());
                        }
                    }
                    let end = if derived::id(&source).is_some() && !log.blackbox_headers.is_empty()
                    {
                        input.samples().last().map_or(log.duration, |point| point.x)
                    } else {
                        log.duration_for(&source)
                    };
                    let mut states = StateBands::build(source, input, end)?;
                    states.apply_flags(rules.clone());
                    item.coordinates = [0., 0., end, 1.];
                    item.states = Some(Arc::new(states));
                }
                Shape::States { source, rules } => {
                    let source = resolve_input(log, source, &symbols, &unavailable)?;
                    let input = get_data(&source, log, &mut data)?;
                    let end = if derived::id(&source).is_some() && !log.blackbox_headers.is_empty()
                    {
                        input.samples().last().map_or(log.duration, |point| point.x)
                    } else {
                        log.duration_for(&source)
                    };
                    let mut states = StateBands::build(source, input, end)?;
                    let mut values = BTreeSet::new();
                    if rules.len() > 256 {
                        return Err("At most 256 state rules per geometry".into());
                    }
                    for rule in rules {
                        if rule.name.trim().is_empty()
                            || !values.insert(rule.value)
                            || !(-9_007_199_254_740_991..=9_007_199_254_740_991)
                                .contains(&rule.value)
                        {
                            return Err(
                                "State rules need unique exact integer values and names".into()
                            );
                        }
                    }
                    states.apply_rules(rules.clone());
                    item.coordinates = [0.0, 0.0, end, 1.0];
                    item.states = Some(Arc::new(states));
                }
            }
            if !item.valid() || !item.nonempty() {
                return Err(format!("Invalid geometry {}", item.name));
            }
            Ok(item)
        })();
        match result {
            Ok(item) => {
                geometry_ids.insert(definition.id.clone(), item.id);
                geometry.push(item);
            }
            Err(error) => {
                unavailable.insert(definition.id.clone(), error.clone());
                report(&mut diagnostics, &definition.name, None, error);
            }
        }
    }
    let mut views = Vec::new();
    let mut view_names = BTreeSet::new();
    for plot in &profile.plot_presets {
        profile.views.push(crate::preset_apply::prepared_recipe(
            plot,
            plot.name.clone(),
        ));
    }
    for view in &profile.view_presets {
        for (index, plot) in view.plots.iter().enumerate() {
            profile.views.push(crate::preset_apply::prepared_recipe(
                plot,
                format!("@workspace/{}/{index}", view.name),
            ));
        }
    }
    // Validate view references even when creating plots is unchecked.
    for workspace in &profile.workspace_presets {
        for (view_index, view) in workspace.views.iter().enumerate() {
            for (plot_index, plot) in view.plots.iter().enumerate() {
                profile.views.push(crate::preset_apply::prepared_recipe(
                    plot,
                    format!(
                        "@workspace-set/{}/{view_index}/{plot_index}",
                        workspace.name
                    ),
                ));
            }
        }
    }
    for view in profile.views {
        if !view_names.insert(view.name.clone()) {
            return Err(format!("Duplicate view name {}", view.name));
        }
        if view.name.trim().is_empty() {
            return Err("Plot view needs a name".into());
        }
        let mut fields = Vec::new();
        let mut bindings = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for source in &view.fields {
            let result = resolve_input(log, source, &symbols, &unavailable)
                .and_then(|key| get_data(&key, log, &mut data).map(|data| (key, data)));
            let (key, input) = match result {
                Ok(value) => value,
                Err(error) => {
                    report(
                        &mut diagnostics,
                        &view.name,
                        Some(format!("{source:?}")),
                        error,
                    );
                    continue;
                }
            };
            if !seen.insert(key.clone()) {
                return Err(format!("Duplicate field in view {}", view.name));
            }
            bindings.insert(
                match source {
                    Source::Field(id) => id.clone(),
                    Source::Setting { setting } => format!(
                        "setting:{}:{:?}:{:?}",
                        setting.name, setting.occurrence, setting.element
                    ),
                },
                key.clone(),
            );
            fields.push((key, input));
        }
        let attached = view
            .geometry
            .iter()
            .filter_map(|id| {
                let found = geometry_ids.get(id.trim_start_matches('$')).copied();
                if found.is_none() {
                    report(
                        &mut diagnostics,
                        &view.name,
                        Some(id.clone()),
                        unavailable
                            .get(id.trim_start_matches('$'))
                            .cloned()
                            .unwrap_or_else(|| format!("Unavailable geometry {id}")),
                    );
                }
                found
            })
            .collect::<Vec<_>>();
        if fields.is_empty() && attached.is_empty() {
            report(
                &mut diagnostics,
                &view.name,
                None,
                "No usable fields or geometry".into(),
            );
            continue;
        }
        views.push(PreparedView {
            name: view.name,
            fields,
            geometry: attached,
            tab: view.tab,
            stacked: view.stacked,
            bindings,
        });
    }
    Ok(Prepared {
        name: profile.name,
        #[cfg(test)]
        warnings: diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.resource, d.message))
            .collect(),
        diagnostics,
        groups,
        symbols,
        ids,
        fields,
        metadata,
        geometry,
        views,
    })
}

fn validate_metadata(metadata: &super::Metadata) -> Result<(), String> {
    if metadata.name.trim().is_empty()
        || metadata.name.len() > 256
        || metadata.unit.len() > 64
        || metadata.axis.len() > 64
    {
        Err("Signal metadata needs a name (up to 256 bytes), unit and axis (up to 64 bytes)".into())
    } else {
        Ok(())
    }
}
fn get_data(
    key: &str,
    log: &LogStore,
    data: &mut BTreeMap<String, SeriesData>,
) -> Result<SeriesData, String> {
    if !data.contains_key(key) {
        data.insert(key.into(), log.prepare(key)?);
    }
    data.get(key)
        .cloned()
        .ok_or("Profile field unavailable".into())
}

pub fn requirements(profile: &Profile, log: &LogStore) -> Result<(), String> {
    for condition in &profile.requires_settings {
        let (_, setting) = resolve::record(log, &condition.setting)?;
        let matches = match (&condition.equals, &condition.starts_with) {
            (Some(expected), None) => setting.value.trim() == expected,
            (None, Some(prefix)) if !prefix.is_empty() => setting.value.trim().starts_with(prefix),
            _ => {
                return Err(
                    "Setting condition needs exactly one equals or nonempty starts_with".into(),
                );
            }
        };
        if !matches {
            return Err(format!(
                "Setting {} = {:?}; profile requires {} {:?}",
                condition.setting.name,
                setting.value.trim(),
                if condition.equals.is_some() {
                    "equals"
                } else {
                    "starts_with"
                },
                condition
                    .equals
                    .as_ref()
                    .or(condition.starts_with.as_ref())
                    .unwrap()
            ));
        }
    }
    Ok(())
}

fn report(
    all: &mut Vec<crate::profile_instance::Diagnostic>,
    resource: &str,
    source: Option<String>,
    message: String,
) {
    if let Some(cause) = all.iter_mut().find(|d| d.message == message) {
        if cause.resource != resource && !cause.dependents.iter().any(|s| s == resource) {
            cause.dependents.push(resource.into());
        }
    } else {
        all.push(crate::profile_instance::Diagnostic {
            resource: resource.into(),
            source,
            message,
            dependents: Vec::new(),
        });
    }
}

fn resolve_input(
    log: &LogStore,
    source: &Source,
    symbols: &BTreeMap<String, String>,
    unavailable: &BTreeMap<String, String>,
) -> Result<String, String> {
    if let Source::Field(id) = source
        && let Some(error) = id.strip_prefix('$').and_then(|id| unavailable.get(id))
    {
        return Err(error.clone());
    }
    resolve::source(log, source, symbols)
}
