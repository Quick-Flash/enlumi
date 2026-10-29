//! Explicit maintenance command; ordinary builds never fetch MAVLink definitions.
use flate2::read::GzDecoder;
use quick_xml::{Reader, events::Event};
use semver::Version;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path, time::Duration};

const INDEX: &str = "https://index.crates.io/ma/vl/mavlink";
const MAX_DOWNLOAD: u64 = 32 * 1024 * 1024;

fn download(url: &str) -> Result<Vec<u8>, String> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err(format!(
            "Cannot load system TLS certificates: {:?}",
            native.errors
        ));
    }
    let roots = native
        .certs
        .iter()
        .map(|cert| ureq::tls::Certificate::from_der(cert.as_ref()).to_owned())
        .collect::<Vec<_>>();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::new_with_certs(&roots))
                .build(),
        )
        .timeout_global(Some(Duration::from_secs(60)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut error = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(attempt));
        }
        match agent.get(url).call().and_then(|mut response| {
            response
                .body_mut()
                .with_config()
                .limit(MAX_DOWNLOAD)
                .read_to_vec()
        }) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => error = err.to_string(),
        }
    }
    Err(format!("Cannot download {url}: {error}"))
}

fn release(index: &[u8], requested: Option<&str>) -> Result<(Version, String), String> {
    let requested = requested
        .map(Version::parse)
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut selected = None;
    for line in index
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let entry: serde_json::Value = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        let version = Version::parse(entry["vers"].as_str().ok_or("Missing crate version")?)
            .map_err(|e| e.to_string())?;
        if entry["yanked"].as_bool() != Some(false)
            || requested.as_ref().is_some_and(|wanted| wanted != &version)
            || (requested.is_none() && !version.pre.is_empty())
        {
            continue;
        }
        let checksum = entry["cksum"].as_str().ok_or("Missing crate checksum")?;
        if checksum.len() != 64 || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid crate checksum".into());
        }
        if selected.as_ref().is_none_or(|(old, _)| &version > old) {
            selected = Some((version, checksum.to_ascii_lowercase()));
        }
    }
    selected.ok_or_else(|| "No matching non-yanked MAVLink release".into())
}

fn xml_name(name: &str) -> bool {
    name.ends_with(".xml")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && !name.contains("..")
}

fn includes(xml: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut includes = Vec::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(element) if element.name().as_ref() == b"include" => {
                let name = reader
                    .read_text(element.name())
                    .map_err(|e| e.to_string())?;
                let name = name.xml10_content().map_err(|e| e.to_string())?;
                let name = name.trim();
                if !xml_name(name) {
                    return Err(format!("Unsupported MAVLink include: {name}"));
                }
                includes.push(name.to_owned());
            }
            Event::Eof => return Ok(includes),
            _ => {}
        }
    }
}

fn definitions(
    bytes: &[u8],
    version: &Version,
    checksum: &str,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if format!("{:x}", Sha256::digest(bytes)) != checksum {
        return Err("MAVLink archive does not match the crates.io SHA-256 checksum".into());
    }
    let prefix = format!("mavlink-{version}/mavlink/");
    let xml_prefix = format!("{prefix}message_definitions/v1.0/");
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let mut files = BTreeMap::new();
    let mut total = 0;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry
            .path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        let name = if path == format!("{prefix}COPYING") {
            "COPYING"
        } else if let Some(name) = path.strip_prefix(&xml_prefix).filter(|name| xml_name(name)) {
            name
        } else {
            continue;
        };
        total += entry.size();
        if !entry.header().entry_type().is_file() || total > MAX_DOWNLOAD {
            return Err("MAVLink definitions must be regular files within the size limit".into());
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if files.insert(name.to_owned(), data).is_some() {
            return Err(format!("Duplicate MAVLink archive file: {name}"));
        }
    }
    let mut selected = BTreeMap::new();
    selected.insert(
        "COPYING".into(),
        files.remove("COPYING").ok_or("Missing upstream COPYING")?,
    );
    let mut pending = vec!["ardupilotmega.xml".to_owned()];
    while let Some(name) = pending.pop() {
        if selected.contains_key(&name) {
            continue;
        }
        let data = files
            .remove(&name)
            .ok_or_else(|| format!("Missing included dialect: {name}"))?;
        pending.extend(includes(&data)?);
        selected.insert(name, data);
    }
    Ok(selected)
}

