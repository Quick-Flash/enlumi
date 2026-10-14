use plot::{PlotPoint, time_series::SeriesData};
use px4_ulog::full_parser::{ParsedData, SomeVec};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct SignalInfo {
    pub id: String,
    pub count: usize,
}

struct RawSignal {
    timestamps: Arc<[u64]>,
    values: SomeVec,
}

pub struct LogStore {
    pub name: String,
    pub catalogue: Vec<SignalInfo>,
    pub duration: f64,
    pub warnings: Vec<String>,
    origin: u64,
    signals: BTreeMap<String, RawSignal>,
}

impl LogStore {
    pub fn read(path: &Path) -> Result<Self, String> {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        // Upstream full parser exposes columns but not header timestamp.
        let mut header = [0; 16];
        std::fs::File::open(path)
            .and_then(|mut file| file.read_exact(&mut header))
            .map_err(|error| format!("Cannot read ULog header: {error}"))?;
        let origin = u64::from_le_bytes(header[8..16].try_into().unwrap());
        let path = path.to_str().ok_or("ULog path must be valid UTF-8")?;
        let parsed = std::panic::catch_unwind(|| px4_ulog::full_parser::read_file(path))
            .map_err(|_| "ULog decoder rejected malformed data".to_owned())?
            .map_err(|error| format!("Cannot read ULog: {error}"))?;
        Self::from_parsed(name, origin, parsed)
    }

    fn from_parsed(name: String, origin: u64, parsed: ParsedData) -> Result<Self, String> {
        let mut signals = BTreeMap::new();
        let mut warnings = Vec::new();
        if parsed.completeness.is_incomplete() {
            warnings.push(format!(
                "Incomplete ULog; showing recovered samples: {:?}",
                parsed.completeness
            ));
        }
        let mut last = origin;
        for (topic, instances) in parsed.messages {
            for (instance, mut fields) in instances {
                let prefix = format!("{topic}/{}", instance.value());
                let timestamps: Arc<[u64]> = match fields.remove("timestamp") {
                    Some(SomeVec::UInt64(values)) => values.into(),
                    Some(SomeVec::UInt32(values)) => {
                        values.into_iter().map(u64::from).collect::<Vec<_>>().into()
                    }
                    _ => {
                        warnings.push(format!("{prefix}: missing unsigned timestamp"));
                        continue;
                    }
                };
                if timestamps.windows(2).any(|pair| pair[1] < pair[0]) {
                    warnings.push(format!("{prefix}: decreasing timestamps; skipped"));
                    continue;
                }
                last = last.max(timestamps.last().copied().unwrap_or(last));
                for (field, values) in fields {
                    if field.starts_with("_padding") || matches!(values, SomeVec::Char(_)) {
                        continue;
                    }
                    if count(&values) != timestamps.len() {
                        warnings.push(format!("{prefix}/{field}: sample count mismatch; skipped"));
                        continue;
                    }
                    if timestamps.is_empty() {
                        continue;
                    }
                    signals.insert(
                        format!("{prefix}/{field}"),
                        RawSignal {
                            timestamps: Arc::clone(&timestamps),
                            values,
                        },
                    );
                }
            }
        }
        if signals.is_empty() {
            return Err("ULog contains no plottable timestamped signals".into());
        }
        let catalogue = signals
            .iter()
            .map(|(id, signal)| SignalInfo {
                id: id.clone(),
                count: count(&signal.values),
            })
            .collect();
        Ok(Self {
            name,
            catalogue,
            duration: relative_seconds(last, origin),
            warnings,
            origin,
            signals,
        })
    }

    pub fn prepare(&self, id: &str) -> Result<SeriesData, String> {
        let raw = self.signals.get(id).ok_or("Signal no longer exists")?;
        let mut points = Vec::with_capacity(raw.timestamps.len());
        for (index, &timestamp) in raw.timestamps.iter().enumerate() {
            points.push(PlotPoint {
                x: relative_seconds(timestamp, self.origin),
                y: value(&raw.values, index),
            });
        }
        SeriesData::new(points)
    }
}

fn relative_seconds(timestamp: u64, origin: u64) -> f64 {
    (i128::from(timestamp) - i128::from(origin)) as f64 / 1_000_000.0
}

