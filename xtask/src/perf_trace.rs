//! CPU wall-time boundaries from the pinned framework's opt-in latency trace.
use std::{collections::BTreeMap, fmt::Write, path::Path};

// Labels deliberately describe the actual boundary, including mixed work.
const STAGES: &[(&str, &str, &str)] = &[
    ("Frame", "f.begin", "f.end"),
    ("Input dispatch", "f.drain", "f.timers"),
    ("Reactive flush", "f.flush", "f.commands"),
    ("Restyle", "f.restyle", "f.restyled"),
    ("Layout + fragments", "f.layout", "f.image_demand"),
    ("Paint preparation", "f.paint", "p.emit"),
    ("Scene traversal/emission", "p.emit", "p.finish"),
    ("Scene finalization", "p.finish", "p.upload"),
    ("Atlas uploads", "p.upload", "p.budget"),
    ("Renderer table preparation", "r.tables", "r.vectors"),
    ("Vector rasterization", "r.vectors", "r.plan"),
    ("Draw planning", "r.plan", "r.encoder"),
    ("Encoder setup + scroll copy", "r.encoder", "r.buffers"),
    ("Buffer uploads", "r.buffers", "r.record"),
    ("Command recording", "r.record", "r.reconfigure"),
    ("Surface reconfiguration", "r.reconfigure", "acq.in"),
    ("Surface acquisition wait", "acq.in", "acq.out"),
    ("Final copy + queue submission", "acq.out", "sub.out"),
    ("  Final copy recording", "acq.out", "r.finish"),
    ("  Command buffer finishing", "r.finish", "r.submit"),
    ("  Queue submission", "r.submit", "r.retire"),
    ("  Retirement tracking", "r.retire", "r.recall"),
    ("  Upload reclamation", "r.recall", "sub.out"),
    ("Presentation", "sub.out", "pres.out"),
];

pub fn summarize(path: &Path) -> Result<(), String> {
    let input = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    println!("{}", report(&input)?);
    Ok(())
}

