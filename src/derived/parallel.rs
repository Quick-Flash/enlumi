//! Independent profile branches share inputs; each Rust invocation owns all its outputs.
use super::*;
use rayon::prelude::*;
use std::sync::OnceLock;

pub(crate) fn pool() -> Result<&'static rayon::ThreadPool, String> {
    static POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
    POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        rayon::ThreadPoolBuilder::new()
            .num_threads(cores.saturating_sub(1).clamp(1, 4))
            .thread_name(|index| format!("profile-{index}"))
            .build()
            .map_err(|error| format!("Cannot start profile workers: {error}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Caller validates fields and supplies only branches whose parents finished.
/// Runs inside the bounded profile pool, never the global Rayon pool.
pub(crate) fn compute_independent(
    fields: Vec<DerivedField>,
    log: &LogStore,
    evaluation: &mut EvaluationCache,
    cancel: Arc<AtomicBool>,
) -> Vec<Result<DerivedField, String>> {
    let raw: BTreeSet<_> = fields
        .iter()
        .flat_map(|field| &field.sources)
        .filter(|source| id(source).is_none() && !evaluation.series.contains_key(*source))
        .cloned()
        .collect();
    let decoded: Vec<_> = raw
        .into_par_iter()
        .map(|source| {
            let result =
                crate::script_compile::check_cancel(&cancel).and_then(|()| log.prepare(&source));
            (source, result)
        })
        .collect();
    let mut failures = BTreeMap::new();
    for (source, result) in decoded {
        match result {
            Ok(data) => {
                evaluation.series.insert(source, data);
            }
            Err(error) => {
                failures.insert(source, error);
            }
        }
    }
    let count = fields.len();
    let mut jobs: Vec<Vec<(usize, DerivedField)>> = Vec::new();
    for (index, field) in fields.into_iter().enumerate() {
        let shared = jobs.iter_mut().find(|job| {
            let other = &job[0].1;
            matches!((&field.filter, &other.filter),
                (FilterSpec::Rust(script, _), FilterSpec::Rust(other, _)) if script == other)
                && field.sources == other.sources
        });
        if let Some(job) = shared {
            job.push((index, field));
        } else {
            jobs.push(vec![(index, field)]);
        }
    }
    let completed: Vec<_> =
        jobs.into_par_iter()
            .map(|job| {
                let first = &job[0].1;
                let series = first
                    .sources
                    .iter()
                    .filter_map(|source| {
                        evaluation
                            .series
                            .get(source)
                            .map(|data| (source.clone(), data.clone()))
                    })
                    .collect();
                let scripts = evaluation.scripts.iter().filter(|cached| {
                matches!(&first.filter, FilterSpec::Rust(script, _) if *script == cached.script)
                    && first.sources == cached.sources
            }).cloned().collect();
                let mut local = EvaluationCache { series, scripts };
                let mut results = Vec::new();
                let mut failure = first
                    .sources
                    .iter()
                    .find_map(|source| failures.get(source))
                    .cloned();
                for (index, mut field) in job {
                    let result =
                        crate::script_compile::check_cancel(&cancel).and_then(
                            |()| match &failure {
                                Some(error) => Err(error.clone()),
                                None => evaluate_field(&mut field, &mut local, cancel.clone()),
                            },
                        );
                    if let Err(error) = &result {
                        failure = Some(error.clone());
                    }
                    results.push((index, result.map(|()| field)));
                }
                (results, local.scripts)
            })
            .collect();
    let mut results = vec![Err("Filter did not run".into()); count];
    for (outputs, scripts) in completed {
        for cached in scripts {
            if !evaluation.scripts.iter().any(|existing| {
                existing.script == cached.script && existing.sources == cached.sources
            }) {
                evaluation.scripts.push(cached);
            }
        }
        for (index, result) in outputs {
            if let Ok(field) = &result {
                evaluation
                    .series
                    .insert(field.key(), field.data.clone().unwrap());
            }
            results[index] = result;
        }
    }
    results
}
