use crate::{filter, log::LogStore};
use plot::time_series::SeriesData;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterSpec {
    LowPass(f64),
    HighPass(f64),
    Affine { gain: f64, offset: f64 },
}
impl FilterSpec {
    pub fn label(self) -> String {
        match self {
            Self::LowPass(hz) => format!("Low-pass · {hz} Hz"),
            Self::HighPass(hz) => format!("High-pass · {hz} Hz"),
            Self::Affine { gain, offset } => format!("Gain {gain} · offset {offset}"),
        }
    }
    pub fn apply(self, data: &SeriesData) -> Result<SeriesData, String> {
        match self {
            Self::LowPass(hz) => filter::low_pass(data, hz),
            Self::HighPass(hz) => {
                let low = filter::low_pass(data, hz)?;
                SeriesData::new(
                    data.samples()
                        .iter()
                        .zip(low.samples())
                        .map(|(raw, low)| plot::PlotPoint {
                            x: raw.x,
                            y: raw.y - low.y,
                        })
                        .collect::<Vec<_>>(),
                )
            }
            Self::Affine { gain, offset } => {
                if !gain.is_finite() || !offset.is_finite() {
                    return Err("Gain and offset must be finite".into());
                }
                SeriesData::new(
                    data.samples()
                        .iter()
                        .map(|point| plot::PlotPoint {
                            x: point.x,
                            y: point.y * gain + offset,
                        })
                        .collect::<Vec<_>>(),
                )
            }
        }
    }
    pub fn validate(self) -> Result<(), String> {
        self.apply(&SeriesData::new(Vec::new())?).map(|_| ())
    }
}

#[derive(Clone)]
pub struct FilterInput {
    pub source: String,
    pub name: String,
}

#[derive(Clone)]
pub struct DerivedField {
    pub id: u64,
    pub name: String,
    pub source: String,
    pub filter: FilterSpec,
    pub data: Option<SeriesData>,
}
impl DerivedField {
    pub fn key(&self) -> String {
        key(self.id)
    }
}
pub fn key(id: u64) -> String {
    format!("@derived/{id}")
}
pub fn id(key: &str) -> Option<u64> {
    key.strip_prefix("@derived/")?.parse().ok()
}

pub fn depends_on(fields: &[DerivedField], source: &str, target: u64) -> bool {
    let mut source = source;
    let mut seen = BTreeSet::new();
    while let Some(id) = id(source) {
        if id == target || !seen.insert(id) {
            return true;
        }
        let Some(field) = fields.iter().find(|field| field.id == id) else {
            return false;
        };
        source = &field.source;
    }
    false
}

pub fn validate(fields: &[DerivedField], log: &LogStore) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for field in fields {
        if field.name.trim().is_empty() {
            return Err("Give output field a name".into());
        }
        if !names.insert(field.name.to_lowercase()) {
            return Err("Derived field names must be unique".into());
        }
        field.filter.validate()?;
        let mut seen = BTreeSet::from([field.id]);
        let mut source = field.source.as_str();
        while let Some(id) = id(source) {
            if !seen.insert(id) {
                return Err("Filter source would create a cycle".into());
            }
            source = &fields
                .iter()
                .find(|node| node.id == id)
                .ok_or("Source field no longer exists")?
                .source;
        }
        if !log.catalogue.iter().any(|raw| raw.id == source) {
            return Err("Choose an existing source field".into());
        }
    }
    Ok(())
}

pub fn compute(
    mut fields: Vec<DerivedField>,
    log: &LogStore,
    mut cache: BTreeMap<String, SeriesData>,
) -> Result<Vec<DerivedField>, String> {
    validate(&fields, log)?;
    let mut pending = BTreeSet::new();
    for field in &fields {
        if let Some(data) = &field.data {
            cache.insert(field.key(), data.clone());
        } else {
            pending.insert(field.id);
        }
    }
    while !pending.is_empty() {
        let mut progress = false;
        for field in &mut fields {
            if !pending.contains(&field.id) {
                continue;
            }
            if let Some(parent) = id(&field.source) {
                if pending.contains(&parent) {
                    continue;
                }
            } else if !cache.contains_key(&field.source) {
                cache.insert(field.source.clone(), log.prepare(&field.source)?);
            }
            let data = field
                .filter
                .apply(cache.get(&field.source).ok_or("Source field unavailable")?)?;
            cache.insert(field.key(), data.clone());
            field.data = Some(data);
            pending.remove(&field.id);
            progress = true;
        }
        if !progress {
            return Err("Filter chain cannot be resolved".into());
        }
    }
    Ok(fields)
}
