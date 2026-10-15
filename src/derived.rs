use crate::{filter, log::LogStore};
use plot::time_series::SeriesData;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterSpec {
    LowPass(f64),
    HighPass(f64),
    Affine { gain: f64, offset: f64 },
    Magnitude,
    Quaternion(RpyAxis, AngleUnit),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AngleUnit {
    Radians,
    Degrees,
}
impl AngleUnit {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Radians => "rad",
            Self::Degrees => "deg",
        }
    }
    pub fn convert_radians(self, value: f64) -> f64 {
        match self {
            Self::Radians => value,
            Self::Degrees => value.to_degrees(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RpyAxis {
    Roll,
    Pitch,
    Yaw,
}
impl RpyAxis {
    pub const ALL: [Self; 3] = [Self::Roll, Self::Pitch, Self::Yaw];
    pub fn name(self) -> &'static str {
        match self {
            Self::Roll => "Roll",
            Self::Pitch => "Pitch",
            Self::Yaw => "Yaw",
        }
    }
}
impl FilterSpec {
    pub fn label(self) -> String {
        match self {
            Self::LowPass(hz) => format!("Low-pass · {hz} Hz"),
            Self::HighPass(hz) => format!("High-pass · {hz} Hz"),
            Self::Affine { gain, offset } => format!("Gain {gain} · offset {offset}"),
            Self::Magnitude => "Magnitude".into(),
            Self::Quaternion(axis, unit) => {
                format!("Quaternion → {} ({})", axis.name(), unit.symbol())
            }
        }
    }
    pub fn apply(self, data: &SeriesData) -> Result<SeriesData, String> {
        self.apply_inputs(&[data])
    }
    pub fn apply_inputs(self, inputs: &[&SeriesData]) -> Result<SeriesData, String> {
        self.validate_inputs(inputs.len())?;
        let data = inputs[0];
        match self {
            Self::Magnitude | Self::Quaternion(..) => crate::filter_multi::apply(self, inputs),
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
        match self {
            Self::Magnitude | Self::Quaternion(..) => Ok(()),
            _ => self.apply(&SeriesData::new(Vec::new())?).map(|_| ()),
        }
    }
    pub fn validate_inputs(self, count: usize) -> Result<(), String> {
        match self {
            Self::Quaternion(..) if count != 4 => {
                Err("Quaternion needs exactly four inputs: w, x, y, z".into())
            }
            Self::Magnitude if count == 0 => Err("Magnitude needs at least one input".into()),
            Self::LowPass(_) | Self::HighPass(_) | Self::Affine { .. } if count != 1 => {
                Err("This filter needs one input per output".into())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterInput {
    pub source: String,
    pub name: String,
}

#[derive(Clone)]
pub struct DerivedField {
    pub id: u64,
    pub name: String,
    pub sources: Vec<String>,
    pub group: u64,
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
    let mut pending = vec![source];
    let mut seen = BTreeSet::new();
    while let Some(source) = pending.pop() {
        let Some(id) = id(source) else { continue };
        if id == target {
            return true;
        }
        if seen.insert(id)
            && let Some(field) = fields.iter().find(|field| field.id == id)
        {
            pending.extend(field.sources.iter().map(String::as_str));
        }
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
        field.filter.validate_inputs(field.sources.len())?;
        for source in &field.sources {
            if depends_on(fields, source, field.id) {
                return Err("Filter source would create a cycle".into());
            }
            if let Some(parent) = id(source) {
                if !fields.iter().any(|node| node.id == parent) {
                    return Err("Source field no longer exists".into());
                }
            } else if !log.catalogue.iter().any(|raw| &raw.id == source) {
                return Err("Choose an existing source field".into());
            }
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
            if field
                .sources
                .iter()
                .filter_map(|source| id(source))
                .any(|id| pending.contains(&id))
            {
                continue;
            }
            for source in &field.sources {
                if id(source).is_none() && !cache.contains_key(source) {
                    cache.insert(source.clone(), log.prepare(source)?);
                }
            }
            let inputs = field
                .sources
                .iter()
                .map(|source| {
                    cache
                        .get(source)
                        .ok_or("Source field unavailable".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let data = field.filter.apply_inputs(&inputs)?;
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
