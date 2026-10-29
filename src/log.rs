mod blackbox;
mod tlog;
mod ulog_settings;

use plot::{PlotPoint, time_series::SeriesData};
use px4_ulog::full_parser::{ParsedData, SomeVec};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};

// Both cases are required by case-sensitive native file pickers.
pub const BLACKBOX_EXTENSIONS: &[&str] = &[
    "bbl", "BBL", "bfl", "BFL", "cfl", "CFL", "txt", "TXT", "log", "LOG",
];
pub const TLOG_EXTENSIONS: &[&str] = &["tlog", "TLOG"];

#[derive(Clone, Debug, PartialEq)]
pub struct SignalInfo {
    pub id: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogSetting {
    pub name: String,
    pub value: String,
    pub kind: String,
}

impl LogSetting {
    pub fn timing(&self) -> &'static str {
        if self.kind.starts_with("Parameter change") {
            "Change record · timestamp unavailable · selected snapshot, not flight history"
        } else if self.kind.starts_with("Initial parameter") {
            "Initial value · log start"
        } else if self.kind.starts_with("Default") {
            "Default value · not an observed change"
        } else {
            "Recorded header · flight snapshot"
        }
    }

    pub fn decoder_header(&self) -> bool {
        self.kind == "Header"
            && (self.name.starts_with("Field ")
                || matches!(
                    self.name.as_str(),
                    "Product" | "Data version" | "I interval" | "P interval" | "P ratio"
                ))
    }

    pub fn numbers(&self) -> Option<Vec<f64>> {
        let value = self.value.trim();
        let value = value
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(value);
        value
            .split(',')
            .map(|part| part.trim().parse::<f64>().ok().filter(|n| n.is_finite()))
            .collect()
    }
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
    pub blackbox_headers: BTreeMap<usize, bbl_parser::BBLHeader>,
    pub settings: BTreeMap<usize, Vec<LogSetting>>,
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
        let mut prefix = [0; 64];
        let length = std::fs::File::open(path)
            .and_then(|mut file| file.read(&mut prefix))
            .map_err(|error| format!("Cannot read log: {error}"))?;
        if prefix[..length].starts_with(b"ULog\x01\x12\x35") {
            return Self::read_ulog(path, name);
        }
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if extension.eq_ignore_ascii_case("tlog") || tlog::has_header(&prefix[..length]) {
            return Self::read_tlog(path, name);
        }
        if prefix[..length].starts_with(b"H Product:Blackbox")
            || BLACKBOX_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        {
            return Self::read_blackbox(path, name);
        }
        Err(
            "Unsupported log format; open ULog (.ulg), Blackbox (.bbl, .bfl, .cfl, .txt, .log), or MAVLink telemetry (.tlog)"
                .into(),
        )
    }

    fn read_ulog(path: &Path, name: String) -> Result<Self, String> {
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
        let mut store = Self::from_parsed(name, origin, parsed)?;
        // The full parser exposes sample columns only. Its public streaming
        // API supplies parameter and information records in a separate pass.
        match std::panic::catch_unwind(|| ulog_settings::read(path)) {
            Ok(Ok(settings)) => {
                store.settings.insert(1, settings);
            }
            Ok(Err(error)) => store.warnings.push(error),
            Err(_) => store
                .warnings
                .push("ULog settings decoder rejected malformed data".into()),
        }
        store.add_setting_signals();
        Ok(store)
    }

    pub(crate) fn from_parsed(
        name: String,
        origin: u64,
        parsed: ParsedData,
    ) -> Result<Self, String> {
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
            blackbox_headers: BTreeMap::new(),
            settings: BTreeMap::new(),
            warnings,
            origin,
            signals,
        })
    }

    pub fn setting(&self, id: &str) -> Option<&LogSetting> {
        let mut parts = id.strip_prefix("@setting/")?.split('/');
        let flight = parts.next()?.parse::<usize>().ok()?;
        let index = parts.next()?.parse::<usize>().ok()?;
        if let Some(element) = parts.next() {
            element.parse::<usize>().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        self.settings.get(&flight)?.get(index)
    }

    pub fn setting_number(&self, id: &str) -> Option<f64> {
        let values = self.setting(id)?.numbers()?;
        match id.split('/').nth(3) {
            Some(index) => values.get(index.parse::<usize>().ok()?).copied(),
            None if values.len() == 1 => values.first().copied(),
            _ => None,
        }
    }

    pub fn setting_label(&self, id: &str) -> Option<String> {
        let setting = self.setting(id)?;
        let suffix = id
            .split('/')
            .nth(3)
            .map_or_else(String::new, |index| format!("[{index}]"));
        let value = self.setting_number(id).map_or_else(
            || setting.value.trim().to_owned(),
            |value| value.to_string(),
        );
        let label = format!("{}{suffix} = {value} ({})", setting.name, setting.kind);
        if self.settings.len() > 1 {
            Some(format!(
                "Flight {} / {label}",
                id.strip_prefix("@setting/")?.split('/').next()?
            ))
        } else {
            Some(label)
        }
    }

    pub(super) fn add_setting_signals(&mut self) {
        for (flight, settings) in &self.settings {
            for (index, setting) in settings.iter().enumerate() {
                if setting.decoder_header() {
                    continue;
                }
                if let Some(values) = setting.numbers() {
                    for element in 0..values.len() {
                        let suffix = if values.len() == 1 {
                            String::new()
                        } else {
                            format!("/{element}")
                        };
                        self.catalogue.push(SignalInfo {
                            id: format!("@setting/{flight}/{index}{suffix}"),
                            count: if self.duration > 0.0 { 2 } else { 1 },
                        });
                    }
                }
            }
        }
    }

    pub fn duration_for(&self, id: &str) -> f64 {
        if self.blackbox_headers.is_empty() {
            return self.duration;
        }
        if let Some(flight) = id
            .strip_prefix("@setting/")
            .and_then(|id| id.split('/').next())
        {
            let prefix = format!("blackbox/{flight}/");
            return self
                .signals
                .iter()
                .filter(|(id, _)| id.starts_with(&prefix))
                .filter_map(|(_, raw)| raw.timestamps.last())
                .max()
                .map_or(0., |timestamp| relative_seconds(*timestamp, self.origin));
        }
        self.signals
            .get(id)
            .and_then(|raw| raw.timestamps.last())
            .map_or(self.duration, |timestamp| {
                relative_seconds(*timestamp, self.origin)
            })
    }

    pub fn prepare(&self, id: &str) -> Result<SeriesData, String> {
        if self.setting(id).is_some() {
            let y = self
                .setting_number(id)
                .ok_or("Parameter needs a finite scalar or array element")?;
            let mut points = vec![PlotPoint { x: 0.0, y }];
            let duration = self.duration_for(id);
            if duration > 0.0 {
                points.push(PlotPoint { x: duration, y });
            }
            return SeriesData::new(points);
        }
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
