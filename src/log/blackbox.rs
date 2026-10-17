use super::{LogSetting, LogStore, RawSignal, SignalInfo};
use bbl_parser::{BBLLog, ExportOptions};
use px4_ulog::full_parser::SomeVec;
use std::{collections::BTreeMap, path::Path, sync::Arc};

impl LogStore {
    pub(super) fn read_blackbox(path: &Path, name: String) -> Result<Self, String> {
        let logs = std::panic::catch_unwind(|| {
            bbl_parser::parse_bbl_file_all_logs(path, ExportOptions::default(), false)
        })
        .map_err(|_| "Blackbox decoder rejected malformed data".to_owned())?
        .map_err(|error| format!("Cannot read Blackbox: {error:#}"))?;
        Self::from_blackbox(name, logs)
    }

    fn from_blackbox(name: String, logs: Vec<BBLLog>) -> Result<Self, String> {
        let mut signals = BTreeMap::new();
        let mut warnings = Vec::new();
        let mut blackbox_headers = BTreeMap::new();
        let mut settings = BTreeMap::new();
        let mut last = 0;
        for log in logs {
            let prefix = format!("blackbox/{}", log.log_number);
            if log.stats.failed_frames > 0 {
                warnings.push(format!(
                    "{prefix}: decoder rejected {} frames; showing recovered samples",
                    log.stats.failed_frames
                ));
            }
            if log.stats.total_frames > 1_000_000 || log.stats.failed_frames > 10_000 {
                warnings.push(format!(
                    "{prefix}: decoder safety limit reached; log truncated"
                ));
            }
            let signed: BTreeMap<_, _> = log
                .header
                .i_frame_def
                .fields
                .iter()
                .chain(&log.header.s_frame_def.fields)
                .map(|field| (field.name.as_str(), field.signed))
                .collect();
            let mut timestamps = Vec::new();
            let mut columns = BTreeMap::<String, Vec<f64>>::new();
            let mut previous = None::<u32>;
            let mut elapsed = 0_u64;
            let mut skipped = 0;
            for frame in log.frames {
                // Slow fields are already merged into I/P frames by the decoder.
                // GPS, home and event records have separate schemas and clocks.
                if !matches!(frame.frame_type, 'I' | 'P') {
                    continue;
                }
                let Some(&time) = frame.data.get("time") else {
                    skipped += 1;
                    continue;
                };
                // Decoder stores unsigned fields as i32. Recover their bits before
                // subtracting, including signed-boundary and 32-bit clock wraps.
                let time = time as u32;
                if let Some(previous) = previous {
                    let delta = time.wrapping_sub(previous);
                    if delta > i32::MAX as u32 {
                        skipped += 1;
                        continue;
                    }
                    elapsed += u64::from(delta);
                }
                previous = Some(time);
                let index = timestamps.len();
                timestamps.push(elapsed);
                for (field, value) in frame.data {
                    let value = if signed.get(field.as_str()).copied().unwrap_or(true) {
                        f64::from(value)
                    } else {
                        f64::from(value as u32)
                    };
                    let values = columns.entry(field).or_default();
                    values.resize(index, f64::NAN);
                    values.push(value);
                }
            }
            settings.insert(
                log.log_number,
                log.header
                    .all_headers
                    .iter()
                    .map(|line| {
                        let line = line.strip_prefix("H ").unwrap_or(line);
                        let (name, value) = line.split_once(':').unwrap_or((line, ""));
                        LogSetting {
                            name: name.to_owned(),
                            value: value.to_owned(),
                            kind: "Header".into(),
                        }
                    })
                    .collect(),
            );
            blackbox_headers.insert(log.log_number, log.header);
            if skipped > 0 {
                warnings.push(format!(
                    "{prefix}: skipped {skipped} frames with invalid timestamps"
                ));
            }
            if timestamps.is_empty() {
                warnings.push(format!("{prefix}: no timestamped main frames"));
                continue;
            }
            last = last.max(elapsed);
            let timestamps: Arc<[u64]> = timestamps.into();
            for (field, mut values) in columns {
                values.resize(timestamps.len(), f64::NAN);
                signals.insert(
                    format!("{prefix}/{field}"),
                    RawSignal {
                        timestamps: Arc::clone(&timestamps),
                        values: SomeVec::Double(values),
                    },
                );
            }
        }
        if signals.is_empty() {
            return Err("Blackbox contains no plottable timestamped signals".into());
        }
        let catalogue = signals
            .iter()
            .map(|(id, signal)| SignalInfo {
                id: id.clone(),
                count: signal.timestamps.len(),
            })
            .collect();
        let mut store = Self {
            name,
            catalogue,
            duration: last as f64 / 1_000_000.0,
            warnings,
            blackbox_headers,
            settings,
            origin: 0,
            signals,
        };
        store.add_setting_signals();
        Ok(store)
    }
}
