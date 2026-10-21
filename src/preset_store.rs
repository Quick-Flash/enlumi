use crate::preset_capture::Bundle;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(test)]
pub fn save(
    bundle: Bundle,
    path: &Path,
    replace: bool,
    origin: Option<&Path>,
) -> Result<(), String> {
    save_with_deleted_filters(bundle, path, replace, origin, &Default::default())
}

pub fn save_with_deleted_filters(
    mut bundle: Bundle,
    path: &Path,
    replace: bool,
    origin: Option<&Path>,
    deleted: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let previous = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let mut document = if let Some(bytes) = &previous {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({"version":2,"name":bundle.document["name"]})
    };
    let existing: crate::profile::Profile = serde_json::from_value(document.clone())
        .map_err(|e| format!("Destination profile: {e}"))?;
    crate::preset::validate(&existing)?;
    reuse_dependencies(&mut bundle.document, &mut document, replace)?;
    for collection in [
        "signals",
        "filters",
        "geometry",
        "plot_presets",
        "view_presets",
        "workspace_presets",
    ] {
        let key = if matches!(
            collection,
            "plot_presets" | "view_presets" | "workspace_presets"
        ) {
            "name"
        } else {
            "id"
        };
        let additions = bundle.document[collection]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let target = document
            .as_object_mut()
            .ok_or("Profile must be an object")?
            .entry(collection)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .ok_or("Invalid profile collection")?;
        for addition in additions {
            if let Some(index) = target.iter().position(|entry| entry[key] == addition[key]) {
                if target[index] == addition {
                    continue;
                }
                if !replace {
                    return Err(format!(
                        "{} already exists. Enable Replace existing entries to update it.",
                        addition[key]
                    ));
                }
                target[index] = addition;
            } else {
                target.push(addition);
            }
        }
    }
    let plot_names = bundle.document["plot_presets"]
        .as_array()
        .map(|plots| {
            plots
                .iter()
                .filter_map(|plot| plot["name"].as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(legacy) = document
        .get_mut("views")
        .and_then(serde_json::Value::as_array_mut)
    {
        if legacy
            .iter()
            .any(|view| plot_names.iter().any(|name| view["name"] == *name))
            && !replace
        {
            return Err(
                "A plot preset with this name already exists. Enable Replace existing entries to update it.".into(),
            );
        }
        legacy.retain(|view| !plot_names.iter().any(|name| view["name"] == *name));
    }
    let deleted = deleted
        .iter()
        .filter(|id| {
            !bundle.document["filters"]
                .as_array()
                .is_some_and(|filters| {
                    filters
                        .iter()
                        .any(|filter| filter["id"].as_str() == Some(id.as_str()))
                })
        })
        .cloned()
        .collect();
    prune_deleted_filters(&mut document, &deleted);
    document["version"] = serde_json::json!(2);
    write_document(document, path, previous, bundle.files, origin)
}

fn prune_deleted_filters(
    document: &mut serde_json::Value,
    deleted: &std::collections::BTreeSet<String>,
) {
    fn references(value: &serde_json::Value, token: &str) -> bool {
        match value {
            serde_json::Value::String(value) => value == token,
            serde_json::Value::Array(values) => values.iter().any(|value| references(value, token)),
            serde_json::Value::Object(values) => {
                values.values().any(|value| references(value, token))
            }
            _ => false,
        }
    }
    loop {
        let removable = document["filters"].as_array().and_then(|filters| {
            filters.iter().position(|filter| {
                filter["id"].as_str().is_some_and(|id| {
                    deleted.contains(id) && !references(document, &format!("${id}"))
                })
            })
        });
        let Some(index) = removable else {
            break;
        };
        document["filters"].as_array_mut().unwrap().remove(index);
    }
}

pub fn write_document(
    mut document: serde_json::Value,
    path: &Path,
    previous: Option<Vec<u8>>,
    mut files: std::collections::BTreeMap<String, String>,
    origin: Option<&Path>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or("Choose an absolute profile file path")?;
    if !path.is_absolute() {
        return Err("Choose an absolute profile file path".into());
    }
    // Copy all Rust dependencies, including unchanged entries during Save as.
    if let Some(filters) = document
        .get_mut("filters")
        .and_then(serde_json::Value::as_array_mut)
    {
        for filter in filters {
            if filter["operation"]["kind"] != "rust" {
                continue;
            }
            let reference = filter["operation"]["path"]
                .as_str()
                .ok_or("Rust filter needs a source path")?;
            let source = if let Some(source) = files.get(reference) {
                source.clone()
            } else {
                let base = origin.and_then(Path::parent).unwrap_or(parent);
                let base = base.canonicalize().map_err(|e| e.to_string())?;
                let source_path = base
                    .join(reference)
                    .canonicalize()
                    .map_err(|e| format!("Missing Rust dependency {reference}: {e}"))?;
                if !source_path.starts_with(&base) {
                    return Err("Rust dependency must remain inside profile directory".into());
                }
                std::fs::read_to_string(source_path).map_err(|e| e.to_string())?
            };
            if source.is_empty() || source.len() > 262144 {
                return Err("Rust dependency must contain 1–262144 bytes".into());
            }
            let reference = format!(
                "sources/{:016x}.rs",
                crate::preset_capture::hash(source.as_bytes())
            );
            files.insert(reference.clone(), source);
            filter["operation"]["path"] = serde_json::json!(reference);
        }
    }
    let profile: crate::profile::Profile =
        serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
    crate::preset::validate(&profile)?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Profile exceeds 4 MiB".into());
    }
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    for (name, source) in files {
        let relative = PathBuf::from(&name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Invalid Rust source destination".into());
        }
        let destination = parent.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(mut file) => {
                file.write_all(source.as_bytes())
                    .map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(&destination).map_err(|e| e.to_string())? != source.as_bytes() {
                    return Err("Rust dependency destination contains different contents".into());
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(&bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    let current = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if current != previous {
        return Err("Profile changed while saving. Reload before trying again.".into());
    }
    if previous.is_none() {
        temporary
            .persist_noclobber(path)
            .map_err(|e| e.to_string())?;
    } else {
        temporary.persist(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Canonicalize shared dependencies before merging named recipes.
fn reuse_dependencies(
    incoming: &mut serde_json::Value,
    existing: &mut serde_json::Value,
    replace: bool,
) -> Result<(), String> {
    for collection in ["signals", "filters", "geometry"] {
        let additions = incoming[collection].as_array().cloned().unwrap_or_default();
        for addition in additions {
            // Earlier dependencies may have remapped this entry's inputs.
            let Some(addition) = incoming[collection]
                .as_array()
                .and_then(|entries| entries.iter().find(|entry| entry["id"] == addition["id"]))
                .cloned()
            else {
                continue;
            };
            let found = existing[collection].as_array().and_then(|entries| {
                entries.iter().position(|entry| match collection {
                    "signals" => entry["source"] == addition["source"],
                    "filters" => {
                        resolved_inputs(&entry["inputs"], existing, incoming)
                            == resolved_inputs(&addition["inputs"], incoming, existing)
                            && normalized_operation(&entry["operation"])
                                == normalized_operation(&addition["operation"])
                            && entry["metadata"] == addition["metadata"]
                    }
                    _ => entry["name"] == addition["name"],
                })
            });
            let Some(index) = found else {
                continue;
            };
            let original = addition["id"].as_str().ok_or("Dependency needs ID")?;
            let canonical = existing[collection][index]["id"]
                .as_str()
                .ok_or("Dependency needs ID")?
                .to_owned();
            let mut normalized = addition.clone();
            normalized["id"] = serde_json::json!(canonical);
            if collection == "filters" {
                normalized["inputs"] = existing[collection][index]["inputs"].clone();
                normalized["operation"] = existing[collection][index]["operation"].clone();
            }
            if normalized != existing[collection][index] {
                if !replace {
                    return Err(format!(
                        "Shared {collection} entry {canonical} differs. Enable Replace existing entries to update it."
                    ));
                }
                existing[collection][index] = normalized;
            }
            incoming[collection]
                .as_array_mut()
                .unwrap()
                .retain(|entry| entry["id"] != original);
            remap(incoming, &format!("${original}"), &format!("${canonical}"));
            remap(
                incoming,
                &format!("line:${original}"),
                &format!("line:${canonical}"),
            );
            if collection == "geometry" {
                remap(incoming, original, &canonical);
                remap(
                    incoming,
                    &format!("geometry:{original}"),
                    &format!("geometry:{canonical}"),
                );
            }
        }
    }
    Ok(())
}
fn normalized_operation(value: &serde_json::Value) -> Option<serde_json::Value> {
    serde_json::from_value::<crate::profile::Operation>(value.clone())
        .ok()
        .and_then(|operation| serde_json::to_value(operation).ok())
}
fn resolved_inputs(
    value: &serde_json::Value,
    primary: &serde_json::Value,
    fallback: &serde_json::Value,
) -> serde_json::Value {
    let resolve = |value: &serde_json::Value| {
        let mut value = value.clone();
        for _ in 0..64 {
            let Some(token) = value.as_str().and_then(|v| v.strip_prefix('$')) else {
                break;
            };
            let source = [primary, fallback].into_iter().find_map(|document| {
                document["signals"]
                    .as_array()?
                    .iter()
                    .find(|signal| signal["id"] == token)
                    .map(|signal| signal["source"].clone())
            });
            let Some(source) = source else {
                break;
            };
            value = source;
        }
        value
    };
    value.as_array().map_or_else(
        || value.clone(),
        |inputs| serde_json::Value::Array(inputs.iter().map(resolve).collect()),
    )
}
fn remap(value: &mut serde_json::Value, from: &str, to: &str) {
    match value {
        serde_json::Value::String(text) if text == from => *text = to.into(),
        serde_json::Value::Array(values) => {
            for value in values {
                remap(value, from, to);
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(value) = values.remove(from) {
                values.insert(to.into(), value);
            }
            for value in values.values_mut() {
                remap(value, from, to);
            }
        }
        _ => {}
    }
}
