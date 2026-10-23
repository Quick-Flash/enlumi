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
    pub title: String,
    pub units: String,
    pub summary: String,
    pub series: TimeSeriesSet,
    pub heatmap: Option<spectral::Heatmap>,
    pub mode: Mode,
    pub options: Options,
    pub condition: String,
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
    pub remove_dc: RwSignal<bool, LocalStorage>,
    pub picking_range: RwSignal<bool, LocalStorage>,
    pub separate: RwSignal<bool, LocalStorage>,
    pub run_selection: RwSignal<bool, LocalStorage>,
    pub target: RwSignal<Option<u64>, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub message: RwSignal<String, LocalStorage>,
    pub results: RwSignal<Vec<AnalysisResult>, LocalStorage>,
    pub revision: RwSignal<u64, LocalStorage>,
    pub drop: NodeRef,
    pub condition_drop: NodeRef,
    pub condition_hover: RwSignal<bool, LocalStorage>,
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
            remove_dc: RwSignal::new_local(true),
            picking_range: RwSignal::new_local(false),
            separate: RwSignal::new_local(false),
            run_selection: RwSignal::new_local(false),
            target: RwSignal::new_local(None),
            busy: RwSignal::new_local(false),
            message: RwSignal::new_local(String::new()),
            results: RwSignal::new_local(Vec::new()),
            revision: RwSignal::new_local(0),
            drop: NodeRef::new(),
            condition_drop: NodeRef::new(),
            condition_hover: RwSignal::new_local(false),
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
        self.picking_range.set(false);
        self.run_selection.set(false);
        self.target.set(None);
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
    pub fn new_analysis(self) {
        self.analysis().cancel();
        self.analysis().run_selection.set(false);
        self.analysis().picking_range.set(false);
        self.analysis().target.set(None);
        self.analysis().open.set(true);
        self.filters_open.set(false);
        self.analysis()
            .message
            .set("Choose analysis type, then drag signals onto a plot".into());
    }
    pub fn select_analysis_mode(self, mode: &str) {
        if self.analysis().mode.get_untracked() != mode {
            self.new_analysis();
            self.analysis().mode.set(mode.into());
        }
    }
    pub fn set_analysis_condition(self, sources: Vec<String>) {
        if sources.len() != 1 {
            self.analysis()
                .message
                .set("Drop one conditioning field".into());
            return;
        }
        self.analysis().cancel();
        self.analysis().condition.set(sources[0].clone());
        self.analysis()
            .message
            .set("Conditioning field set. Drag signal onto plot to create heatmap.".into());
    }
    pub fn create_selected_spectra(self, mode: &'static str) {
        if self.loading.get_untracked()
            || self.derived_busy.get_untracked()
            || self.profiles.busy.get_untracked()
        {
            self.analysis().open.set(true);
            self.analysis()
                .message
                .set("Wait for log, profiles, and filters to finish".into());
            return;
        }
        self.analysis().reset();
        self.analysis().mode.set(mode.into());
        self.analysis().separate.set(true);
        self.analysis().run_selection.set(true);
        self.analyze_selection();
        if self.analysis().run_selection.get_untracked() {
            // Profile selections resume after asynchronous preparation.
            if !self.profiles.busy.get_untracked()
                && self.profiles.pending_drop.with_value(Option::is_none)
            {
                self.analysis().run_selection.set(false);
                self.run_analysis();
            }
        }
    }
    pub fn edit_analysis(self, pane: Pane) {
        let Some(result) = pane.analysis.get_untracked() else {
            return;
        };
        let ui = self.analysis();
        ui.reset();
        ui.open.set(true);
        self.filters_open.set(false);
        ui.target.set(Some(pane.id));
        ui.sources.set(pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .filter_map(|line| line.source.strip_prefix("@spectrum/").map(str::to_owned))
                .collect()
        }));
        if result.heatmap.is_some() {
            ui.sources.set(
                result
                    .series
                    .series()
                    .iter()
                    .map(|series| series.id().to_owned())
                    .collect(),
            );
        }
        ui.mode.set(
            match result.mode {
                Mode::Fft => "fft",
                Mode::Psd => "psd",
                Mode::Heatmap => "heatmap",
                Mode::Spectrogram => "spectrogram",
            }
            .into(),
        );
        ui.window.set(
            match result.options.window {
                spectral::Window::Hann => "hann",
                spectral::Window::Hamming => "hamming",
                spectral::Window::Blackman => "blackman",
                spectral::Window::Rectangular => "rectangular",
            }
            .into(),
        );
        ui.size.set(result.options.size.to_string());
        ui.overlap.set(result.options.overlap.to_string());
        ui.heatmap_bins.set(result.options.heatmap_bins.to_string());
        ui.start.set(
            result
                .options
                .start
                .map_or_else(String::new, |value| value.to_string()),
        );
        ui.end.set(
            result
                .options
                .end
                .map_or_else(String::new, |value| value.to_string()),
        );
        ui.remove_dc.set(result.options.remove_dc);
        ui.condition.set(result.condition);
        ui.separate.set(false);
    }
    pub fn analyze_selection(self) {
        self.analysis().target.set(None);
        let sources = self
            .selected_field
            .get_untracked()
            .map(|source| self.drag_fields(source))
            .unwrap_or_default();
        self.analysis().open.set(true);
        self.filters_open.set(false);
        self.analysis().start.set(String::new());
        self.analysis().end.set(String::new());
        self.queue_field_sources(
            sources
                .into_iter()
                .filter(|source| !source.starts_with("@setting/"))
                .collect(),
            crate::profile_state::Destination::Analysis,
            None,
        );
    }

    pub fn use_visible_analysis_range(self) {
        let pane = self.pane();
        if pane.analysis.get_untracked().is_some() {
            self.analysis()
                .message
                .set("Select a time plot first".into());
            return;
        }
        let range = pane.plot.ranges.get_untracked().x;
        self.analysis().start.set(range.min.to_string());
        self.analysis().end.set(range.max.to_string());
        self.analysis().picking_range.set(false);
    }
    pub fn add_analysis_sources(self, sources: Vec<String>) {
        self.analysis().cancel();
        self.analysis().sources.update(|all| {
            for source in sources {
                if !all.contains(&source) {
                    all.push(source);
                }
            }
        });
        self.analysis().message.set("Drag signals onto a plot to create analysis. Results are snapshots; drag again after changing settings.".into());
    }
    pub fn run_analysis(self) {
        self.run_analysis_request(None, self.analysis().target.get_untracked(), false);
    }
    pub fn drop_analysis_sources(self, sources: Vec<String>, target: Option<u64>) {
        if self.analysis().busy.get_untracked() {
            self.analysis().message.set(
                "Wait for current analysis to finish, or Cancel before dropping another signal"
                    .into(),
            );
            return;
        }
        let mode = self.analysis().mode.get_untracked();
        let accepts = |pane: &Pane| {
            pane.analysis.with_untracked(|result| {
                result.as_ref().is_some_and(|result| {
                    result.heatmap.is_none()
                        && matches!(
                            (mode.as_str(), result.mode),
                            ("fft", Mode::Fft) | ("psd", Mode::Psd)
                        )
                })
            })
        };
        let target = if matches!(mode.as_str(), "fft" | "psd") {
            target
                .and_then(|id| self.find(id))
                .filter(accepts)
                .or_else(|| {
                    self.panes()
                        .with_untracked(|panes| panes.iter().copied().find(accepts))
                })
                .map(|pane| pane.id)
        } else {
            None
        };
        self.run_analysis_request(Some(sources), target, true);
    }
    fn run_analysis_request(
        self,
        requested: Option<Vec<String>>,
        target: Option<u64>,
        append: bool,
    ) {
        self.bind_view()
            .start_analysis_request(requested, target, append);
    }
    fn start_analysis_request(
        self,
        requested: Option<Vec<String>>,
        target: Option<u64>,
        append: bool,
    ) {
        let ui = self.analysis();
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
                size: if matches!(ui.mode.get_untracked().as_str(), "heatmap" | "spectrogram") {
                    ui.size
                        .get_untracked()
                        .parse()
                        .map_err(|_| "Choose a usable FFT window size")?
                } else {
                    0
                },
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
                remove_dc: ui.remove_dc.get_untracked(),
            })
        })();
        let options = match options {
            Ok(value) => value,
            Err(error) => {
                ui.message.set(error);
                return;
            }
        };
        let sources = requested.unwrap_or_else(|| {
            ui.sources.with_untracked(|all| {
                all.iter()
                    .filter(|source| {
                        !ui.disabled
                            .with_untracked(|disabled| disabled.contains(*source))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
        });
        if sources.is_empty() {
            ui.message.set("Enable at least one signal".into());
            return;
        }
        let separate = !append && ui.separate.get_untracked();
        let condition = ui.condition.get_untracked();
        if options.mode == Mode::Heatmap && condition.is_empty() {
            ui.message.set("Choose heatmap conditioning field".into());
            return;
        }
        let fields = self.derived.get_untracked();
        let names = sources
            .iter()
            .map(|source| (source.clone(), self.field_name_untracked(source)))
            .collect::<BTreeMap<_, _>>();
        let metadata = self.profiles.metadata.get_untracked();
        let condition_name = self.field_name_untracked(&condition);
        let condition_unit = metadata
            .get(&condition)
            .map_or_else(String::new, |m| m.unit.clone());
        let window_name = match options.window {
            spectral::Window::Hann => "Hann",
            spectral::Window::Hamming => "Hamming",
            spectral::Window::Blackman => "Blackman",
            spectral::Window::Rectangular => "Rectangular",
        };
        let overlap_description = if options.mode.windowed() {
            format!("{}% overlap", options.overlap)
        } else {
            "longest valid run".into()
        };
        let completion = format!(
            "Complete. {window_name} window · {overlap_description} · {}. {}",
            if options.remove_dc {
                "DC removed"
            } else {
                "DC retained"
            },
            match options.mode {
                Mode::Fft =>
                    "FFT uses longest valid run; brief dropouts interpolated; no zero padding.",
                Mode::Psd =>
                    "PSD uses longest valid run; brief dropouts interpolated; no zero padding.",
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
                        "{name}: {:.2} samples/s · Δf {:.3} Hz · {} windows · {} skipped windows · {} excluded samples",
                        spectrum.rate,
                        spectrum.rate / spectrum.size as f64,
                        spectrum.windows,
                        spectrum.skipped,
                        spectrum.excluded_samples
                    );
                    let series = TimeSeries::new(
                        source.clone(),
                        name.clone(),
                        SeriesData::new(spectrum.points).expect("frequency bins sorted"),
                    );
                    let key = if (append || target.is_some()) && !options.mode.windowed() {
                        String::new()
                    } else if separate
                        || matches!(options.mode, Mode::Heatmap | Mode::Spectrogram)
                        || unit == "unknown unit"
                    {
                        source.clone()
                    } else {
                        units.clone()
                    };
                    let entry = grouped.entry(key).or_insert_with(|| AnalysisResult {
                        mode: options.mode,
                        options: options.clone(),
                        condition: condition.clone(),
                        title: match options.mode {
                            Mode::Fft => "FFT amplitude".into(),
                            Mode::Psd => "PSD".into(),
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
                    if separate && !options.mode.windowed() {
                        entry.title = format!(
                            "{name} · {}",
                            if options.mode == Mode::Psd {
                                "PSD"
                            } else {
                                "FFT"
                            }
                        );
                    }
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
            if !self.view_exists() || self.generation.get_untracked() != generation
                || ui.revision.get_untracked() != revision
            {
                return;
            }
            ui.busy.set(false);
            ui.cancel.set_value(None);
            ui.message.set(if messages.is_empty() {
                format!("{completion}\n{}", results.iter().map(|result| result.summary.as_str()).collect::<Vec<_>>().join("\n"))
            } else {
                messages.join("\n")
            });
            if target.is_some_and(|id|self.find(id).is_none()) {return;}
            for (index, result) in results.iter().enumerate() {
                if let Some(pane) = target.filter(|_| append || index == 0).and_then(|id| self.find(id)) {
                    let mut result = result.clone();
                    if append && let Some(previous) = pane.analysis.get_untracked() {
                        let mut series = pane.lines.with_untracked(|lines| lines.iter().filter_map(|line| {
                            let source = line.source.strip_prefix("@spectrum/")?;
                            (!result.series.series().iter().any(|series| series.id() == source)).then(|| TimeSeries::new(source, line.name.clone(), line.data.clone()))
                        }).collect::<Vec<_>>());
                        series.extend_from_slice(result.series.series());
                        result.series = TimeSeriesSet::new(series).expect("unique spectral sources");
                        result.summary = format!("{}\n{}", previous.summary, result.summary);
                    }
                    self.set_analysis_panel(pane, result);
                    if append { self.activate(pane.id); }
                } else {
                    self.add_analysis_panel(result.clone());
                }
            }
            ui.results.set(results);
        });
    }
}

impl Workbench {
    pub fn add_analysis_panel(self, result: AnalysisResult) -> Pane {
        let pane = self.add_plot(self.active().get_untracked(), false, true);
        pane.links.set(plot::zgui_link::LinkOptions {
            x: false,
            y: false,
            pointer: false,
        });
        self.set_analysis_panel(pane, result);
        pane
    }
    pub(crate) fn set_analysis_panel(self, pane: Pane, result: AnalysisResult) {
        let updating = pane.analysis.with_untracked(Option::is_some);
        if updating {
            pane.plot.auto_fit.set(false);
        }
        self.profiles.titles.update(|titles| {
            titles
                .entry(pane.id)
                .or_insert_with(|| result.title.clone());
        });
        if result.heatmap.is_none() {
            pane.lines.set(
                result
                    .series
                    .series()
                    .iter()
                    .map(|series| crate::state::Line {
                        id: pane
                            .lines
                            .with_untracked(|lines| {
                                lines
                                    .iter()
                                    .find(|line| {
                                        line.source.strip_prefix("@spectrum/") == Some(series.id())
                                    })
                                    .map(|line| line.id)
                            })
                            .unwrap_or_else(|| self.id()),
                        // Snapshot identity prevents live derived-field updates replacing spectral data.
                        source: format!("@spectrum/{}", series.id()),
                        name: series.label().to_owned(),
                        data: series.data().clone(),
                        revision: 0,
                    })
                    .collect(),
            );
        }
        if result.heatmap.is_none() {
            self.ensure_line_colors(pane);
            let ids = pane
                .lines
                .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>());
            pane.drawing.update(|drawing| {
                drawing.retain(|id, _| ids.contains(id));
                for id in ids {
                    drawing.entry(id).or_default();
                }
            });
        }
        pane.analysis.set(Some(result));
        if !updating {
            pane.fit_axes(plot::interaction::Axes::BOTH);
        }
    }

    pub fn analysis_size_job(self) -> impl FnOnce() -> Vec<usize> + Send {
        let ui = self.analysis();
        let start = ui.start.get().trim().parse::<f64>().ok();
        let end = ui.end.get().trim().parse::<f64>().ok();
        let sources = ui.sources.get();
        let disabled = ui.disabled.get();
        let derived = self.derived.get();
        let log = self.log.get();
        let mode_name = ui.mode.get();
        let overlap = ui.overlap.get().parse::<usize>().unwrap_or(100);
        let rows = ui.heatmap_bins.get().parse::<usize>().unwrap_or(0);
        move || {
            if !matches!(mode_name.as_str(), "heatmap" | "spectrogram") {
                return Vec::new();
            }
            let mode = if mode_name == "heatmap" {
                Mode::Heatmap
            } else {
                Mode::Spectrogram
            };
            let mut choices: Option<Vec<usize>> = None;
            for source in sources.iter().filter(|source| !disabled.contains(*source)) {
                let data = if crate::derived::id(source).is_some() {
                    derived
                        .iter()
                        .find(|field| field.key() == *source)
                        .and_then(|field| field.data.clone())
                } else {
                    log.as_ref().and_then(|log| log.prepare(source).ok())
                };
                let available = data.map_or_else(Vec::new, |data| {
                    spectral::window_sizes(&data, start, end, mode, overlap, rows)
                });
                if let Some(choices) = &mut choices {
                    choices.retain(|size| available.contains(size));
                } else {
                    choices = Some(available);
                }
            }
            choices.unwrap_or_default()
        }
    }
}
