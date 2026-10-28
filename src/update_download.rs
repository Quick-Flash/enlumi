//! Download, verify and stage a complete portable bundle before touching installation.
use crate::update_release::{Nightly, agent, hex, platform};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const MARKER: &str = "enlumi-bundle.json";
pub const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;
const MAX_UNPACKED: u64 = 3 * 1024 * 1024 * 1024;

#[derive(Deserialize)]
pub struct Bundle {
    pub format: u32,
    pub platform: String,
    pub commit: String,
}

pub fn bundle(directory: &Path) -> Result<Bundle, String> {
    let bytes = fs::File::open(directory.join(MARKER))
        .and_then(|file| {
            let mut bytes = Vec::new();
            file.take(4097).read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .map_err(|e| format!("Use portable Enlumi bundle to install updates: {e}"))?;
    if bytes.len() > 4096 {
        return Err("Invalid bundle manifest".into());
    }
    let manifest: Bundle = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if manifest.format != 1 || manifest.platform != platform()? || !hex(&manifest.commit, 40) {
        return Err("Incompatible bundle manifest".into());
    }
    Ok(manifest)
}

pub fn executable() -> String {
    format!("enlumi{}", std::env::consts::EXE_SUFFIX)
}

pub fn installation() -> Result<PathBuf, String> {
    let running = std::env::current_exe().map_err(|e| e.to_string())?;
    if running
        .file_name()
        .is_none_or(|name| name != executable().as_str())
    {
        return Err("Keep portable executable's original name to install updates".into());
    }
    let directory = running
        .parent()
        .ok_or("Cannot locate Enlumi installation")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let manifest = bundle(&directory)?;
    if manifest.commit != crate::updates::BUILD_COMMIT {
        return Err("Installed bundle does not match running executable".into());
    }
    Ok(directory)
}

#[derive(Serialize, Deserialize)]
pub struct Plan {
    pub destination: PathBuf,
    pub base_commit: String,
    pub commit: String,
}

pub struct Prepared {
    pub directory: tempfile::TempDir,
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Update cancelled".into())
    } else {
        Ok(())
    }
}

pub fn prepare(release: Nightly, cancel: &AtomicBool) -> Result<Prepared, String> {
    let destination = installation()?;
    let parent = destination
        .parent()
        .ok_or("Cannot stage update beside installation")?;
    let directory = tempfile::Builder::new()
        .prefix(".enlumi-update-")
        .tempdir_in(parent)
        .map_err(|e| format!("Installation folder is not writable: {e}"))?;
    if release.size == 0 || release.size > MAX_DOWNLOAD {
        return Err("Nightly archive exceeds size limit".into());
    }
    let url = format!(
        "https://github.com/Quick-Flash/enlumi/releases/download/{}/{}",
        release.tag, release.asset
    );
    cancelled(cancel)?;
    let mut response = agent(Duration::from_secs(15 * 60))
        .get(&url)
        .header("User-Agent", concat!("Enlumi/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("Update download failed: {e}"))?;
    let archive_path = directory.path().join("archive");
    let mut archive = fs::File::create(&archive_path).map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().as_reader();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    let mut received = 0;
    loop {
        cancelled(cancel)?;
        let length = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if length == 0 {
            break;
        }
        received += length as u64;
        if received > release.size || received > MAX_DOWNLOAD {
            return Err("Unexpected update download size".into());
        }
        hash.update(&buffer[..length]);
        archive
            .write_all(&buffer[..length])
            .map_err(|e| e.to_string())?;
    }
    if received != release.size || format!("{:x}", hash.finalize()) != release.digest {
        return Err("Update archive failed SHA-256 verification".into());
    }
    drop(archive);
    let unpacked = directory.path().join("unpacked");
    fs::create_dir(&unpacked).map_err(|e| e.to_string())?;
    extract(&archive_path, &unpacked, cancel)?;
    let payload = unpacked.join("enlumi-portable");
    let manifest = bundle(&payload)?;
    if manifest.commit != release.commit
        || ["LICENSE", "COPYRIGHT", "THIRD_PARTY_NOTICES.md"]
            .iter()
            .any(|name| !payload.join(name).is_file())
        || !payload.join("assets/enlumi.png").is_file()
        || !payload.join(executable()).is_file()
        || !payload
            .join("rust-toolchain/lib/rustlib/wasm32-unknown-unknown/lib")
            .is_dir()
        || !payload
            .join("rust-toolchain/bin")
            .join(format!("rustc{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    {
        return Err("Downloaded nightly bundle is incomplete or has wrong revision".into());
    }
    let plan = Plan {
        destination,
        base_commit: crate::updates::BUILD_COMMIT.to_owned(),
        commit: release.commit,
    };
    fs::write(
        directory.path().join("plan.json"),
        serde_json::to_vec(&plan).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    cancelled(cancel)?;
    Ok(Prepared { directory })
}

fn safe_path(path: &Path) -> bool {
    // Check names, not the rendered path: Windows inserts backslashes between
    // perfectly valid components when ZIP paths are converted to PathBuf.
    path.components().all(|component| match component {
        Component::Normal(name) => !name.to_string_lossy().contains([':', '\\', '\0']),
        _ => false,
    }) && path
        .components()
        .next()
        .is_some_and(|p| p.as_os_str() == "enlumi-portable")
}

#[cfg(not(windows))]
fn extract(archive: &Path, destination: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let gzip = flate2::read::GzDecoder::new(fs::File::open(archive).map_err(|e| e.to_string())?);
    let mut archive = tar::Archive::new(gzip);
    let mut total = 0_u64;
    for (count, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        cancelled(cancel)?;
        let mut entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.header().entry_type();
        total = total
            .checked_add(entry.size())
            .ok_or("Update archive size overflow")?;
        if count > 100_000
            || total > MAX_UNPACKED
            || !safe_path(&entry.path().map_err(|e| e.to_string())?)
            || !(kind.is_file() || kind.is_dir())
        {
            return Err("Invalid paths, links, or size in update archive".into());
        }
        if !entry.unpack_in(destination).map_err(|e| e.to_string())? {
            return Err("Update archive path escapes staging directory".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn extract(archive: &Path, destination: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut total = 0_u64;
    for index in 0..archive.len() {
        cancelled(cancel)?;
        let mut file = archive.by_index(index).map_err(|e| e.to_string())?;
        let path = file.enclosed_name().ok_or("Invalid ZIP entry path")?;
        total = total
            .checked_add(file.size())
            .ok_or("Update ZIP size overflow")?;
        if index > 100_000
            || total > MAX_UNPACKED
            // enclosed_name normalizes away roots and parent components. Validate
            // the original name too so those entries are rejected, not rewritten.
            || !safe_path(Path::new(file.name()))
            || !safe_path(&path)
            || file
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Invalid paths, links, or size in update ZIP".into());
        }
        let target = destination.join(path);
        if file.is_dir() {
            fs::create_dir_all(target).map_err(|e| e.to_string())?;
        } else {
            fs::create_dir_all(target.parent().ok_or("Invalid ZIP path")?)
                .map_err(|e| e.to_string())?;
            let mut output = fs::File::options()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| e.to_string())?;
            let expected = file.size();
            let copied = std::io::copy(&mut file.by_ref().take(expected + 1), &mut output)
                .map_err(|e| e.to_string())?;
            if copied != expected {
                return Err("Invalid ZIP entry size".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_paths_accept_native_separators_and_reject_escapes() {
        assert!(safe_path(
            &Path::new("enlumi-portable")
                .join("assets")
                .join("enlumi.png")
        ));
        assert!(safe_path(Path::new(
            "enlumi-portable/rust-toolchain/bin/rustc.exe"
        )));
        for name in [
            "../enlumi-portable/enlumi.exe",
            "enlumi-portable/../outside.exe",
            "/enlumi-portable/enlumi.exe",
            "C:/enlumi-portable/enlumi.exe",
            "other/enlumi.exe",
            "enlumi-portable/enlumi.exe:stream",
            "enlumi-portable/bad\0name",
        ] {
            assert!(!safe_path(Path::new(name)), "Accepted {name:?}");
        }
    }

    #[cfg(windows)]
    mod windows_zip {
        use super::*;
        use zip::{ZipWriter, write::SimpleFileOptions};

        fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
            let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
            for (name, bytes) in entries {
                zip.start_file(*name, SimpleFileOptions::default()).unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }

        #[test]
        fn extracts_nested_portable_zip_files_on_windows() {
            let directory = tempfile::tempdir().unwrap();
            let archive = directory.path().join("update.zip");
            let destination = directory.path().join("unpacked");
            fs::create_dir(&destination).unwrap();
            let entries: &[(&str, &[u8])] = &[
                ("enlumi-portable/enlumi.exe", b"app"),
                ("enlumi-portable/assets/enlumi.png", b"icon"),
                ("enlumi-portable/rust-toolchain/bin/rustc.exe", b"compiler"),
                (
                    "enlumi-portable/rust-toolchain/lib/rustlib/wasm32-unknown-unknown/lib/libstd.rlib",
                    b"std",
                ),
            ];
            write_zip(&archive, entries);
            extract(&archive, &destination, &AtomicBool::new(false)).unwrap();
            for (name, bytes) in entries {
                assert_eq!(fs::read(destination.join(name)).unwrap(), *bytes);
            }
        }

        #[test]
        fn zip_extraction_rejects_unsafe_original_paths() {
            for name in [
                "enlumi-portable/../outside.exe",
                "enlumi-portable/../../outside.exe",
                "/enlumi-portable/enlumi.exe",
                "C:/enlumi-portable/enlumi.exe",
                "enlumi-portable/enlumi.exe:stream",
                "other/enlumi.exe",
                r"enlumi-portable\..\outside.exe",
            ] {
                let directory = tempfile::tempdir().unwrap();
                let archive = directory.path().join("update.zip");
                let destination = directory.path().join("unpacked");
                fs::create_dir(&destination).unwrap();
                write_zip(&archive, &[(name, b"bad")]);
                assert!(
                    extract(&archive, &destination, &AtomicBool::new(false)).is_err(),
                    "Accepted {name:?}"
                );
                assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
                assert!(!directory.path().join("outside.exe").exists());
            }
        }

        #[test]
        fn zip_extraction_still_rejects_symlinks() {
            let directory = tempfile::tempdir().unwrap();
            let archive = directory.path().join("update.zip");
            let destination = directory.path().join("unpacked");
            fs::create_dir(&destination).unwrap();
            let mut zip = ZipWriter::new(fs::File::create(&archive).unwrap());
            zip.add_symlink(
                "enlumi-portable/link",
                "../outside",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zip.finish().unwrap();
            assert!(extract(&archive, &destination, &AtomicBool::new(false)).is_err());
            assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
        }
    }
}
