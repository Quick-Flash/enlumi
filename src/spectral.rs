//! One-sided, mean-removed spectra over contiguous, uniformly resampled runs.
use plot::{PlotPoint, time_series::SeriesData};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fft,
    Psd,
    Heatmap,
    Spectrogram,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Window {
    Hann,
    Hamming,
    Blackman,
    Rectangular,
}

impl Mode {
    pub fn windowed(self) -> bool {
        matches!(self, Self::Heatmap | Self::Spectrogram)
    }
}

#[derive(Clone)]
pub struct Options {
    pub mode: Mode,
    pub size: usize,
    pub window: Window,
    pub overlap: usize,
    pub heatmap_bins: usize,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub remove_dc: bool,
}

#[derive(Clone)]
pub struct Heatmap {
    pub values: std::sync::Arc<[Option<f64>]>,
    pub columns: usize,
    pub rows: usize,
    pub x_low: f64,
    pub x_high: f64,
    pub y_low: f64,
    pub y_high: f64,
    pub x_label: String,
    pub y_label: String,
    pub min_db: f64,
    pub max_db: f64,
}
pub struct Spectrum {
    pub points: Vec<PlotPoint>,
    pub heatmap: Option<Heatmap>,
    pub rate: f64,
    pub size: usize,
    pub windows: usize,
    pub skipped: usize,
    pub excluded_samples: usize,
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Analysis cancelled".into())
    } else {
        Ok(())
    }
}

fn interval(points: &[PlotPoint]) -> Result<f64, String> {
    let mut deltas = points
        .windows(2)
        .filter_map(|pair| {
            let dt = pair[1].x - pair[0].x;
            (dt.is_finite() && dt > 0.).then_some(dt)
        })
        .collect::<Vec<_>>();
    if deltas.is_empty() {
        return Err("Need timestamped samples, not a constant parameter".into());
    }
    let middle = deltas.len() / 2;
    deltas.select_nth_unstable_by(middle, f64::total_cmp);
    let median = deltas[middle];
    // Average nearby clock intervals to reduce timestamp quantization error;
    // dropped-sample gaps never contribute to inferred cadence.
    let nearby = deltas
        .iter()
        .filter(|dt| (**dt / median - 1.).abs() <= 0.1)
        .copied()
        .collect::<Vec<_>>();
    Ok(nearby.iter().sum::<f64>() / nearby.len() as f64)
}

// Bridge brief dropped samples only; never extrapolate or bridge long log gaps.
fn at(points: &[PlotPoint], time: f64, max_gap: f64, cursor: &mut usize) -> Option<f64> {
    while *cursor + 1 < points.len() && points[*cursor + 1].x <= time {
        *cursor += 1;
    }
    let a = points.get(*cursor)?;
    let tolerance = max_gap * 1e-6 + time.abs().max(a.x.abs()) * f64::EPSILON * 8.;
    if (a.x - time).abs() <= tolerance {
        return a.y.is_finite().then_some(a.y);
    }
    let b = points.get(*cursor + 1)?;
    if (b.x - time).abs() <= tolerance {
        return b.y.is_finite().then_some(b.y);
    }
    let dt = b.x - a.x;
    if time < a.x || time > b.x || dt <= 0. || dt > max_gap || !a.y.is_finite() || !b.y.is_finite()
    {
        return None;
    }
    Some(a.y + (b.y - a.y) * ((time - a.x) / dt))
}

