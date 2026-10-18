//! A copied helper waits for parent exit before replacing Windows-locked files.
use crate::update_download::{self, MARKER, Plan, Prepared, executable};
use std::{
    fs,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

fn helper_name() -> String {
    format!("enlumi-update-helper{}", std::env::consts::EXE_SUFFIX)
}

fn command(executable: &Path) -> Command {
    let command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x08000000);
        command
    }
    #[cfg(not(windows))]
    command
}

pub fn start(prepared: Prepared, cancel: Arc<AtomicBool>) -> Result<ChildStdin, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Update cancelled".into());
    }
    let stage = prepared.directory.path();
    let helper = stage.join(helper_name());
    fs::copy(std::env::current_exe().map_err(|e| e.to_string())?, &helper)
        .map_err(|e| e.to_string())?;
    let log = fs::File::create(stage.join("helper.log")).map_err(|e| e.to_string())?;
    let mut child = command(&helper)
        .arg("--enlumi-apply-update")
        .arg(stage)
        .current_dir(stage)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("Cannot start update helper: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing helper handshake pipe")?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut ready = String::new();
        let result = std::io::BufReader::new(stdout)
            .read_line(&mut ready)
            .map(|_| ready);
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(ready)) if ready.trim() == "READY" => {}
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            let mut details = String::new();
            if let Ok(log) = fs::File::open(stage.join("helper.log")) {
                let _ = log.take(8192).read_to_string(&mut details);
            }
            return Err(format!(
                "Update helper failed to prepare; current installation unchanged. {}",
                details.trim()
            ));
        }
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Update cancelled".into());
    }
    let input = child.stdin.take().ok_or("Missing helper shutdown pipe")?;
    // Helper now owns staging. Keep pipe alive until process exits; EOF is handoff.
    let _ = prepared.directory.keep();
    Ok(input)
}

fn plan(stage: &Path) -> Result<Plan, String> {
    let bytes = fs::read(stage.join("plan.json")).map_err(|e| e.to_string())?;
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let parent = plan
        .destination
        .parent()
        .ok_or("Invalid installation path")?;
    if stage.parent() != Some(parent)
        || !stage
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".enlumi-update-"))
        || !stage.join(helper_name()).is_file()
        || !crate::update_release::hex(&plan.commit, 40)
        || !crate::update_release::hex(&plan.base_commit, 40)
    {
        return Err("Invalid update staging directory".into());
    }
    Ok(plan)
}

fn error_path() -> Option<PathBuf> {
    crate::preferences::config_directory().map(|p| p.join("update-error.txt"))
}

pub fn take_error() -> Option<String> {
    let path = error_path()?;
    let mut text = String::new();
    fs::File::open(&path)
        .ok()?
        .take(8192)
        .read_to_string(&mut text)
        .ok()?;
    let _ = fs::remove_file(path);
    Some(text)
}

fn record_error(error: &str) {
    if let Some(path) = error_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, error);
    }
}

fn wait_for_parent() -> Result<(), String> {
    let mut byte = [0_u8];
    match std::io::stdin().read(&mut byte) {
        Ok(0) => Ok(()),
        _ => Err("Invalid update handoff".into()),
    }
}

fn rename(source: &Path, destination: &Path) -> Result<(), String> {
    let start = Instant::now();
    loop {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(e)
                if cfg!(windows)
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::WouldBlock
                    )
                    && start.elapsed() < Duration::from_secs(75) =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!(
                    "Cannot replace {}: {e}. Close other Enlumi instances.",
                    source.display()
                ));
            }
        }
    }
}

#[derive(Default)]
struct Swap {
    moved: Vec<String>,
    installed: Vec<String>,
}

impl Swap {
    fn rollback(&self, stage: &Path, destination: &Path) -> Result<(), String> {
        let payload = stage.join("unpacked/enlumi-portable");
        let mut errors = Vec::new();
        for name in self.installed.iter().rev() {
            if let Err(e) = rename(&destination.join(name), &payload.join(name)) {
                errors.push(e);
            }
        }
        for name in self.moved.iter().rev() {
            if let Err(e) = rename(&stage.join("previous").join(name), &destination.join(name)) {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Rollback incomplete: {}. Backup: {}",
                errors.join("; "),
                stage.join("previous").display()
            ))
        }
    }

    fn apply(&mut self, stage: &Path, destination: &Path) -> Result<(), String> {
        let payload = stage.join("unpacked/enlumi-portable");
        fs::create_dir(stage.join("previous")).map_err(|e| e.to_string())?;
        // Only owned application files move. Profiles, logs and custom files stay.
        for name in [
            "rust-toolchain".to_owned(),
            "assets".to_owned(),
            "LICENSE".into(),
            "COPYRIGHT".into(),
            "THIRD_PARTY_NOTICES.md".into(),
            MARKER.into(),
            executable(),
        ] {
            if destination.join(&name).exists() {
                rename(
                    &destination.join(&name),
                    &stage.join("previous").join(&name),
                )?;
                self.moved.push(name.clone());
            }
            rename(&payload.join(&name), &destination.join(&name))?;
            self.installed.push(name);
        }
        Ok(())
    }
}