pub fn update(requested: Option<&str>) -> Result<(), String> {
    // Reject invalid input before making a network request.
    if let Some(version) = requested {
        Version::parse(version).map_err(|e| e.to_string())?;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let (version, checksum) = release(&download(INDEX)?, requested)?;
    let url = format!("https://static.crates.io/crates/mavlink/mavlink-{version}.crate");
    println!("Fetching MAVLink {version} and verifying crates.io checksum");
    let mut files = definitions(&download(&url)?, &version, &checksum)?;
    files.insert(
        "README.md".into(),
        format!(
            "# MAVLink message definitions\n\n\
         Unmodified `ardupilotmega` XML and its recursive includes, including `common`\n\
         used by PX4, from the `mavlink` {version} crates.io archive.\n\n\
         Source: {url}\n\
         Archive SHA-256: `{checksum}`\n\
         Upstream: https://github.com/mavlink/mavlink\n\n\
         `COPYING` is retained verbatim, including the generator-output MIT exception.\n\
         `build/tlog.rs` generates metadata locally; ordinary builds need no downloads.\n\n\
         Update definitions, license, provenance, Rust MAVLink dependencies and lockfile,\n\
         then run importer tests with one command:\n\n\
         ```sh\ncargo xtask update-mavlink\n```\n\n\
         To select a specific published version: `cargo xtask update-mavlink {version}`.\n\
         Failed updates restore the previous files. Review and commit the resulting diff.\n"
        )
        .into_bytes(),
    );
    install(root, &files, &version, || {
        for package in ["mavlink-core", "mavlink-bindgen"] {
            super::cargo(
                &["update", "-p", package, "--precise", &version.to_string()],
                &[],
            )?;
        }
        super::cargo(&["test", "--locked", "-p", "enlumi", "log::tlog"], &[])
    })?;
    println!(
        "Updated {} XML dialects to MAVLink {version}; importer tests passed. Review and commit the diff.",
        files.len() - 2
    );
    Ok(())
}

fn install(
    root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    version: &Version,
    verify: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let manifest_path = root.join("Cargo.toml");
    let original_manifest = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let original_lock = fs::read(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    let mut manifest = original_manifest
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| e.to_string())?;
    manifest["dependencies"]["mavlink-core"]["version"] = toml_edit::value(version.to_string());
    manifest["build-dependencies"]["mavlink-bindgen"]["version"] =
        toml_edit::value(version.to_string());
    let staging = tempfile::tempdir_in(root.join("vendor")).map_err(|e| e.to_string())?;
    let next = staging.path().join("next");
    fs::create_dir(&next).map_err(|e| e.to_string())?;
    for (name, bytes) in files {
        fs::write(next.join(name), bytes).map_err(|e| e.to_string())?;
    }
    let destination = root.join("vendor/mavlink-dialects");
    let previous = staging.path().join("previous");
    fs::rename(&destination, &previous).map_err(|e| e.to_string())?;
    let result = (|| {
        fs::rename(&next, &destination).map_err(|e| e.to_string())?;
        fs::write(&manifest_path, manifest.to_string()).map_err(|e| e.to_string())?;
        verify()
    })();
    if let Err(error) = result {
        // Move failed files aside, rather than deleting anything outside the temporary directory.
        let rollback = (|| -> std::io::Result<()> {
            if destination.exists() {
                fs::rename(&destination, staging.path().join("failed"))?;
            }
            fs::rename(&previous, &destination)?;
            fs::write(&manifest_path, original_manifest)?;
            fs::write(root.join("Cargo.lock"), original_lock)
        })();
        if let Err(rollback_error) = rollback {
            let backup = staging.keep();
            return Err(format!(
                "{error}; rollback failed: {rollback_error}. Backup retained at {}",
                backup.display()
            ));
        }
        return Err(format!("{error}; previous MAVLink files restored"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> (Vec<u8>, String) {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for (name, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, name, *bytes).unwrap();
        }
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        (bytes, checksum)
    }

    #[test]
    fn release_selection_skips_yanked_and_prereleases_and_supports_pins() {
        let index = [
            ("0.19.0", false),
            ("0.19.1", false),
            ("0.20.0", true),
            ("0.21.0-rc.1", false),
        ]
        .map(|(version, yanked)| {
            serde_json::json!({
                "vers": version, "yanked": yanked, "cksum": "a".repeat(64),
            })
            .to_string()
        })
        .join("\n");
        assert_eq!(
            release(index.as_bytes(), None).unwrap().0.to_string(),
            "0.19.1"
        );
        assert_eq!(
            release(index.as_bytes(), Some("0.19.0"))
                .unwrap()
                .0
                .to_string(),
            "0.19.0"
        );
        assert!(release(index.as_bytes(), Some("0.20.0")).is_err());
        assert!(release(index.as_bytes(), Some("../../bad")).is_err());
    }

    #[test]
    fn archive_verifies_checksum_and_copies_only_recursive_includes_and_license() {
        let (bytes, checksum) = archive(&[
            ("mavlink-0.19.1/mavlink/COPYING", b"license\n"),
            (
                "mavlink-0.19.1/mavlink/message_definitions/v1.0/ardupilotmega.xml",
                b"<mavlink><include>common.xml</include></mavlink>",
            ),
            (
                "mavlink-0.19.1/mavlink/message_definitions/v1.0/common.xml",
                b"<mavlink><include>minimal.xml</include></mavlink>",
            ),
            (
                "mavlink-0.19.1/mavlink/message_definitions/v1.0/minimal.xml",
                b"<mavlink/>",
            ),
            (
                "mavlink-0.19.1/mavlink/message_definitions/v1.0/unused.xml",
                b"unused",
            ),
        ]);
        let version = Version::parse("0.19.1").unwrap();
        let files = definitions(&bytes, &version, &checksum).unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(files["COPYING"], b"license\n");
        assert_eq!(files["minimal.xml"], b"<mavlink/>");
        assert!(!files.contains_key("unused.xml"));
        assert!(
            definitions(&bytes, &version, &"0".repeat(64))
                .unwrap_err()
                .contains("checksum")
        );
        assert!(definitions(&bytes, &Version::parse("0.19.2").unwrap(), &checksum).is_err());
    }

    #[test]
    fn missing_dialects_and_path_includes_are_rejected() {
        assert!(includes(b"<mavlink><include>../outside.xml</include></mavlink>").is_err());
        assert!(includes(b"<mavlink><include>C:\\outside.xml</include></mavlink>").is_err());
        let (bytes, checksum) = archive(&[
            ("mavlink-0.19.1/mavlink/COPYING", b"license"),
            (
                "mavlink-0.19.1/mavlink/message_definitions/v1.0/ardupilotmega.xml",
                b"<mavlink><include>missing.xml</include></mavlink>",
            ),
        ]);
        assert!(
            definitions(&bytes, &Version::parse("0.19.1").unwrap(), &checksum)
                .unwrap_err()
                .contains("missing.xml")
        );
    }

    #[test]
    fn failed_verification_restores_directory_manifest_and_lockfile() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("vendor/mavlink-dialects")).unwrap();
        let original = "[dependencies]\nmavlink-core = { version = \"0.19.1\", default-features = false }\n[build-dependencies]\nmavlink-bindgen = { version = \"0.19.1\" }\n";
        fs::write(root.path().join("Cargo.toml"), original).unwrap();
        fs::write(root.path().join("Cargo.lock"), b"old lock").unwrap();
        fs::write(root.path().join("vendor/mavlink-dialects/old.xml"), b"old").unwrap();
        let files = BTreeMap::from([("new.xml".into(), b"new".to_vec())]);
        let version = Version::parse("0.20.0").unwrap();
        let error = install(root.path(), &files, &version, || {
            assert!(!root.path().join("vendor/mavlink-dialects/old.xml").exists());
            assert!(
                fs::read_to_string(root.path().join("Cargo.toml"))
                    .unwrap()
                    .contains("0.20.0")
            );
            fs::write(root.path().join("Cargo.lock"), b"changed lock").unwrap();
            Err("fixture check failed".into())
        })
        .unwrap_err();
        assert!(error.contains("previous MAVLink files restored"));
        assert_eq!(
            fs::read_to_string(root.path().join("Cargo.toml")).unwrap(),
            original
        );
        assert_eq!(
            fs::read(root.path().join("Cargo.lock")).unwrap(),
            b"old lock"
        );
        assert!(root.path().join("vendor/mavlink-dialects/old.xml").exists());
        assert!(!root.path().join("vendor/mavlink-dialects/new.xml").exists());
        install(root.path(), &files, &version, || Ok(())).unwrap();
        assert!(root.path().join("vendor/mavlink-dialects/new.xml").exists());
        assert!(!root.path().join("vendor/mavlink-dialects/old.xml").exists());
        assert!(
            fs::read_to_string(root.path().join("Cargo.toml"))
                .unwrap()
                .contains("default-features = false")
        );
    }
}