pub fn compute(
    data: &SeriesData,
    condition: Option<&SeriesData>,
    options: &Options,
    cancel: &AtomicBool,
) -> Result<Spectrum, String> {
    let windowed = options.mode.windowed();
    if windowed && (!options.size.is_power_of_two() || !(128..=65536).contains(&options.size)) {
        return Err("FFT size must be power of two from 128 to 65536".into());
    }
    if windowed && options.overlap >= 100 {
        return Err("Window overlap must be below 100%".into());
    }
    if options.mode == Mode::Heatmap && !(8..=512).contains(&options.heatmap_bins) {
        return Err("Heatmap bin count must be from 8 to 512".into());
    }
    if options.start.is_some_and(|v| !v.is_finite())
        || options.end.is_some_and(|v| !v.is_finite())
        || options.start.zip(options.end).is_some_and(|(a, b)| a >= b)
    {
        return Err("Time range needs finite start < end".into());
    }
    let all = data.samples();
    let first = all.partition_point(|p| options.start.is_some_and(|start| p.x < start));
    let last = all.partition_point(|p| options.end.is_none_or(|end| p.x <= end));
    let selected = all.get(first..last).ok_or("Invalid time range")?;
    let finite = selected
        .iter()
        .copied()
        .filter(|p| p.y.is_finite())
        .collect::<Vec<_>>();
    let points = finite.as_slice();
    if points.len() < 2 {
        return Err("Need at least two samples in selected time range".into());
    }
    let dt = interval(selected)?;
    let rate = 1. / dt;
    let mut start = points[0].x;
    let count = (((points[points.len() - 1].x - start) / dt) + 1e-6).floor() + 1.;
    if !rate.is_finite() || !(2.0..=4_000_000.).contains(&count) {
        return Err("Analysis needs 2–4,000,000 uniformly sampled points; narrow time range or reduce FFT size".into());
    }
    let mut count = count as usize;
    let mut uniform = Vec::with_capacity(count);
    let mut cursor = 0;
    for i in 0..count {
        if i % 8192 == 0 {
            cancelled(cancel)?;
        }
        uniform.push(at(points, start + i as f64 * dt, dt * 3.5, &mut cursor));
    }
    let mut excluded_samples = 0;
    if !windowed {
        let mut best = 0..0;
        let mut begin = 0;
        for (i, sample) in uniform.iter().chain(std::iter::once(&None)).enumerate() {
            if sample.is_none() {
                if i - begin > best.len() {
                    best = begin..i;
                }
                begin = i + 1;
            }
        }
        if best.len() < 2 {
            return Err("Need at least two samples in a valid run".into());
        }
        start += best.start as f64 * dt;
        uniform = uniform[best].to_vec();
        count = uniform.len();
        let end = start + (count - 1) as f64 * dt;
        let tolerance = dt * 1e-6;
        excluded_samples = selected
            .iter()
            .filter(|p| p.x < start - tolerance || p.x > end + tolerance)
            .count();
    }
    let n = if windowed { options.size } else { count };
    if n > count {
        return Err(format!("Need {n} samples in selected time range"));
    }
    let condition_points = condition.map(|data| {
        data.samples()
            .iter()
            .copied()
            .filter(|p| p.y.is_finite())
            .collect::<Vec<_>>()
    });
    let condition = if options.mode == Mode::Heatmap {
        let data = condition.ok_or("Choose conditioning field for heatmap")?;
        Some((
            condition_points.as_ref().unwrap().as_slice(),
            interval(data.samples())? * 3.5,
        ))
    } else {
        None
    };
    let window = (0..n)
        .map(|i| {
            let phase = std::f64::consts::TAU * i as f64 / n as f64;
            match options.window {
                Window::Hann => 0.5 - 0.5 * phase.cos(),
                Window::Hamming => 0.54 - 0.46 * phase.cos(),
                Window::Blackman => 0.42 - 0.5 * phase.cos() + 0.08 * (2. * phase).cos(),
                Window::Rectangular => 1.,
            }
        })
        .collect::<Vec<_>>();
    let sum_w = window.iter().sum::<f64>();
    let sum_w2 = window.iter().map(|v| v * v).sum::<f64>();
    let bins = n / 2 + 1;
    let mut sum = vec![0.; bins];
    let mut frames = Vec::new();
    let mut windows = 0;
    let mut skipped = 0;
    let fft = FftPlanner::<f64>::new().plan_fft_forward(n);
    let mut buffer = vec![Complex::new(0., 0.); n];
    let mut scratch = vec![Complex::new(0., 0.); fft.get_inplace_scratch_len()];
    let step = if windowed {
        (n * (100 - options.overlap) / 100).max(1)
    } else {
        n
    };
    for offset in (0..=count.saturating_sub(n)).step_by(step) {
        cancelled(cancel)?;
        let samples = &uniform[offset..offset + n];
        if samples.iter().any(Option::is_none) {
            skipped += 1;
            continue;
        }
        let mut coordinate = (options.mode == Mode::Spectrogram)
            .then_some(start + (offset as f64 + (n - 1) as f64 / 2.) * dt);
        if let Some((points, gap)) = condition {
            let mut cursor = points
                .partition_point(|p| p.x < start + offset as f64 * dt)
                .saturating_sub(1);
            let mut total = 0.;
            let mut complete = true;
            for i in 0..n {
                if let Some(value) = at(points, start + (offset + i) as f64 * dt, gap, &mut cursor)
                {
                    total += value;
                } else {
                    complete = false;
                    break;
                }
            }
            if !complete {
                skipped += 1;
                continue;
            }
            coordinate = Some(total / n as f64);
        }
        let mean = if options.remove_dc {
            samples.iter().map(|v| v.unwrap()).sum::<f64>() / n as f64
        } else {
            0.
        };
        for (i, value) in samples.iter().enumerate() {
            buffer[i] = Complex::new((value.unwrap() - mean) * window[i], 0.);
        }
        fft.process_with_scratch(&mut buffer, &mut scratch);
        let mut power = Vec::with_capacity(bins);
        for k in 0..bins {
            let factor = if k == 0 || (n.is_multiple_of(2) && k == n / 2) {
                1.
            } else {
                2.
            };
            let value = if options.mode == Mode::Fft {
                buffer[k].norm_sqr() * factor * factor / (sum_w * sum_w)
            } else {
                buffer[k].norm_sqr() * factor / (rate * sum_w2)
            };
            if !value.is_finite() {
                return Err("Signal magnitude exceeds spectral calculation range".into());
            }
            sum[k] += value;
            if !sum[k].is_finite() {
                return Err("Spectral average exceeds calculation range".into());
            }
            power.push(value);
        }
        if let Some(value) = coordinate {
            // Bound temporary spectra storage for long logs and large transforms.
            if frames.len().saturating_add(1).saturating_mul(bins) > 8_000_000 {
                return Err(
                    "Frequency map too large; narrow time range or increase FFT size".into(),
                );
            }
            frames.push((offset / step, value, power));
        }
        windows += 1;
    }
    if windows == 0 {
        return Err(
            "No complete windows: reduce FFT size or choose contiguous data/time range".into(),
        );
    }
    let points = sum
        .into_iter()
        .enumerate()
        .map(|(k, value)| PlotPoint {
            x: k as f64 / n as f64 * rate,
            y: if options.mode == Mode::Fft {
                (value / windows as f64).sqrt()
            } else {
                value / windows as f64
            },
        })
        .collect();
    let heatmap = if matches!(options.mode, Mode::Heatmap | Mode::Spectrogram) {
        let (columns, rows, x_low, x_high, y_low, y_high, x_label, y_label, values) = if options
            .mode
            == Mode::Heatmap
        {
            if options.heatmap_bins * bins > 8_000_000 {
                return Err("Heatmap too large; reduce bin count or increase FFT size".into());
            }
            let low = frames.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
            let high = frames.iter().map(|v| v.1).fold(f64::NEG_INFINITY, f64::max);
            if !low.is_finite() || !high.is_finite() || high <= low {
                return Err("Conditioning field needs varying values over FFT windows".into());
            }
            let rows = options.heatmap_bins;
            let mut totals = vec![0.; rows * bins];
            let mut counts = vec![0_usize; rows];
            for (_, coordinate, power) in frames {
                let row =
                    (((coordinate - low) / (high - low) * rows as f64) as usize).min(rows - 1);
                counts[row] += 1;
                for (k, value) in power.into_iter().enumerate() {
                    totals[row * bins + k] += value;
                }
            }
            let values: Vec<Option<f64>> = totals
                .into_iter()
                .enumerate()
                .map(|(i, value)| {
                    (counts[i / bins] > 0 && value > 0.)
                        .then(|| 10. * (value / counts[i / bins] as f64).log10())
                })
                .collect();
            (
                bins,
                rows,
                0.,
                rate / 2.,
                low,
                high,
                "Frequency (Hz)".into(),
                "Condition".into(),
                values,
            )
        } else {
            let columns = (count - n) / step + 1;
            if columns * bins > 8_000_000 {
                return Err("Spectrogram too large; narrow time range or increase FFT size".into());
            }
            let mut totals = vec![0.; columns * bins];
            let mut counts = vec![0_usize; columns];
            for (column, _, power) in frames {
                counts[column] += 1;
                for (row, value) in power.into_iter().enumerate() {
                    totals[row * columns + column] += value;
                }
            }
            let values: Vec<Option<f64>> = totals
                .into_iter()
                .enumerate()
                .map(|(i, value)| {
                    (counts[i % columns] > 0 && value > 0.)
                        .then(|| 10. * (value / counts[i % columns] as f64).log10())
                })
                .collect();
            let first_center = start + (n - 1) as f64 * dt / 2.;
            let hop = step as f64 * dt;
            let last_center = first_center + (columns.saturating_sub(1)) as f64 * hop;
            (
                columns,
                bins,
                first_center - hop / 2.,
                last_center + hop / 2.,
                0.,
                rate / 2.,
                "Time (s)".into(),
                "Frequency (Hz)".into(),
                values,
            )
        };
        let min_db = values
            .iter()
            .flatten()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let max_db = values
            .iter()
            .flatten()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if !min_db.is_finite() || !max_db.is_finite() {
            return Err("Heatmap has no positive spectral power".into());
        }
        Some(Heatmap {
            values: values.into(),
            columns,
            rows,
            x_low,
            x_high,
            y_low,
            y_high,
            x_label,
            y_label,
            min_db,
            max_db,
        })
    } else {
        None
    };
    Ok(Spectrum {
        points,
        heatmap,
        rate,
        size: n,
        windows,
        skipped,
        excluded_samples,
    })
}

