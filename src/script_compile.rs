//! Compile local Rust source without Cargo, build scripts, or third-party crates.
use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    process::{Command, Stdio},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use wasmtime::{Config, Engine, Module};

pub const EXAMPLE: &str = include_str!("script_example.rs.txt");
type Cache = VecDeque<(String, Module)>;

pub fn engine() -> Result<&'static Engine, String> {
    static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config.consume_fuel(true).epoch_interruption(true);
            Engine::new(&config).map_err(|error| format!("WASM engine: {error}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn compile(source: &str, cancel: &AtomicBool) -> Result<Module, String> {
    check_cancel(cancel)?;
    if source.len() > 256 * 1024 || source.trim().is_empty() {
        return Err("Rust source must contain 1–262144 bytes".into());
    }
    crate::script_contract::output_count(source)?;
    static CACHE: Mutex<Cache> = Mutex::new(VecDeque::new());
    if let Some((_, module)) = CACHE
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .find(|(key, _)| key == source)
    {
        return Ok(module.clone());
    }
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = directory.path();
    std::fs::write(path.join("filter.rs"), source).map_err(|e| e.to_string())?;
    std::fs::write(path.join("wrapper.rs"), include_str!("script_guest.rs.txt"))
        .map_err(|e| e.to_string())?;
    let diagnostics = path.join("diagnostics.txt");
    let mut command = compiler()?;
    command
        .arg(path.join("wrapper.rs"))
        .args([
            "--crate-name",
            "enlumi_filter",
            "--crate-type",
            "cdylib",
            "--edition=2024",
            "--target",
            "wasm32-unknown-unknown",
            "-C",
            "opt-level=2",
            "-C",
            "panic=abort",
            "--color=never",
            "-o",
        ])
        .arg(path.join("filter.wasm"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(&diagnostics).map_err(|e| e.to_string())?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().map_err(|e| format!(
        "Cannot start Rust compiler: {e}. Use Enlumi portable bundle (including rust-toolchain), or set ENLUMI_RUSTC to a compiler with wasm32-unknown-unknown installed."
    ))?;
    let started = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            check_cancel(cancel)?;
            return Err("Rust compilation exceeded 60 seconds".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Rust compiler: {error}"));
            }
        }
    };
    check_cancel(cancel)?;
    if !status.success() {
        let mut message = Vec::new();
        File::open(diagnostics)
            .map_err(|e| e.to_string())?
            .take(65536)
            .read_to_end(&mut message)
            .map_err(|e| e.to_string())?;
        let message = String::from_utf8_lossy(&message);
        return Err(format!(
            "Rust compilation failed:\n{message}\nMissing target? Run `rustup target add wasm32-unknown-unknown` for compiler selected by ENLUMI_RUSTC or PATH."
        ));
    }
    let wasm = std::fs::read(path.join("filter.wasm")).map_err(|e| e.to_string())?;
    let module = Module::new(engine()?, wasm).map_err(|e| format!("WASM compilation: {e}"))?;
    check_cancel(cancel)?;
    let mut cache = CACHE.lock().map_err(|e| e.to_string())?;
    if cache.len() == 32 {
        cache.pop_front();
    }
    cache.push_back((source.into(), module.clone()));
    Ok(module)
}

pub fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Filter run cancelled".into())
    } else {
        Ok(())
    }
}

// Explicit override wins; portable releases never depend on rustup or PATH.
fn compiler() -> Result<Command, String> {
    if let Some(path) = std::env::var_os("ENLUMI_RUSTC") {
        return Ok(Command::new(path));
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = executable
        .parent()
        .ok_or("Cannot locate Enlumi directory")?
        .join("rust-toolchain");
    if root.exists() {
        let rustc = root
            .join("bin")
            .join(format!("rustc{}", std::env::consts::EXE_SUFFIX));
        if !rustc.is_file() || !root.join("lib/rustlib/wasm32-unknown-unknown/lib").is_dir() {
            return Err("Bundled Rust compiler is incomplete. Extract entire Enlumi portable bundle, including rust-toolchain.".into());
        }
        let mut command = Command::new(rustc);
        #[cfg(target_os = "macos")]
        command.env("DYLD_LIBRARY_PATH", root.join("lib"));
        command.arg("--sysroot").arg(root);
        Ok(command)
    } else {
        Ok(Command::new("rustc"))
    }
}
