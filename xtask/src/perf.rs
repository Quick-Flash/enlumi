//! Summarize the optional completed-frame CSV emitted by Enlumi.
use std::path::Path;

pub fn summarize(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    println!("{}", report(&text)?);
    Ok(())
}

fn percentile(values: &[f64], fraction: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(sorted.len() as f64 * fraction).ceil().max(1.) as usize - 1]
}

fn report(text: &str) -> Result<String, String> {
    let mut lines = text.lines();
    if lines.next()
        != Some(
            "at_ms,presented,frame_ms,paint_submit_ms,restyled,draw_calls,uploaded_bytes,damage_px",
        )
    {
        return Err("Expected an ENLUMI_PERF_LOG CSV".into());
    }
    let mut times = Vec::new();
    let mut paint = Vec::new();
    let mut at = Vec::new();
    let mut presented = 0.;
    let mut uploads = 0.;
    let mut calls = 0.;
    let mut restyled = 0.;
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row = line
            .split(',')
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| format!("Invalid CSV row {}", index + 2))?;
        if row.len() != 8
            || row.iter().any(|value| !value.is_finite() || *value < 0.)
            || row[1] > 1.
        {
            return Err(format!("Invalid CSV row {}", index + 2));
        }
        at.push(row[0]);
        presented += row[1];
        times.push(row[2]);
        paint.push(row[3]);
        restyled += row[4];
        calls += row[5];
        uploads += row[6];
    }
    if times.is_empty() {
        return Err("No completed frames in log".into());
    }
    let count = times.len() as f64;
    Ok(format!(
        "{} completed frames, {:.0} presented, {:.2}s recorded\nFrame wall time: mean {:.2} ms, p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms\nPaint + renderer submission: mean {:.2} ms, p95 {:.2} ms\nRestyled elements/frame: {:.1}; draw calls/frame: {:.1}; uploaded: {:.2} MiB\nSubmission time includes driver waits; this is not GPU execution timing.",
        times.len(),
        presented,
        (at.last().copied().unwrap_or(0.) - at[0]).max(0.) / 1000.,
        times.iter().sum::<f64>() / count,
        percentile(&times, 0.5),
        percentile(&times, 0.95),
        percentile(&times, 1.),
        paint.iter().sum::<f64>() / count,
        percentile(&paint, 0.95),
        restyled / count,
        calls / count,
        uploads / 1048576.
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summarizes_frame_costs_and_rejects_invalid_or_empty_logs() {
        let header =
            "at_ms,presented,frame_ms,paint_submit_ms,restyled,draw_calls,uploaded_bytes,damage_px";
        let result = report(&format!(
            "{header}\n0,1,10,4,20,2,1048576,100\n1000,0,2,1,0,0,0,0\n"
        ))
        .unwrap();
        assert!(result.contains("2 completed frames, 1 presented"));
        assert!(result.contains("mean 6.00 ms, p50 2.00 ms, p95 10.00 ms"));
        assert!(result.contains("uploaded: 1.00 MiB"));
        assert!(report(header).is_err());
        assert!(report(&format!("{header}\n0,1,NaN,0,0,0,0,0")).is_err());
        assert!(report("other data").is_err());
    }
}