/// Largest gap-free run available after applying time bounds and inferred cadence.
pub fn available_size(data: &SeriesData, start: Option<f64>, end: Option<f64>) -> usize {
    let all = data.samples();
    let first = all.partition_point(|p| start.is_some_and(|start| p.x < start));
    let last = all.partition_point(|p| end.is_none_or(|end| p.x <= end));
    let Some(points) = all.get(first..last) else {
        return 0;
    };
    let Ok(dt) = interval(points) else {
        return 0;
    };
    let mut run = 0;
    let mut longest = 0;
    let mut previous: Option<f64> = None;
    for point in points {
        if !point.y.is_finite() {
            continue;
        }
        if previous.is_none_or(|time| point.x - time > dt * 3.5) {
            run = 1;
        } else if previous.is_some_and(|time| point.x > time) {
            run += ((point.x - previous.unwrap()) / dt).round() as usize;
        }
        previous = Some(point.x);
        longest = longest.max(run);
    }
    longest
}

/// Choices that fit complete signal runs and the frequency-map memory budget.
pub fn window_sizes(
    data: &SeriesData,
    start: Option<f64>,
    end: Option<f64>,
    mode: Mode,
    overlap: usize,
    rows: usize,
) -> Vec<usize> {
    if overlap >= 100 {
        return Vec::new();
    }
    let all = data.samples();
    let first = all.partition_point(|p| start.is_some_and(|start| p.x < start));
    let last = all.partition_point(|p| end.is_none_or(|end| p.x <= end));
    let Some(points) = all.get(first..last) else {
        return Vec::new();
    };
    let Ok(dt) = interval(points) else {
        return Vec::new();
    };
    let count = (((points.last().unwrap().x - points[0].x) / dt) + 1e-6).floor() + 1.0;
    if !(2.0..=4_000_000.0).contains(&count) {
        return Vec::new();
    }
    let available = available_size(data, start, end);
    [
        128_usize, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
    ]
    .into_iter()
    .filter(|&n| {
        if n > available || n > count as usize {
            return false;
        }
        let bins = n / 2 + 1;
        let step = (n * (100 - overlap) / 100).max(1);
        let frames = (count as usize - n) / step + 1;
        frames.saturating_mul(bins) <= 8_000_000
            && (mode != Mode::Heatmap || rows.saturating_mul(bins) <= 8_000_000)
    })
    .collect()
}