fn new_profiles(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Ok(());
    }
    if !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            new_profiles(&entry.path(), &target)?;
        } else {
            match fs::File::options()
                .write(true)
                .create_new(true)
                .open(target)
            {
                Ok(mut output) => {
                    std::io::copy(&mut fs::File::open(entry.path())?, &mut output)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

fn restart(destination: &Path, stage: Option<&Path>) -> Result<ChildStdin, String> {
    let mut command = command(&destination.join(executable()));
    command
        .current_dir(destination)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(stage) = stage {
        let _ = fs::remove_file(stage.join("restart-ready"));
        command.arg("--enlumi-finish-update").arg(stage);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot restart Enlumi: {e}"))?;
    let input = child.stdin.take().ok_or("Missing restart handoff pipe")?;
    if let Some(stage) = stage {
        let started = Instant::now();
        loop {
            if fs::read(stage.join("restart-ready")).is_ok_and(|bytes| bytes == b"ready") {
                break;
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!(
                    "Updated app exited before startup handshake: {status}"
                ));
            }
            if started.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Updated app did not acknowledge startup".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    // Child waits for EOF before removing helper/staging, including on Windows.
    Ok(input)
}

fn apply(stage: &Path) -> Result<(), String> {
    let plan = plan(stage)?;
    let payload = stage.join("unpacked/enlumi-portable");
    let manifest = update_download::bundle(&payload)?;
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(plan.destination.join(".enlumi-update.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|e| format!("Another update is in progress: {e}"))?;
    if update_download::bundle(&plan.destination)?.commit != plan.base_commit {
        return Err("Installation changed during download. Restart Enlumi before updating.".into());
    }
    if manifest.commit != plan.commit {
        return Err("Update revision mismatch".into());
    }
    println!("READY");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    wait_for_parent()?;
    let mut swap = Swap::default();
    let result = swap.apply(stage, &plan.destination).and_then(|()| {
        if let Err(e) = new_profiles(
            &payload.join("profiles"),
            &plan.destination.join("profiles"),
        ) {
            record_error(&format!(
                "App updated, but new example profiles could not be copied: {e}"
            ));
        }
        restart(&plan.destination, Some(stage))
    });
    if let Err(error) = result {
        let restored = swap.rollback(stage, &plan.destination);
        let message = match &restored {
            Ok(()) => format!("Update failed; previous version restored. {error}"),
            Err(rollback) => format!("Update failed: {error}. {rollback}"),
        };
        record_error(&message);
        if let Err(e) = restart(&plan.destination, restored.is_ok().then_some(stage)) {
            record_error(&format!(
                "{message}. {e}. Recovery files: {}",
                stage.display()
            ));
        }
    }
    Ok(())
}

pub fn startup() -> Result<bool, String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|a| a.to_str()) {
        Some("--enlumi-apply-update") if args.len() == 2 => {
            apply(
                &PathBuf::from(&args[1])
                    .canonicalize()
                    .map_err(|e| e.to_string())?,
            )?;
            Ok(true)
        }
        Some("--enlumi-finish-update") if args.len() == 2 => {
            let stage = PathBuf::from(&args[1])
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let plan = plan(&stage)?;
            if update_download::installation()? != plan.destination {
                return Err("Update cleanup destination mismatch".into());
            }
            fs::write(stage.join("restart-ready"), b"ready").map_err(|e| e.to_string())?;
            wait_for_parent()?;
            let started = Instant::now();
            loop {
                match fs::remove_dir_all(&stage) {
                    Ok(()) => break,
                    Err(e) if started.elapsed() > Duration::from_secs(5) => {
                        record_error(&format!(
                            "Update installed. Could not clean {}: {e}",
                            stage.display()
                        ));
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