fn count(values: &SomeVec) -> usize {
    match values {
        SomeVec::Int8(v) => v.len(),
        SomeVec::UInt8(v) => v.len(),
        SomeVec::Int16(v) => v.len(),
        SomeVec::UInt16(v) => v.len(),
        SomeVec::Int32(v) => v.len(),
        SomeVec::UInt32(v) => v.len(),
        SomeVec::Int64(v) => v.len(),
        SomeVec::UInt64(v) => v.len(),
        SomeVec::Float(v) => v.len(),
        SomeVec::Double(v) => v.len(),
        SomeVec::Bool(v) => v.len(),
        SomeVec::Char(v) => v.len(),
    }
}

fn value(values: &SomeVec, i: usize) -> f64 {
    match values {
        SomeVec::Int8(v) => v[i] as f64,
        SomeVec::UInt8(v) => v[i] as f64,
        SomeVec::Int16(v) => v[i] as f64,
        SomeVec::UInt16(v) => v[i] as f64,
        SomeVec::Int32(v) => v[i] as f64,
        SomeVec::UInt32(v) => v[i] as f64,
        SomeVec::Int64(v) => v[i] as f64,
        SomeVec::UInt64(v) => v[i] as f64,
        SomeVec::Float(v) => v[i] as f64,
        SomeVec::Double(v) => v[i],
        SomeVec::Bool(v) => u8::from(v[i]) as f64,
        SomeVec::Char(_) => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use px4_ulog::full_parser::{Completeness, MultiId};

    #[test]
    fn discovers_real_fixture_arrays_and_prepares_samples() {
        let log = LogStore::read(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap();
        let field = log
            .catalogue
            .iter()
            .find(|s| s.id == "esc_status/0/esc[5].esc_rpm")
            .unwrap();
        assert_eq!(log.prepare(&field.id).unwrap().samples().len(), field.count);
        let header = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        ))
        .unwrap();
        assert_eq!(
            log.origin,
            u64::from_le_bytes(header[8..16].try_into().unwrap())
        );
    }

    #[test]
    fn truncated_tail_keeps_samples_and_reports_warning() {
        let fixture = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        ));
        let complete = LogStore::read(fixture).unwrap();
        let mut bytes = std::fs::read(fixture).unwrap();
        // Record declares 100 bytes but file ends after one payload byte.
        bytes.extend_from_slice(&[100, 0, b'D', 0]);
        let path =
            std::env::temp_dir().join(format!("enlumi-truncated-{}.ulg", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let recovered = LogStore::read(&path);
        std::fs::remove_file(path).unwrap();
        let recovered = recovered.unwrap();
        assert_eq!(recovered.catalogue, complete.catalogue);
        assert!(recovered.warnings.iter().any(|warning| {
            warning.contains("Incomplete ULog") && warning.contains("Truncated")
        }));
    }

    #[test]
    fn preserves_instances_precision_and_reports_bad_topics() {
        let epoch = 9_000_000_000_000_000_u64;
        let fields = |timestamps| {
            BTreeMap::from([
                ("timestamp".into(), SomeVec::UInt64(timestamps)),
                ("value".into(), SomeVec::Double(vec![1.0, f64::NAN, 3.0])),
            ])
            .into_iter()
            .collect()
        };
        let parsed = ParsedData {
            completeness: Completeness::Complete,
            messages: std::collections::HashMap::from([(
                "sensor".into(),
                std::collections::HashMap::from([
                    (MultiId::new(0), fields(vec![epoch - 1, epoch, epoch + 1])),
                    (MultiId::new(1), fields(vec![epoch, epoch + 1, epoch + 2])),
                    (MultiId::new(2), fields(vec![epoch + 2, epoch, epoch + 1])),
                ]),
            )]),
        };
        let log = LogStore::from_parsed("fixture".into(), epoch, parsed).unwrap();
        assert_eq!(log.catalogue.len(), 2);
        assert_eq!(log.warnings.len(), 1);
        let data = log.prepare("sensor/0/value").unwrap();
        assert_eq!(data.samples()[0].x, -0.000001);
        assert_eq!(data.samples()[2].x, 0.000001);
        assert!(data.samples()[1].y.is_nan());
    }

    #[test]
    fn malformed_file_returns_error() {
        let path =
            std::env::temp_dir().join(format!("ulog-workbench-invalid-{}.ulg", std::process::id()));
        std::fs::write(&path, b"invalid ULog").unwrap();
        assert!(LogStore::read(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
