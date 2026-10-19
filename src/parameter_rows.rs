use crate::{field_tree::TreeRow, log::LogStore};
use std::collections::BTreeSet;

pub fn rows(log: &LogStore, query: &str, expanded: &BTreeSet<String>) -> Vec<TreeRow> {
    let query = query.trim().to_lowercase();
    let mut entries = Vec::new();
    for (flight, records) in &log.settings {
        for (index, setting) in records.iter().enumerate() {
            if setting.decoder_header() {
                continue;
            }
            let id = format!("@setting/{flight}/{index}");
            let label = log.setting_label(&id).unwrap_or_default();
            if !query.is_empty()
                && !"parameters settings".contains(&query)
                && !label.to_lowercase().contains(&query)
            {
                continue;
            }
            let name = if log.settings.len() > 1 {
                format!("Flight {flight} / {}", setting.name)
            } else {
                setting.name.clone()
            };
            let values = setting.numbers().unwrap_or_default();
            let array = values.len() > 1;
            let open = array && expanded.contains(&id);
            entries.push(TreeRow {
                id: id.clone(),
                label: name.clone(),
                depth: 1,
                count: if array {
                    values.len()
                } else if values.len() == 1 {
                    2
                } else {
                    0
                },
                field: if array { None } else { Some(id.clone()) },
                open,
                last: false,
                guides: Vec::new(),
            });
            if open {
                for element in 0..values.len() {
                    let key = format!("{id}/{element}");
                    entries.push(TreeRow {
                        id: key.clone(),
                        label: format!("{}[{element}]", setting.name),
                        depth: 2,
                        count: 2,
                        field: Some(key),
                        open: false,
                        last: element + 1 == values.len(),
                        guides: Vec::new(),
                    });
                }
            }
        }
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let open = expanded.contains("@settings");
    let mut rows = vec![TreeRow {
        id: "@settings".into(),
        label: "Parameters".into(),
        depth: 0,
        count: entries.iter().filter(|entry| entry.depth == 1).count(),
        field: None,
        open,
        last: false,
        guides: Vec::new(),
    }];
    if open {
        rows.extend(entries);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::LogSetting;

    #[test]
    fn search_filters_parameters_without_overriding_manual_expansion() {
        let mut log = LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap();
        log.settings = std::collections::BTreeMap::from([(
            1,
            vec![
                LogSetting {
                    name: "GAIN".into(),
                    value: "[1, 2, 3]".into(),
                    kind: "Initial parameter".into(),
                },
                LogSetting {
                    name: "RATE".into(),
                    value: "100".into(),
                    kind: "Initial parameter".into(),
                },
            ],
        )]);
        let mut expanded = BTreeSet::new();
        let collapsed = rows(&log, " GAIN ", &expanded);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].count, 1);
        assert!(!collapsed[0].open);

        expanded.insert("@settings".into());
        let matching = rows(&log, "gain", &expanded);
        assert_eq!(matching.len(), 2);
        assert_eq!(matching[1].label, "GAIN");
        assert!(!matching[1].open);
        expanded.insert("@setting/1/0".into());
        let open = rows(&log, "gain", &expanded);
        assert_eq!(open.len(), 5);
        assert_eq!(
            open.last().unwrap().field.as_deref(),
            Some("@setting/1/0/2")
        );
        assert_eq!(rows(&log, "", &expanded).len(), 6);
        assert_eq!(rows(&log, "parameters", &expanded).len(), 6);
        assert!(rows(&log, "missing", &expanded).is_empty());

        expanded.remove("@setting/1/0");
        assert_eq!(rows(&log, "gain", &expanded).len(), 2);
        expanded.remove("@settings");
        assert_eq!(rows(&log, "gain", &expanded).len(), 1);
        assert!(expanded.is_empty());
    }
}