fn report(input: &str) -> Result<String, String> {
    let mut frame = None::<BTreeMap<String, u64>>;
    let mut samples = BTreeMap::<String, Vec<Vec<f64>>>::new();
    let mut preparations = BTreeMap::<(String, String), (Vec<f64>, Vec<f64>)>::new();
    let mut case = "all frames".to_owned();
    let mut measuring = true;
    let mut epoch = None;
    let mut completed = 0;
    for (index, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("Invalid trace row {}: {error}", index + 1))?;
        let invalid = || format!("Missing/invalid trace fields at row {}", index + 1);
        let at = value["t"].as_u64().ok_or_else(invalid)?;
        let base = value["base_unix_ns"].as_u64().ok_or_else(invalid)?;
        let stage = value["stage"].as_str().ok_or_else(invalid)?;
        if epoch != Some(base) {
            frame = None;
            epoch = Some(base);
            case = "all frames".to_owned();
            measuring = true;
        }
        // Workers stamp their marks before acquiring the trace sink lock, so
        // independent job completions may arrive slightly out of time order.
        // Frame-stage pairs below still validate their own ordering.
        if stage == "plot.prepared" {
            if !measuring {
                continue;
            }
            let note = value["note"].as_str().ok_or_else(invalid)?;
            let parts = note.split(',').collect::<Vec<_>>();
            if parts.len() != 3 || parts[0].is_empty() {
                return Err(invalid());
            }
            let queue = parts[1].parse::<u64>().map_err(|_| invalid())?;
            let work = parts[2].parse::<u64>().map_err(|_| invalid())?;
            let entry = preparations
                .entry((case.clone(), parts[0].to_owned()))
                .or_default();
            entry.0.push(queue as f64 / 1_000_000.);
            entry.1.push(work as f64 / 1_000_000.);
            continue;
        }
        if stage == "replay.begin" {
            case = value["note"].as_str().ok_or_else(invalid)?.to_owned();
            measuring = true;
            frame = None;
        } else if stage == "replay.end" {
            measuring = false;
            frame = None;
        } else if stage == "f.begin" {
            frame = measuring.then(|| BTreeMap::from([(stage.to_owned(), at)]));
        } else if let Some(marks) = frame.as_mut() {
            marks.entry(stage.to_owned()).or_insert(at);
            if stage == "f.end" {
                let rows = samples
                    .entry(case.clone())
                    .or_insert_with(|| vec![Vec::new(); STAGES.len()]);
                for (row, (_, start, end)) in rows.iter_mut().zip(STAGES) {
                    if let Some((start, end)) = marks.get(*start).zip(marks.get(*end)) {
                        if end < start {
                            return Err("Stage boundaries are out of order".into());
                        }
                        row.push((end - start) as f64 / 1_000_000.);
                    }
                }
                completed += 1;
                frame = None;
            }
        }
    }
    if completed == 0 && preparations.is_empty() {
        return Err("No complete frames in trace".into());
    }
    let mut output = String::new();
    for ((case, kind), (queue, work)) in preparations {
        writeln!(
            output,
            "{case}: {kind} geometry preparation (completed jobs; ms, mean / p95 / max; samples)"
        )
        .unwrap();
        for (label, mut values) in [("Queue delay", queue), ("Computation", work)] {
            values.sort_by(f64::total_cmp);
            writeln!(
                output,
                "  {label}: {:.3} / {:.3} / {:.3}; {}",
                values.iter().sum::<f64>() / values.len() as f64,
                values[(values.len() * 95).div_ceil(100) - 1],
                values[values.len() - 1],
                values.len()
            )
            .unwrap();
        }
    }
    for (case, rows) in samples {
        writeln!(
            output,
            "{case}: CPU wall time (ms), mean / p95 / max; samples"
        )
        .unwrap();
        for ((label, _, _), mut values) in STAGES.iter().zip(rows) {
            if values.is_empty() {
                continue;
            }
            values.sort_by(f64::total_cmp);
            writeln!(
                output,
                "  {label}: {:.3} / {:.3} / {:.3}; {}",
                values.iter().sum::<f64>() / values.len() as f64,
                values[(values.len() * 95).div_ceil(100) - 1],
                values[values.len() - 1],
                values.len()
            )
            .unwrap();
        }
    }
    output.push_str("Stages can include driver waits; these are not GPU execution times.\nMissing stages are omitted, not recorded as zero. Use a single-window trace.");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn trace(marks: &[(&str, u64)]) -> String {
        marks
            .iter()
            .map(|(stage, t)| {
                serde_json::json!({"stage":stage,"t":t,"base_unix_ns":1,"note":"drag"}).to_string()
                    + "\n"
            })
            .collect()
    }
    #[test]
    fn separates_stages_and_never_pairs_incomplete_frames() {
        let input = trace(&[
            ("f.begin", 0),
            ("r.buffers", 1_000_000),
            ("f.begin", 2_000_000),
            ("r.record", 9_000_000),
            ("acq.in", 10_000_000),
            ("acq.out", 13_000_000),
            ("f.end", 14_000_000),
        ]);
        let result = report(&input).unwrap();
        assert!(result.contains("Frame: 12.000 / 12.000 / 12.000; 1"));
        assert!(result.contains("Surface acquisition wait: 3.000 / 3.000 / 3.000; 1"));
        assert!(!result.contains("Buffer uploads:"));
    }
    #[test]
    fn excludes_warmups_between_replay_cases_and_rejects_bad_data() {
        let input = trace(&[
            ("replay.begin", 0),
            ("f.begin", 1),
            ("f.end", 2),
            ("replay.end", 3),
            ("f.begin", 4),
            ("f.end", 5),
        ]);
        assert!(report(&input).unwrap().contains("; 1"));
        assert!(report(&trace(&[("f.begin", 2), ("f.end", 1)])).is_err());
        assert!(report(&trace(&[("f.begin", 0)])).is_err());
        assert!(report("{}").is_err());
    }

    #[test]
    fn splits_submission_instead_of_attributing_the_whole_block_to_queue_waits() {
        let input = trace(&[
            ("f.begin", 0),
            ("acq.out", 1_000_000),
            ("r.finish", 2_000_000),
            ("r.submit", 5_000_000),
            ("r.retire", 7_000_000),
            ("r.recall", 7_500_000),
            ("sub.out", 8_000_000),
            ("f.end", 9_000_000),
        ]);
        let result = report(&input).unwrap();
        assert!(result.contains("Final copy + queue submission: 7.000 / 7.000 / 7.000; 1"));
        assert!(result.contains("Command buffer finishing: 3.000 / 3.000 / 3.000; 1"));
        assert!(result.contains("Queue submission: 2.000 / 2.000 / 2.000; 1"));
        assert!(result.contains("Upload reclamation: 0.500 / 0.500 / 0.500; 1"));
    }

    #[test]
    fn worker_timings_are_per_job_and_may_interleave_with_frame_marks() {
        let mut input = trace(&[("f.begin", 10), ("r.record", 20)]);
        input.push_str(
            &serde_json::json!({"stage":"plot.prepared","t":15,"base_unix_ns":1,
            "note":"time-series,1000000,3000000"})
            .to_string(),
        );
        input.push('\n');
        input.push_str(&trace(&[("f.end", 30)]));
        let result = report(&input).unwrap();
        assert!(result.contains("time-series geometry preparation (completed jobs"));
        assert!(result.contains("Queue delay: 1.000 / 1.000 / 1.000; 1"));
        assert!(result.contains("Computation: 3.000 / 3.000 / 3.000; 1"));
    }
}
