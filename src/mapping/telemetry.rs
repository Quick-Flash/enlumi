//! Time-aligned map colors, shared by 2D paths, 3D paths and their scale.
use crate::{
    state::{Line, Pane, Workbench},
    trajectory::{Spec, Tracks},
};
use plot::PlotPoint;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zgui::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorBy {
    #[default]
    Trace,
    Speed,
    Altitude,
    Channel,
}
#[derive(Default)]
pub struct Coloring {
    pub samples: BTreeMap<u64, Arc<[f64]>>,
    pub range: Option<[f64; 2]>,
    pub label: String,
    pub scales: BTreeMap<u64, Scale>,
    pub times: BTreeMap<u64, Arc<[f64]>>,
    pub tints: Vec<crate::state_tint::Tint>,
}
pub struct Scale {
    pub gradient: crate::trace_color::Gradient,
    pub label: String,
}
impl Coloring {
    pub fn bucket_for(&self, id: u64, value: f64) -> Option<u8> {
        let range = if let Some(scale) = self.scales.get(&id) {
            Some(scale.gradient.range)
        } else if self.scales.is_empty() {
            self.range
        } else {
            None
        };
        let [min, max] = range?;
        value.is_finite().then(|| {
            if min == max {
                31
            } else {
                (((value - min) / (max - min)).clamp(0., 1.) * 63.).round() as u8
            }
        })
    }
    pub fn palette_for(&self, id: u64, bucket: u8) -> [u8; 4] {
        if let Some(scale) = self.scales.get(&id) {
            let [r, g, b] = scale.gradient.color(f64::from(bucket.min(63)) / 63.);
            [r, g, b, 255]
        } else {
            palette(bucket)
        }
    }
    pub fn shade(&self, id: u64, value: f64, time: f64, fallback: [u8; 4]) -> [u8; 4] {
        let gradient =
            self.scales.contains_key(&id) || (self.scales.is_empty() && self.range.is_some());
        let base = if gradient {
            self.bucket_for(id, value)
                .map_or([145, 145, 145, fallback[3]], |bucket| {
                    let [r, g, b, _] = self.palette_for(id, bucket);
                    [r, g, b, fallback[3]]
                })
        } else {
            fallback
        };
        crate::state_tint::at(&self.tints, time, base)
    }
    pub fn color(&self, id: u64, index: usize, fallback: [u8; 4]) -> [u8; 4] {
        let value = self
            .samples
            .get(&id)
            .and_then(|values| values.get(index))
            .copied()
            .unwrap_or(f64::NAN);
        let time = self
            .times
            .get(&id)
            .and_then(|times| times.get(index))
            .copied()
            .unwrap_or(f64::NAN);
        self.shade(id, value, time, fallback)
    }
}
pub fn palette(bucket: u8) -> [u8; 4] {
    const STOPS: [[f64; 3]; 5] = [
        [68., 1., 84.],
        [59., 82., 139.],
        [33., 145., 140.],
        [94., 201., 98.],
        [253., 231., 37.],
    ];
    let at = f64::from(bucket.min(63)) / 63. * 4.;
    let i = (at.floor() as usize).min(3);
    let rgb = std::array::from_fn::<_, 3, _>(|axis| {
        (STOPS[i][axis] + (STOPS[i + 1][axis] - STOPS[i][axis]) * (at - i as f64)).round() as u8
    });
    [rgb[0], rgb[1], rgb[2], 255]
}
#[derive(Clone, Copy)]
pub struct ColorContext(pub Signal<Arc<Coloring>, LocalStorage>);

