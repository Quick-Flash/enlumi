use crate::{filter, log::LogStore};
use plot::time_series::SeriesData;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

mod parallel;
pub(crate) use parallel::{compute_independent, pool};

#[derive(Clone, Debug, PartialEq)]
pub enum FilterSpec {
    LowPass(f64),
    HighPass(f64),
    ZeroPhaseLowPass(f64),
    ZeroPhaseHighPass(f64),
    MovingAverage(usize),
    Derivative,
    Integral(f64),
    Affine { gain: f64, offset: f64 },
    Magnitude,
    Rust(Arc<crate::script::Script>, usize),
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
    pub fn label(&self) -> String {
        match self {
            Self::LowPass(hz) => format!("Low-pass · {hz} Hz"),
            Self::HighPass(hz) => format!("High-pass · {hz} Hz"),
            Self::ZeroPhaseLowPass(hz) => format!("Low-pass / zero phase / {hz} Hz"),
            Self::ZeroPhaseHighPass(hz) => format!("High-pass / zero phase / {hz} Hz"),
            Self::MovingAverage(window) => format!("Moving average / {window} samples"),
            Self::Derivative => "Derivative".into(),
            Self::Integral(initial) => format!("Integral / initial {initial}"),
            Self::Affine { gain, offset } => format!("Gain {gain} · offset {offset}"),
            Self::Magnitude => "Magnitude".into(),
            Self::Rust(..) => "Rust / WASM".into(),
            Self::Quaternion(axis, unit) => {
                format!("Quaternion → {} ({})", axis.name(), unit.symbol())
            }
        }
    }
    pub fn apply(&self, data: &SeriesData) -> Result<SeriesData, String> {
        self.apply_inputs(&[data])
    }
    pub fn apply_inputs(&self, inputs: &[&SeriesData]) -> Result<SeriesData, String> {
        self.validate_inputs(inputs.len())?;
        let data = inputs[0];
        match self {
            Self::Magnitude | Self::Quaternion(..) => crate::filter_multi::apply(self, inputs),
            Self::Rust(script, output) => script
                .apply(inputs, Arc::new(AtomicBool::new(false)))?
                .get(*output)
                .cloned()
                .ok_or("Rust output index out of range".into()),
            Self::LowPass(hz) => filter::low_pass(data, *hz),
            Self::HighPass(hz) => filter::high_pass(data, *hz),
            Self::ZeroPhaseLowPass(hz) => crate::filter_zero_phase::apply(data, *hz, false),
            Self::ZeroPhaseHighPass(hz) => crate::filter_zero_phase::apply(data, *hz, true),
            Self::MovingAverage(window) => crate::filter_extra::moving_average(data, *window),
            Self::Derivative => crate::filter_extra::derivative(data),
            Self::Integral(initial) => crate::filter_extra::integral(data, *initial),
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
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Magnitude | Self::Quaternion(..) => Ok(()),
            Self::Rust(script, output) => {
                script.validate(1)?;
                if *output >= script.outputs {
                    return Err("Rust output index out of range".into());
                }
                Ok(())
            }
            _ => self.apply(&SeriesData::new(Vec::new())?).map(|_| ()),
        }
    }
    pub fn validate_inputs(&self, count: usize) -> Result<(), String> {
        match self {
            Self::Rust(script, _) => script.validate(count),
            Self::Quaternion(..) if count != 4 => {
                Err("Quaternion needs exactly four inputs: w, x, y, z".into())
            }
            Self::Magnitude if count == 0 => Err("Magnitude needs at least one input".into()),
            Self::LowPass(_)
            | Self::HighPass(_)
            | Self::ZeroPhaseLowPass(_)
            | Self::ZeroPhaseHighPass(_)
            | Self::MovingAverage(_)
            | Self::Derivative
            | Self::Integral(_)
            | Self::Affine { .. }
                if count != 1 =>
            {
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
        validate_field(field, fields, log)?;
    }
    Ok(())
}

/// Existing fields already passed validation in this immutable preparation run.
pub(crate) fn validate_addition(
    field: &DerivedField,
    fields: &[DerivedField],
    log: &LogStore,
) -> Result<(), String> {
    if field.name.trim().is_empty() {
        return Err("Give output field a name".into());
    }
    let name = field.name.to_lowercase();
    if fields.iter().any(|other| other.name.to_lowercase() == name) {
        return Err("Derived field names must be unique".into());
    }
    validate_field(field, fields, log)
}

fn validate_field(
    field: &DerivedField,
    fields: &[DerivedField],
    log: &LogStore,
) -> Result<(), String> {
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
    Ok(())
}

pub fn compute_cancelled(
    fields: Vec<DerivedField>,
    log: &LogStore,
    cache: BTreeMap<String, SeriesData>,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<DerivedField>, String> {
    compute_cached(
        fields,
        log,
        &mut EvaluationCache {
            series: cache,
            ..Default::default()
        },
        cancel,
    )
}

#[derive(Default)]
pub(crate) struct EvaluationCache {
    series: BTreeMap<String, SeriesData>,
    scripts: Vec<CachedScript>,
}
impl EvaluationCache {
    pub(crate) fn series(&self) -> &BTreeMap<String, SeriesData> {
        &self.series
    }
    pub(crate) fn insert_series(&mut self, source: String, data: SeriesData) {
        self.series.insert(source, data);
    }
    pub(crate) fn into_series(self) -> BTreeMap<String, SeriesData> {
        self.series
    }
}
#[derive(Clone)]
struct CachedScript {
    script: Arc<crate::script::Script>,
    sources: Vec<String>,
    outputs: Vec<SeriesData>,
}
pub(crate) fn compute_cached(
    mut fields: Vec<DerivedField>,
    log: &LogStore,
    evaluation: &mut EvaluationCache,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<DerivedField>, String> {
    validate(&fields, log)?;
    let mut pending = BTreeSet::new();
    for field in &fields {
        if let Some(data) = &field.data {
            evaluation.series.insert(field.key(), data.clone());
        } else {
            pending.insert(field.id);
        }
    }
    while !pending.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Filter run cancelled".into());
        }
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
                if id(source).is_none() && !evaluation.series.contains_key(source) {
                    evaluation
                        .series
                        .insert(source.clone(), log.prepare(source)?);
                }
            }
            evaluate_field(field, evaluation, cancel.clone())?;
            pending.remove(&field.id);
            progress = true;
        }
        if !progress {
            return Err("Filter chain cannot be resolved".into());
        }
    }
    Ok(fields)
}

pub(crate) fn evaluate_field(
    field: &mut DerivedField,
    evaluation: &mut EvaluationCache,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let cache = &mut evaluation.series;
    let scripts = &mut evaluation.scripts;
    let inputs = field
        .sources
        .iter()
        .map(|source| {
            cache
                .get(source)
                .ok_or("Source field unavailable".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let data = if let FilterSpec::Rust(script, output) = &field.filter {
        let index = if let Some(index) = scripts
            .iter()
            .position(|cached| cached.script == *script && cached.sources == field.sources)
        {
            index
        } else {
            scripts.push(CachedScript {
                script: script.clone(),
                sources: field.sources.clone(),
                outputs: script.apply(&inputs, cancel.clone())?,
            });
            scripts.len() - 1
        };
        scripts[index]
            .outputs
            .get(*output)
            .cloned()
            .ok_or("Rust output index out of range")?
    } else {
        field.filter.apply_inputs(&inputs)?
    };
    cache.insert(field.key(), data.clone());
    field.data = Some(data);
    Ok(())
}
