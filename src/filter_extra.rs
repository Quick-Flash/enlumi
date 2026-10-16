use plot::{PlotPoint, time_series::SeriesData};
use std::{collections::VecDeque, ops::Range};

/// Never cross missing values, duplicate timestamps, or gaps exceeding 10 median dt.
pub(crate) fn segments(points: &[PlotPoint], gap: f64) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (i, point) in points.iter().enumerate() {
        let broken = i > 0 && (point.x - points[i - 1].x <= 0.0 || point.x - points[i - 1].x > gap);
        if (!point.y.is_finite() || broken)
            && let Some(begin) = start.take()
        {
            ranges.push(begin..i);
        }
        if point.y.is_finite() && start.is_none() {
            start = Some(i);
        }
    }
    if let Some(begin) = start {
        ranges.push(begin..points.len());
    }
    ranges
}
pub fn moving_average(source: &SeriesData, window: usize) -> Result<SeriesData, String> {
    if !(1..=1_000_000).contains(&window) {
        return Err("Window must contain 1-1000000 samples".into());
    }
    let points = source.samples();
    let mut output: Vec<_> = points
        .iter()
        .map(|point| PlotPoint {
            x: point.x,
            y: f64::NAN,
        })
        .collect();
    for range in segments(points, crate::filter::gap_threshold(points)) {
        let mut values = VecDeque::new();
        let mut sum = 0.0;
        for i in range {
            values.push_back(points[i].y);
            sum += points[i].y;
            if values.len() > window {
                sum -= values.pop_front().expect("full window");
            }
            output[i].y = sum / values.len() as f64;
        }
    }
    SeriesData::new(output)
}
pub fn derivative(source: &SeriesData) -> Result<SeriesData, String> {
    let points = source.samples();
    let mut output: Vec<_> = points
        .iter()
        .map(|point| PlotPoint {
            x: point.x,
            y: f64::NAN,
        })
        .collect();
    for range in segments(points, crate::filter::gap_threshold(points)) {
        if range.len() < 2 {
            continue;
        }
        let slope = |a: usize, b: usize| (points[b].y - points[a].y) / (points[b].x - points[a].x);
        output[range.start].y = slope(range.start, range.start + 1);
        output[range.end - 1].y = slope(range.end - 2, range.end - 1);
        for i in range.start + 1..range.end - 1 {
            let a = points[i].x - points[i - 1].x;
            let b = points[i + 1].x - points[i].x;
            output[i].y = (b * slope(i - 1, i) + a * slope(i, i + 1)) / (a + b);
        }
    }
    SeriesData::new(output)
}
pub fn integral(source: &SeriesData, initial: f64) -> Result<SeriesData, String> {
    if !initial.is_finite() {
        return Err("Initial integral value must be finite".into());
    }
    let points = source.samples();
    let mut output: Vec<_> = points
        .iter()
        .map(|point| PlotPoint {
            x: point.x,
            y: f64::NAN,
        })
        .collect();
    for range in segments(points, crate::filter::gap_threshold(points)) {
        let mut sum = initial;
        output[range.start].y = sum;
        for i in range.start + 1..range.end {
            sum += (points[i - 1].y * 0.5 + points[i].y * 0.5) * (points[i].x - points[i - 1].x);
            output[i].y = sum;
        }
    }
    SeriesData::new(output)
}