pub fn prepare(data: &Tracks, lines: &[Line], spec: &Spec, hidden: &BTreeSet<String>) -> Coloring {
    let mode = spec.map.unwrap_or_default().coloring;
    if mode == ColorBy::Trace {
        return Coloring::default();
    }
    let label = match mode {
        ColorBy::Speed => "Ground speed · m/s".into(),
        ColorBy::Altitude => "Bound Z / altitude · m".into(),
        ColorBy::Channel => lines
            .iter()
            .find(|line| line.source == spec.color_source)
            .map_or_else(|| "Telemetry channel".into(), |line| line.name.clone()),
        ColorBy::Trace => unreachable!(),
    };
    let pairs = spec.pairs();
    let mut samples = BTreeMap::new();
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    for trace in &data.traces {
        if hidden.contains(&trace.id.to_string()) {
            continue;
        }
        let reference = trace
            .track
            .times
            .iter()
            .map(|&x| PlotPoint { x, y: 0. })
            .collect::<Vec<_>>();
        let altitude = lines
            .iter()
            .find(|line| line.id == trace.id)
            .and_then(|line| pairs.iter().position(|pair| pair.y == line.source))
            .and_then(|index| spec.altitudes.get(&index))
            .cloned()
            .unwrap_or_default();
        let source = match mode {
            ColorBy::Altitude => &altitude.source,
            _ => &spec.color_source,
        };
        let mut aligned = lines
            .iter()
            .find(|line| line.source == *source)
            .map(|line| crate::filter_multi::Aligned::new(&line.data, &reference));
        let mut values = vec![f64::NAN; reference.len()];
        for (index, value) in values.iter_mut().enumerate() {
            let p = trace.track.points[index];
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            *value = match mode {
                ColorBy::Speed if index > 0 => {
                    let a = trace.track.points[index - 1];
                    let dt = trace.track.times[index] - trace.track.times[index - 1];
                    if dt > 0. && a.x.is_finite() && a.y.is_finite() {
                        let scale = super::terrain::ground_scale(super::camera::Camera {
                            center: [(a.x + p.x) / 2., (a.y + p.y) / 2.],
                            zoom: 0.,
                        });
                        (p.x - a.x).hypot(p.y - a.y) * scale / dt
                    } else {
                        f64::NAN
                    }
                }
                ColorBy::Speed => f64::NAN,
                _ => {
                    aligned
                        .as_mut()
                        .map_or(f64::NAN, |aligned| aligned.at(index, reference[index].x))
                        * if mode == ColorBy::Altitude {
                            altitude.scale
                        } else {
                            1.
                        }
                }
            };
        }
        if mode == ColorBy::Speed {
            for index in 0..values.len().saturating_sub(1) {
                if !values[index].is_finite()
                    && values[index + 1].is_finite()
                    && trace.track.points[index].x.is_finite()
                    && trace.track.points[index].y.is_finite()
                {
                    values[index] = values[index + 1];
                }
            }
        }
        if mode == ColorBy::Altitude
            && altitude.local
            && altitude.at_first
            && let Some(first) = values.iter().find(|v| v.is_finite()).copied()
        {
            for value in &mut values {
                *value -= first;
            }
        }
        for &value in &values {
            if value.is_finite() {
                range[0] = range[0].min(value);
                range[1] = range[1].max(value);
            }
        }
        samples.insert(trace.id, values.into());
    }
    Coloring {
        samples,
        range: range.iter().all(|v| v.is_finite()).then_some(range),
        label,
        ..Default::default()
    }
}
pub fn style_for(
    spec: &Spec,
    styles: &BTreeMap<u64, crate::trace_color::TraceColor>,
    id: u64,
) -> crate::trace_color::TraceColor {
    styles
        .get(&id)
        .copied()
        .unwrap_or_else(|| crate::trace_color::TraceColor {
            mode: match spec.map.unwrap_or_default().coloring {
                ColorBy::Speed => crate::trace_color::Mode::Speed,
                ColorBy::Altitude => crate::trace_color::Mode::Height,
                _ => crate::trace_color::Mode::Solid,
            },
            ..Default::default()
        })
}
pub fn prepare_styled(
    data: &Tracks,
    lines: &[Line],
    spec: &Spec,
    hidden: &BTreeSet<String>,
    styles: &BTreeMap<u64, crate::trace_color::TraceColor>,
    tints: Vec<crate::state_tint::Tint>,
) -> Coloring {
    let mut output = Coloring {
        tints,
        ..Default::default()
    };
    let mut speed = None;
    let mut height = None;
    for trace in &data.traces {
        if hidden.contains(&trace.id.to_string()) {
            continue;
        }
        let style = style_for(spec, styles, trace.id);
        let values = match style.mode {
            crate::trace_color::Mode::Solid => None,
            crate::trace_color::Mode::Speed => Some(speed.get_or_insert_with(|| {
                let mut spec = spec.clone();
                spec.map.as_mut().unwrap().coloring = ColorBy::Speed;
                prepare(data, lines, &spec, hidden)
            })),
            crate::trace_color::Mode::Height => Some(height.get_or_insert_with(|| {
                let mut spec = spec.clone();
                spec.map.as_mut().unwrap().coloring = ColorBy::Altitude;
                prepare(data, lines, &spec, hidden)
            })),
        };
        if let Some(gradient) = style.gradient().filter(|gradient| gradient.valid()) {
            let label = format!(
                "{} · {}",
                lines
                    .iter()
                    .find(|line| line.id == trace.id)
                    .map_or("Trace", |line| line.name.as_str()),
                if style.mode == crate::trace_color::Mode::Speed {
                    "speed (m/s)"
                } else {
                    "height (m)"
                }
            );
            output.scales.insert(
                trace.id,
                Scale {
                    gradient,
                    label: label.clone(),
                },
            );
            if output.range.is_none() {
                output.range = Some(gradient.range);
                output.label = label;
            }
        }
        if let Some(values) = values.and_then(|values| values.samples.get(&trace.id)) {
            output.samples.insert(trace.id, values.clone());
        } else if !output.tints.is_empty() {
            output
                .samples
                .insert(trace.id, vec![f64::NAN; trace.track.times.len()].into());
        }
        if output.samples.contains_key(&trace.id) {
            output.times.insert(trace.id, trace.track.times.clone());
        }
    }
    output
}
pub fn install(workbench: Workbench, pane: Pane) -> Signal<Arc<Coloring>, LocalStorage> {
    let output = RwSignal::new_local(Arc::new(Coloring::default()));
    let generation = StoredValue::new_local(0_u64);
    let settings = Memo::new(move |_| {
        pane.trajectory.with(|s| {
            s.as_ref().map(|s| {
                (
                    s.map.unwrap_or_default().coloring,
                    s.color_source.clone(),
                    s.altitudes.clone(),
                    s.pairs(),
                )
            })
        })
    });
    let tints = Memo::new(move |_| crate::state_tint::bands(workbench, pane));
    let effect = zgui::reactive::RenderEffect::new(move |_| {
        let options = settings.get();
        let styles = pane.trace_colors.get();
        let tints = tints.get();
        generation.update_value(|value| *value = value.wrapping_add(1));
        let current = generation.get_value();
        if options
            .as_ref()
            .is_none_or(|s| matches!(s.0, ColorBy::Trace | ColorBy::Channel))
            && styles
                .values()
                .all(|style| style.mode == crate::trace_color::Mode::Solid)
            && tints.is_empty()
        {
            output.set(Arc::new(Coloring::default()));
            return;
        }
        let Some(data) = pane.trajectory_data.get() else {
            output.set(Arc::new(Coloring::default()));
            return;
        };
        let Some(spec) = pane.trajectory.get_untracked() else {
            return;
        };
        let lines = pane.lines.get();
        let hidden = pane.plot.hidden.get();
        workbench.task(async move {
            let result = plot::preparation::prepare("map-colors", move || {
                prepare_styled(&data, &lines, &spec, &hidden, &styles, tints)
            })
            .await;
            if generation.try_get_value() == Some(current) {
                output.set(Arc::new(result));
            }
        });
    });
    on_cleanup_local(move || drop(effect));
    Signal::derive_local(move || output.get())
}
#[cfg(test)]
#[path = "telemetry_tests.rs"]
pub(crate) mod tests;
