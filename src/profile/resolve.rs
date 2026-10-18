use super::{Operation, Scalar, Setting, Source};
use crate::{
    derived::{AngleUnit, FilterSpec, RpyAxis},
    log::LogStore,
};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};

pub fn record<'a>(
    log: &'a LogStore,
    reference: &Setting,
) -> Result<(usize, &'a crate::log::LogSetting), String> {
    let entries = log
        .settings
        .get(&reference.flight)
        .ok_or_else(|| format!("Missing settings for flight {}", reference.flight))?;
    let matches: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.name == reference.name)
        .collect();
    let selected = match reference.occurrence {
        Some(index) => matches.get(index).copied(),
        None if matches.len() == 1 => matches.first().copied(),
        None if matches.len() > 1 => {
            return Err(format!(
                "Setting {} has {} records; specify occurrence",
                reference.name,
                matches.len()
            ));
        }
        None => None,
    }
    .ok_or_else(|| format!("Missing setting {}", reference.name))?;
    Ok(selected)
}

pub fn setting(log: &LogStore, reference: &Setting) -> Result<String, String> {
    let (index, record) = record(log, reference)?;
    if record.kind.starts_with("Parameter change") && reference.occurrence.is_none() {
        return Err(format!(
            "Setting {} is an untimed change; select explicit occurrence to use snapshot",
            reference.name
        ));
    }
    let suffix = reference
        .element
        .map_or_else(String::new, |index| format!("/{index}"));
    let key = format!("@setting/{}/{index}{suffix}", reference.flight);
    if !log.catalogue.iter().any(|entry| entry.id == key) {
        return Err(format!("Setting {} is not a finite number", reference.name));
    }
    Ok(key)
}

pub fn source(
    log: &LogStore,
    source: &Source,
    symbols: &BTreeMap<String, String>,
) -> Result<String, String> {
    match source {
        Source::Setting { setting: reference } => setting(log, reference),
        Source::Field(field) => {
            if let Some(id) = field.strip_prefix('$') {
                symbols
                    .get(id)
                    .cloned()
                    .ok_or_else(|| format!("Unknown profile field ${id}"))
            } else if log.catalogue.iter().any(|entry| entry.id == *field) {
                Ok(field.clone())
            } else {
                Err(format!("Missing field {field}"))
            }
        }
    }
}

pub fn scalar(log: &LogStore, value: &Scalar) -> Result<f64, String> {
    let value = match value {
        Scalar::Number(value) => *value,
        Scalar::Setting { setting: reference } => {
            let key = setting(log, reference)?;
            log.setting_number(&key)
                .ok_or("Parameter is not a finite numeric element")?
        }
        Scalar::Ratio { ratio } => {
            let denominator = scalar(log, &ratio[1])?;
            if denominator == 0.0 {
                return Err("Profile ratio divides by zero".into());
            }
            scalar(log, &ratio[0])? / denominator
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Profile scalar must be finite".into())
    }
}

pub fn operation(log: &LogStore, operation: &Operation, base: &Path) -> Result<FilterSpec, String> {
    Ok(match operation {
        Operation::Affine { gain, offset } => FilterSpec::Affine {
            gain: scalar(log, gain)?,
            offset: scalar(log, offset)?,
        },
        Operation::LowPass { hz } => FilterSpec::LowPass(scalar(log, hz)?),
        Operation::HighPass { hz } => FilterSpec::HighPass(scalar(log, hz)?),
        Operation::ZeroPhaseLowPass { hz } => FilterSpec::ZeroPhaseLowPass(scalar(log, hz)?),
        Operation::ZeroPhaseHighPass { hz } => FilterSpec::ZeroPhaseHighPass(scalar(log, hz)?),
        Operation::MovingAverage { samples } => FilterSpec::MovingAverage(*samples),
        Operation::Derivative => FilterSpec::Derivative,
        Operation::Integral { initial } => FilterSpec::Integral(scalar(log, initial)?),
        Operation::Magnitude => FilterSpec::Magnitude,
        Operation::Quaternion { axis, degrees } => FilterSpec::Quaternion(
            match axis.as_str() {
                "roll" => RpyAxis::Roll,
                "pitch" => RpyAxis::Pitch,
                "yaw" => RpyAxis::Yaw,
                _ => return Err(format!("Unknown quaternion axis {axis}")),
            },
            if *degrees {
                AngleUnit::Degrees
            } else {
                AngleUnit::Radians
            },
        ),
        Operation::Rust {
            path,
            outputs,
            output,
        } => {
            let base = base.canonicalize().map_err(|e| e.to_string())?;
            let script_path = base
                .join(path)
                .canonicalize()
                .map_err(|e| format!("Rust filter {}: {e}", path.display()))?;
            if path.is_absolute() || !script_path.starts_with(&base) {
                return Err("Rust filter path must stay inside profile directory".into());
            }
            let mut source = String::new();
            std::fs::File::open(script_path)
                .map_err(|e| e.to_string())?
                .take(262_145)
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            if source.len() > 262_144 {
                return Err("Rust filter exceeds 256 KiB".into());
            }
            FilterSpec::Rust(
                Arc::new(crate::script::Script {
                    source,
                    outputs: *outputs,
                }),
                *output,
            )
        }
    })
}
