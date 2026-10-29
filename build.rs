#[path = "build/icon.rs"]
mod icon;
#[path = "build/revision.rs"]
mod revision;
#[path = "build/tlog.rs"]
mod tlog;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    icon::build();
    tlog::build();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/revision.rs");
    println!("cargo:rerun-if-env-changed=ENLUMI_BUILD_PLATFORM");
    println!("cargo:rerun-if-env-changed=ENLUMI_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let platform = std::env::var("ENLUMI_BUILD_PLATFORM").unwrap_or_default();
    println!("cargo:rustc-env=ENLUMI_BUILD_PLATFORM={platform}");
    // Watch actual ref files, including worktrees and packed refs.
    for name in [
        Some("HEAD".to_owned()),
        git(&["symbolic-ref", "-q", "HEAD"]),
        Some("packed-refs".to_owned()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &name]) {
            println!("cargo:rerun-if-changed={}", PathBuf::from(path).display());
        }
    }
    let commit = revision::resolve(Path::new(".")).unwrap_or_default();
    println!("cargo:rustc-env=ENLUMI_BUILD_COMMIT={commit}");
}
