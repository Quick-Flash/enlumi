use crate::{filter_multi::Aligned, script_compile};
use plot::{PlotPoint, time_series::SeriesData};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use wasmtime::{Instance, Store, StoreLimitsBuilder, UpdateDeadline};

const CHUNK: usize = 4096;
const FUEL: u64 = 100_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub source: String,
    pub outputs: usize,
}
impl Script {
    pub fn validate(&self, inputs: usize) -> Result<(), String> {
        if !(1..=64).contains(&inputs) || !(1..=16).contains(&self.outputs) {
            return Err("Rust filters need 1–64 inputs and 1–16 outputs".into());
        }
        if self.source.trim().is_empty() || self.source.len() > 256 * 1024 {
            return Err("Rust source must contain 1–262144 bytes".into());
        }
        let declared = crate::script_contract::output_count(&self.source)?;
        if declared != self.outputs {
            return Err(format!(
                "Code declares {declared} outputs, but editor has {} named rows. Update OUTPUTS or output rows before compiling.",
                self.outputs
            ));
        }
        Ok(())
    }
    pub fn apply(
        &self,
        inputs: &[&SeriesData],
        cancel: Arc<AtomicBool>,
    ) -> Result<Vec<SeriesData>, String> {
        self.validate(inputs.len())?;
        let module = script_compile::compile(&self.source, &cancel)?;
        let engine = script_compile::engine()?;
        // No WASI or host imports: guest only receives numeric buffers.
        let mut store = Store::new(
            engine,
            StoreLimitsBuilder::new()
                .memory_size(128 * 1024 * 1024)
                .instances(1)
                .memories(1)
                .tables(1)
                .build(),
        );
        store.limiter(|limits| limits);
        store.set_fuel(FUEL).map_err(|e| e.to_string())?;
        store.set_epoch_deadline(1);
        let started = Instant::now();
        let cancelled = cancel.clone();
        store.epoch_deadline_callback(move |_| {
            script_compile::check_cancel(&cancelled).map_err(wasmtime::Error::msg)?;
            if started.elapsed() > Duration::from_secs(30) {
                return Err(wasmtime::Error::msg("Filter execution exceeded 30 seconds"));
            }
            Ok(UpdateDeadline::Continue(1))
        });
        let done = Arc::new(AtomicBool::new(false));
        let _ticker = EpochTicker::new(engine.clone(), done)?;
        let result = (|| -> wasmtime::Result<Vec<SeriesData>> {
            let instance = Instance::new(&mut store, &module, &[])?;
            let init = instance.get_typed_func::<(i32, i32), i32>(&mut store, "init")?;
            let pointer = instance.get_typed_func::<i32, i32>(&mut store, "buffer")?;
            let process = instance.get_typed_func::<(i32, i32), ()>(&mut store, "process")?;
            let memory = instance
                .get_memory(&mut store, "memory")
                .ok_or_else(|| wasmtime::Error::msg("Missing WASM memory"))?;
            let context = init.call(&mut store, (inputs.len() as i32, self.outputs as i32))?;
            let base = pointer.call(&mut store, context)? as u32 as usize;
            let reference = inputs[0].samples();
            let mut aligned: Vec<_> = inputs
                .iter()
                .map(|input| Aligned::new(input, reference))
                .collect();
            let mut outputs = vec![Vec::with_capacity(reference.len()); self.outputs];
            let mut bytes = vec![0; CHUNK * 8];
            for start in (0..reference.len()).step_by(CHUNK) {
                script_compile::check_cancel(&cancel).map_err(wasmtime::Error::msg)?;
                let points = &reference[start..reference.len().min(start + CHUNK)];
                for (i, point) in points.iter().enumerate() {
                    bytes[i * 8..i * 8 + 8].copy_from_slice(&point.x.to_le_bytes());
                }
                memory.write(&mut store, base, &bytes[..points.len() * 8])?;
                for (column, input) in aligned.iter_mut().enumerate() {
                    for (i, point) in points.iter().enumerate() {
                        bytes[i * 8..i * 8 + 8]
                            .copy_from_slice(&input.at(start + i, point.x).to_le_bytes());
                    }
                    memory.write(
                        &mut store,
                        base + (column + 1) * CHUNK * 8,
                        &bytes[..points.len() * 8],
                    )?;
                }
                store.set_fuel(FUEL)?;
                process.call(&mut store, (context, points.len() as i32))?;
                // Re-read through Memory API after guest call: memory may have grown.
                for (column, output) in outputs.iter_mut().enumerate() {
                    memory.read(
                        &store,
                        base + (1 + inputs.len() + column) * CHUNK * 8,
                        &mut bytes[..points.len() * 8],
                    )?;
                    for (point, value) in points.iter().zip(bytes.chunks_exact(8)) {
                        let y = f64::from_le_bytes(value.try_into().expect("eight bytes"));
                        output.push(PlotPoint {
                            x: point.x,
                            y: if y.is_finite() { y } else { f64::NAN },
                        });
                    }
                }
            }
            outputs
                .into_iter()
                .map(|points| SeriesData::new(points).map_err(wasmtime::Error::msg))
                .collect()
        })();
        script_compile::check_cancel(&cancel)?;
        result.map_err(|error| format!("Rust filter failed: {error:#}"))
    }
}

struct EpochTicker {
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl EpochTicker {
    fn new(engine: wasmtime::Engine, done: Arc<AtomicBool>) -> Result<Self, String> {
        let stopped = done.clone();
        let thread = std::thread::Builder::new()
            .name("filter-interrupt".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(10));
                    engine.increment_epoch();
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            done,
            thread: Some(thread),
        })
    }
}
impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
