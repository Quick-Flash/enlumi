use crate::spectral::{self, Mode, Options, Window};
use plot::{PlotPoint, time_series::SeriesData};
use std::sync::atomic::AtomicBool;

fn signal(n: usize, bin: usize, amplitude: f64) -> SeriesData {
    SeriesData::new(
        (0..n)
            .map(|i| PlotPoint {
                x: i as f64 / 1000.0,
                y: amplitude * (std::f64::consts::TAU * bin as f64 * i as f64 / n as f64).sin(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn options(mode: Mode) -> Options {
    Options {
        mode,
        size: 2048,
        window: Window::Rectangular,
        overlap: 50,
        heatmap_bins: 40,
        start: None,
        end: None,
    }
}
#[test]
fn full_fft_uses_odd_field_length_and_scales_last_positive_bin() {
    let n = 255;
    let result = spectral::compute(
        &signal(n, 127, 2.5),
        None,
        &options(Mode::Fft),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.size, n);
    assert_eq!(result.windows, 1);
    assert_eq!(result.points.len(), 128);
    assert!((result.points[127].y - 2.5).abs() < 1e-9);
    assert!((result.points[127].x - 127000.0 / n as f64).abs() < 1e-8);
}
#[test]
fn full_psd_integrates_to_variance_without_padding_or_window_size_control() {
    let result = spectral::compute(
        &signal(1001, 23, 3.0),
        None,
        &options(Mode::Psd),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.size, 1001);
    let power = result.points.iter().map(|p| p.y).sum::<f64>() * result.rate / result.size as f64;
    assert!((power - 4.5).abs() < 1e-9);
}
#[test]
fn selected_time_range_sets_transform_length_and_maps_require_complete_windows() {
    let data = signal(1001, 20, 1.0);
    let mut opts = options(Mode::Fft);
    opts.start = Some(0.1);
    opts.end = Some(0.5);
    let result = spectral::compute(&data, None, &opts, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.size, 401);
    opts.mode = Mode::Spectrogram;
    assert!(spectral::compute(&data, None, &opts, &AtomicBool::new(false)).is_err());
    opts.size = 128;
    let map = spectral::compute(&data, None, &opts, &AtomicBool::new(false)).unwrap();
    assert!(map.heatmap.is_some());
    assert_eq!(map.size, 128);
    assert_eq!(spectral::available_size(&data, opts.start, opts.end), 401);
}
#[test]
fn full_transform_rejects_gaps_instead_of_silently_discarding_data() {
    let mut points = signal(513, 5, 1.0).samples().to_vec();
    points[256].y = f64::NAN;
    let data = SeriesData::new(points).unwrap();
    assert!(
        spectral::compute(&data, None, &options(Mode::Fft), &AtomicBool::new(false))
            .err()
            .unwrap()
            .contains("gaps")
    );
    assert_eq!(spectral::available_size(&data, None, None), 256);
}

#[test]
fn offered_map_sizes_fit_data_and_memory_limits() {
    let data = signal(512, 5, 1.0);
    let choices = spectral::window_sizes(&data, None, None, Mode::Spectrogram, 50, 40);
    assert_eq!(choices, vec![128, 256, 512]);
    for size in choices {
        let mut opts = options(Mode::Spectrogram);
        opts.size = size;
        assert!(spectral::compute(&data, None, &opts, &AtomicBool::new(false)).is_ok());
    }
    let data = signal(65536, 50, 1.0);
    assert!(!spectral::window_sizes(&data, None, None, Mode::Heatmap, 50, 320).contains(&65536));
    assert!(spectral::window_sizes(&data, None, None, Mode::Spectrogram, 50, 40).contains(&65536));
}
