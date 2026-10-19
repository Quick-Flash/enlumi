//! Native portable distribution, including offline Rust filter compiler.
use std::{env, fs, path::Path, process::Command};

pub fn build() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    // Validate before compiling or copying the bundled toolchain.
    let commit = super::revision::resolve(root)?;
    let destination = root.join("target/enlumi-portable");
    if destination.exists() {
        return Err(format!(
            "Move or remove {} before bundling again",
            destination.display()
        ));
    }
    let compiler = env::var_os("ENLUMI_RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(&compiler)
        .args(["--print", "sysroot"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let sysroot_text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let sysroot = Path::new(sysroot_text.trim());
    if !sysroot
        .join("lib/rustlib/wasm32-unknown-unknown/lib")
        .is_dir()
    {
        return Err("Bundle compiler needs WASM target: rustup target add wasm32-unknown-unknown (or select complete compiler with ENLUMI_RUSTC)".into());
    }
    let version = Command::new(&compiler)
        .arg("-vV")
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !version.status.success() {
        return Err("Cannot determine compiler host".into());
    }
    let version_text = String::from_utf8_lossy(&version.stdout);
    let host = version_text
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("Compiler did not report host")?;
    super::cargo(&["build", "--locked", "--release", "-p", "enlumi"], &[])?;
    // Stage separately: failed copies never produce an apparently complete bundle.
    let staging = root.join("target/enlumi-portable-staging");
    if staging.exists() {
        return Err(format!(
            "Move or remove {} before bundling again",
            staging.display()
        ));
    }
    let result = (|| -> std::io::Result<()> {
        fs::create_dir_all(&staging)?;
        let toolchain = staging.join("rust-toolchain");
        fs::create_dir_all(toolchain.join("bin"))?;
        let rustc = format!("rustc{}", env::consts::EXE_SUFFIX);
        fs::copy(
            sysroot.join("bin").join(&rustc),
            toolchain.join("bin").join(rustc),
        )?;
        // Windows compiler dependencies live beside rustc; Unix dependencies live in lib.
        for entry in fs::read_dir(sysroot.join("bin"))? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                fs::copy(entry.path(), toolchain.join("bin").join(entry.file_name()))?;
            }
        }
        // Compiler shared libraries sit directly in lib. Host std, sources,
        // analysis tools and other installed targets are unnecessary for WASM.
        fs::create_dir_all(toolchain.join("lib"))?;
        for entry in fs::read_dir(sysroot.join("lib"))? {
            let entry = entry?;
            if !entry.path().is_dir() {
                fs::copy(entry.path(), toolchain.join("lib").join(entry.file_name()))?;
            }
        }
        copy_tree(
            &sysroot.join("lib/rustlib/wasm32-unknown-unknown/lib"),
            &toolchain.join("lib/rustlib/wasm32-unknown-unknown/lib"),
        )?;
        let linker = format!("lib/rustlib/{host}/bin/rust-lld{}", env::consts::EXE_SUFFIX);
        let linker_target = toolchain.join(&linker);
        fs::create_dir_all(linker_target.parent().unwrap())?;
        fs::copy(sysroot.join(linker), linker_target)?;
        let rust_docs = sysroot.join("share/doc/rust");
        for name in ["COPYRIGHT.html", "COPYRIGHT-library.html"] {
            fs::copy(rust_docs.join(name), toolchain.join(name))?;
        }
        copy_tree(&rust_docs.join("licenses"), &toolchain.join("licenses"))?;
        let app = format!("enlumi{}", env::consts::EXE_SUFFIX);
        fs::copy(root.join("target/release").join(&app), staging.join(app))?;
        copy_tree(&root.join("examples/profiles"), &staging.join("profiles"))?;
        copy_tree(&root.join("assets"), &staging.join("assets"))?;
        for name in ["LICENSE", "COPYRIGHT", "THIRD_PARTY_NOTICES.md"] {
            fs::copy(root.join(name), staging.join(name))?;
        }
        let platform = match (env::consts::OS, env::consts::ARCH) {
            ("linux", "x86_64")
                if env::var("ENLUMI_BUILD_PLATFORM").as_deref() == Ok("arch-linux-x86_64") =>
            {
                "arch-linux-x86_64"
            }
            ("linux", "x86_64") => "linux-x86_64",
            ("windows", "x86_64") => "windows-x86_64",
            ("macos", "aarch64") => "macos-aarch64",
            ("macos", "x86_64") => "macos-x86_64",
            _ => {
                return Err(std::io::Error::other(
                    "Unsupported portable update platform",
                ));
            }
        };
        fs::write(
            staging.join("enlumi-bundle.json"),
            format!("{{\"format\":1,\"platform\":\"{platform}\",\"commit\":\"{commit}\"}}\n"),
        )?;
        check_compiler(&toolchain, &staging)?;
        fs::rename(&staging, &destination)
    })();
    if let Err(error) = result {
        return Err(format!(
            "Bundle failed: {error}. Incomplete staging directory: {}",
            staging.display()
        ));
    }
    println!(
        "Portable bundle: {} (ship entire directory)",
        destination.display()
    );
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            // Dereference file symlinks so bundle never points into developer's sysroot.
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// Packaging check: actual filter ABI, empty PATH, no rustup/compiler environment.
fn check_compiler(toolchain: &Path, staging: &Path) -> std::io::Result<()> {
    let check = staging.join("compiler-check");
    fs::create_dir(&check)?;
    fs::write(
        check.join("filter.rs"),
        include_str!("../../src/script_example.rs.txt"),
    )?;
    fs::write(
        check.join("wrapper.rs"),
        include_str!("../../src/script_guest.rs.txt"),
    )?;
    let mut command = Command::new(
        toolchain
            .join("bin")
            .join(format!("rustc{}", env::consts::EXE_SUFFIX)),
    );
    command
        .current_dir(&check)
        .env("PATH", "")
        .env_remove("RUSTUP_HOME")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTC")
        .env_remove("RUSTFLAGS")
        .env_remove("LD_LIBRARY_PATH")
        .arg("--sysroot")
        .arg(toolchain)
        .args([
            "wrapper.rs",
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
            "-o",
            "filter.wasm",
        ]);
    // rust-lld on macOS also needs LLVM dylibs from bundled lib directory.
    #[cfg(target_os = "macos")]
    command.env("DYLD_LIBRARY_PATH", toolchain.join("lib"));
    let output = command.output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "Bundled compiler failed with empty PATH: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let wasm = fs::read(check.join("filter.wasm"))?;
    if !wasm.starts_with(b"\0asm") {
        return Err(std::io::Error::other(
            "Bundled compiler did not produce WebAssembly",
        ));
    }
    fs::remove_dir_all(check)
}
