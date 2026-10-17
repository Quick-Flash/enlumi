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
}

pub fn prepare(
    profile: Profile,
    log: &LogStore,
    base: &Path,
    first_id: u64,
    cancel: Arc<AtomicBool>,
) -> Result<Prepared, String> {
    let mut symbols = BTreeMap::new();
    let mut metadata = MetadataMap::new();
    let mut ids = BTreeSet::new();
    requirements(&profile, log)?;
    for id in profile
        .signals
        .iter()
        .map(|s| &s.id)
        .chain(profile.filters.iter().map(|s| &s.id))
        .chain(profile.geometry.iter().map(|s| &s.id))
    {
        if id.is_empty() || id.starts_with('$') || !ids.insert(id.clone()) {
            return Err(format!("Invalid or duplicate profile ID {id}"));
        }
    }
    for signal in &profile.signals {
        let source = resolve::source(log, &signal.source, &symbols)?;
        validate_metadata(&signal.metadata)?;
        if metadata
            .insert(source.clone(), signal.metadata.clone())
            .is_some()
        {
            return Err(format!("Multiple aliases for {source}"));
        }
        symbols.insert(signal.id.clone(), source);
    }
    for (index, filter) in profile.filters.iter().enumerate() {
        symbols.insert(filter.id.clone(), derived::key(first_id + index as u64));
    }
    let mut fields = Vec::new();
    for (index, filter) in profile.filters.iter().enumerate() {
        let id = first_id + index as u64;
        validate_metadata(&filter.metadata)?;
        metadata.insert(derived::key(id), filter.metadata.clone());
        fields.push(DerivedField {
            id,
            group: id,
            name: filter.metadata.name.clone(),
            data: None,
            sources: filter
                .inputs
                .iter()
                .map(|s| resolve::source(log, s, &symbols))
                .collect::<Result<_, _>>()?,
            filter: resolve::operation(log, &filter.operation, base)?,
        });
    }
    let fields = derived::compute_cancelled(fields, log, BTreeMap::new(), cancel.clone())?;
    let mut data: BTreeMap<_, _> = fields
        .iter()
        .filter_map(|f| f.data.clone().map(|data| (f.key(), data)))
        .collect();
    let mut geometry = Vec::new();
    let mut geometry_ids = BTreeMap::new();
    for (index, definition) in profile.geometry.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Profile cancelled".into());
        }
        if definition.name.trim().is_empty() {
            return Err("Geometry needs a name".into());
        }
        let id = first_id + fields.len() as u64 + index as u64;
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
            Shape::States { source, rules } => {
                let source = resolve::source(log, source, &symbols)?;
                let input = get_data(&source, log, &mut data)?;
                let end = if derived::id(&source).is_some() && !log.blackbox_headers.is_empty() {
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
                        || !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&rule.value)
                    {
                        return Err("State rules need unique exact integer values and names".into());
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
        geometry_ids.insert(definition.id.clone(), id);
        geometry.push(item);
    }
    let mut views = Vec::new();
    // Validate view references even when creating plots is unchecked.
    for view in profile.views {
        if view.name.trim().is_empty() {
            return Err("Plot view needs a name".into());
        }
        let mut fields = Vec::new();
        let mut seen = BTreeSet::new();
        for source in &view.fields {
            let key = resolve::source(log, source, &symbols)?;
            if !seen.insert(key.clone()) {
                return Err(format!("Duplicate field in view {}", view.name));
            }
            fields.push((key.clone(), get_data(&key, log, &mut data)?));
        }
        let attached = view
            .geometry
            .iter()
            .map(|id| {
                geometry_ids
                    .get(id.trim_start_matches('$'))
                    .copied()
                    .ok_or_else(|| format!("Unknown geometry {id}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        views.push(PreparedView {
            name: view.name,
            fields,
            geometry: attached,
            tab: view.tab,
            stacked: view.stacked,
        });
    }
    Ok(Prepared {
        name: profile.name,
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
    for field in &profile.requires {
        resolve::source(log, &Source::Field(field.clone()), &BTreeMap::new())?;
    }
    let sources = profile
        .signals
        .iter()
        .map(|signal| &signal.source)
        .chain(profile.filters.iter().flat_map(|filter| &filter.inputs))
        .chain(profile.views.iter().flat_map(|view| &view.fields))
        .chain(
            profile
                .geometry
                .iter()
                .filter_map(|geometry| match &geometry.shape {
                    Shape::States { source, .. } => Some(source),
                    _ => None,
                }),
        );
    for source in sources {
        if matches!(source, Source::Field(id) if id.starts_with('$')) {
            continue;
        }
        resolve::source(log, source, &BTreeMap::new())?;
    }
    Ok(())
}
