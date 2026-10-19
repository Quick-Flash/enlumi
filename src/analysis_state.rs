use crate::{
    spectral::{self, Mode, Options},
    state::{Pane, Workbench},
};
use plot::time_series::{SeriesData, TimeSeries, TimeSeriesSet};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use zgui::prelude::*;

#[derive(Clone)]
pub struct AnalysisResult {
    pub id: u64,
    pub title: String,
    pub units: String,
    pub summary: String,
    pub series: TimeSeriesSet,
    pub heatmap: Option<spectral::Heatmap>,
}

#[derive(Clone, Copy)]
pub struct AnalysisUi {
    pub open: RwSignal<bool, LocalStorage>,
    pub sources: RwSignal<Vec<String>, LocalStorage>,
    pub disabled: RwSignal<std::collections::BTreeSet<String>, LocalStorage>,
    pub condition: RwSignal<String, LocalStorage>,
    pub mode: RwSignal<String, LocalStorage>,
    pub size: RwSignal<String, LocalStorage>,
    pub window: RwSignal<String, LocalStorage>,
    pub overlap: RwSignal<String, LocalStorage>,
    pub heatmap_bins: RwSignal<String, LocalStorage>,
    pub start: RwSignal<String, LocalStorage>,
    pub end: RwSignal<String, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub message: RwSignal<String, LocalStorage>,
    pub results: RwSignal<Vec<AnalysisResult>, LocalStorage>,
    pub revision: RwSignal<u64, LocalStorage>,
    pub drop: NodeRef,
    pub hover: RwSignal<bool, LocalStorage>,
    cancel: StoredValue<Option<Arc<AtomicBool>>, LocalStorage>,
}
impl AnalysisUi {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new_local(false),
            sources: RwSignal::new_local(Vec::new()),
            disabled: RwSignal::new_local(Default::default()),
            condition: RwSignal::new_local(String::new()),
            mode: RwSignal::new_local("fft".into()),
            size: RwSignal::new_local("2048".into()),
            window: RwSignal::new_local("hann".into()),
            overlap: RwSignal::new_local("50".into()),
            heatmap_bins: RwSignal::new_local("40".into()),
            start: RwSignal::new_local(String::new()),
            end: RwSignal::new_local(String::new()),
            busy: RwSignal::new_local(false),
            message: RwSignal::new_local(String::new()),
            results: RwSignal::new_local(Vec::new()),
            revision: RwSignal::new_local(0),
            drop: NodeRef::new(),
            hover: RwSignal::new_local(false),
            cancel: StoredValue::new_local(None),
        }
    }
    pub fn cancel(self) {
        if let Some(cancel) = self.cancel.get_value() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.revision.update(|revision| *revision += 1);
        self.busy.set(false);
    }
    pub fn reset(self) {
        self.cancel();
        self.sources.set(Vec::new());
        self.disabled.set(Default::default());
        self.results.set(Vec::new());
        self.condition.set(String::new());
        self.message.set(String::new());
        self.start.set(String::new());
        self.end.set(String::new());
    }
}
impl Workbench {
    pub fn analyze_plot(self, pane: Pane) {
        self.analysis.open.set(true);
        self.filters_open.set(false);
        self.add_analysis_sources(pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .filter(|line| {
                    !pane
                        .plot
                        .hidden
                        .with_untracked(|hidden| hidden.contains(&line.id.to_string()))
                })
                .map(|line| line.source.clone())
                .collect()
        }));
        let range = pane.plot.ranges.get_untracked().x;
        self.analysis.start.set(range.min.to_string());
        self.analysis.end.set(range.max.to_string());
    }
    pub fn add_analysis_sources(self, sources: Vec<String>) {
        self.analysis.cancel();
        self.analysis.sources.update(|all| {
            for source in sources {
                if !all.contains(&source) {
                    all.push(source);
                }
            }
        });
        self.analysis.message.set("Select signals, then Run. Results are snapshots; rerun after changing inputs, filters, or settings.".into());
    }
    pub fn run_analysis(self) {
        let ui = self.analysis;
        if self.loading.get_untracked()
            || self.derived_busy.get_untracked()
            || self.profiles.busy.get_untracked()
        {
            ui.message
                .set("Wait for log, profiles, and filters to finish".into());
            return;
        }
        let Some(log) = self.log.get_untracked() else {
            ui.message.set("Open a log first".into());
            return;
        };
        let parse_time = |value: String| -> Result<Option<f64>, String> {
            if value.trim().is_empty() {
                Ok(None)
            } else {
                value
                    .trim()
                    .parse::<f64>()
                    .map(Some)
                    .map_err(|_| "Time bounds must be numbers".into())
            }
        };
        let options = (|| {
            Ok::<_, String>(Options {
                mode: match ui.mode.get_untracked().as_str() {
                    "psd" => Mode::Psd,
                    "heatmap" => Mode::Heatmap,
                    "spectrogram" => Mode::Spectrogram,
                    _ => Mode::Fft,
                },
                size: ui
                    .size
                    .get_untracked()
                    .parse()
                    .map_err(|_| "Invalid FFT size")?,
                window: match ui.window.get_untracked().as_str() {
                    "hamming" => spectral::Window::Hamming,
                    "blackman" => spectral::Window::Blackman,
                    "rectangular" => spectral::Window::Rectangular,
                    _ => spectral::Window::Hann,
                },
                overlap: ui
                    .overlap
                    .get_untracked()
                    .parse()
                    .map_err(|_| "Invalid window overlap")?,
                heatmap_bins: ui
                    .heatmap_bins
                    .get_untracked()
                    .parse()
                    .map_err(|_| "Invalid heatmap bin count")?,
                start: parse_time(ui.start.get_untracked())?,
                end: parse_time(ui.end.get_untracked())?,
            })
        })();
        let options = match options {
            Ok(value) => value,
            Err(error) => {
                ui.message.set(error);
                return;
            }
        };
        let sources = ui.sources.with_untracked(|all| {
            all.iter()
                .filter(|source| {
                    !ui.disabled
                        .with_untracked(|disabled| disabled.contains(*source))
                })
                .cloned()
                .collect::<Vec<_>>()
        });
        if sources.is_empty() || sources.len() > 16 {
            ui.message.set("Enable between 1 and 16 signals".into());
            return;
        }
        let condition = ui.condition.get_untracked();
        if options.mode == Mode::Heatmap && condition.is_empty() {
            ui.message.set("Choose heatmap conditioning field".into());
            return;
        }
        let fields = self.derived.get_untracked();
        let names = sources
            .iter()
            .map(|source| (source.clone(), self.field_name(source)))
            .collect::<BTreeMap<_, _>>();
        let metadata = self.profiles.metadata.get_untracked();
        let condition_name = self.field_name(&condition);
        let condition_unit = metadata
            .get(&condition)
            .map_or_else(String::new, |m| m.unit.clone());
        let window_name = match options.window {
            spectral::Window::Hann => "Hann",
            spectral::Window::Hamming => "Hamming",
            spectral::Window::Blackman => "Blackman",
            spectral::Window::Rectangular => "Rectangular",
        };
        let completion = format!(
            "Complete. {window_name} window, {}% overlap, mean removed per window; gaps excluded. {}",
            options.overlap,
            match options.mode {
                Mode::Fft => "FFT shows RMS average of peak-amplitude spectra.",
                Mode::Psd => "PSD shows Welch mean power density.",
                Mode::Heatmap => "Heatmap averages PSD within conditioning bins.",
                Mode::Spectrogram => "Spectrogram shows PSD for each time window.",
            }
        );
        ui.cancel();
        let revision = ui.revision.get_untracked();
        let generation = self.generation.get_untracked();
        let cancel = Arc::new(AtomicBool::new(false));
        ui.cancel.set_value(Some(cancel.clone()));
        ui.busy.set(true);
        ui.message.set("Computing spectra…".into());
        self.task(async move {
            let (results, messages) = blocking(move || {
                let prepare = |source: &str| -> Result<SeriesData, String> {
                    if crate::derived::id(source).is_some() {
                        fields
                            .iter()
                            .find(|field| field.key() == source)
                            .and_then(|field| field.data.clone())
                            .ok_or_else(|| "Derived field unavailable".into())
                    } else {
                        log.prepare(source)
                    }
                };
                let conditioning = if options.mode == Mode::Heatmap {
                    match prepare(&condition) {
                        Ok(data) => Some(data),
                        Err(error) => {
                            return (Vec::new(), vec![format!("Conditioning field: {error}")]);
                        }
                    }
                } else {
                    None
                };
                let mut grouped = BTreeMap::<String, AnalysisResult>::new();
                let mut messages = Vec::new();
                for source in sources {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let name = &names[&source];
                    let unit = metadata.get(&source).map_or("unknown unit", |m| {
                        if m.unit.is_empty() {
                            "unknown unit"
                        } else {
                            &m.unit
                        }
                    });
                    let spectrum = prepare(&source).and_then(|data| {
                        spectral::compute(&data, conditioning.as_ref(), &options, &cancel)
                    });
                    let spectrum = match spectrum {
                        Ok(value) => value,
                        Err(error) => {
                            messages.push(format!("{name}: {error}"));
                            continue;
                        }
                    };
                    let units = if options.mode == Mode::Fft {
                        unit.to_owned()
                    } else {
                        format!("({unit})²/Hz")
                    };
                    let summary = format!(
                        "{name}: {:.2} samples/s · Δf {:.3} Hz · {} windows · {} skipped",
                        spectrum.rate,
                        spectrum.rate / options.size as f64,
                        spectrum.windows,
                        spectrum.skipped
                    );
                    let series = TimeSeries::new(
                        source.clone(),
                        name.clone(),
                        SeriesData::new(spectrum.points).expect("frequency bins sorted"),
                    );
                    let key = if matches!(options.mode, Mode::Heatmap | Mode::Spectrogram)
                        || unit == "unknown unit"
                    {
                        source
                    } else {
                        units.clone()
                    };
                    let id = revision.wrapping_mul(32) + grouped.len() as u64;
                    let entry = grouped.entry(key).or_insert_with(|| AnalysisResult {
                        id,
                        title: match options.mode {
                            Mode::Fft => "FFT amplitude".into(),
                            Mode::Psd => "Welch PSD".into(),
                            Mode::Heatmap => format!("{name} · FFT vs {condition_name}"),
                            Mode::Spectrogram => format!("{name} · Spectrogram"),
                        },
                        units: match options.mode {
                            Mode::Heatmap => format!(
                                "Y: {condition_name} [{condition_unit}] · color: dB re 1 {units}"
                            ),
                            Mode::Spectrogram => {
                                format!("Y: Frequency (Hz) · color: dB re 1 {units}")
                            }
                            _ => units,
                        },
                        summary: String::new(),
                        series: TimeSeriesSet::new(Vec::new()).expect("empty series valid"),
                        heatmap: spectrum.heatmap.clone(),
                    });
                    if !entry.summary.is_empty() {
                        entry.summary.push('\n');
                    }
                    entry.summary.push_str(&summary);
                    let mut all = entry.series.series().to_vec();
                    all.push(series);
                    entry.series = TimeSeriesSet::new(all).expect("unique input sources");
                }
                (grouped.into_values().collect::<Vec<_>>(), messages)
            })
            .await;
            if self.generation.get_untracked() != generation
                || ui.revision.get_untracked() != revision
            {
                return;
            }
            ui.busy.set(false);
            ui.cancel.set_value(None);
            ui.message.set(if messages.is_empty() {
                completion
            } else {
                messages.join("\n")
            });
            ui.results.set(results);
        });
    }
}
