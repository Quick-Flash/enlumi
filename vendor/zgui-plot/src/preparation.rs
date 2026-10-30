//! Shared bounded background executor and opt-in geometry-preparation timing.
//!
//! zgui's default executor has at most four workers. Callers keep one active
//! job and replace pending snapshots, retaining displayed geometry until a
//! compatible result is ready. This helper does not mutate UI state on workers.

/// Prepare geometry off the UI thread, recording queue and computation time
/// separately when `ZGUI_LATENCY` tracing is enabled.
///
/// `kind` identifies the preparation stage and must not contain commas.
pub async fn prepare<T: Send + 'static>(
    kind: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let queued = zgui_profile::latency::marker();
    zgui::prelude::blocking(move || {
        let started = queued.map(|_| std::time::Instant::now());
        let result = work();
        if let Some((queued, started)) = queued.zip(started) {
            let elapsed = started.elapsed();
            zgui_profile::latency::note_with("plot.prepared", || {
                format!(
                    "{kind},{},{}",
                    started.duration_since(queued).as_nanos(),
                    elapsed.as_nanos()
                )
            });
        }
        result
    })
    .await
}
