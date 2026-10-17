use crate::{field_tree::TreeRow, log::LogStore};
use std::collections::BTreeSet;

pub fn rows(log: &LogStore, query: &str, expanded: &BTreeSet<String>) -> Vec<TreeRow> {
    let query = query.to_lowercase();
    let mut entries = Vec::new();
    for (flight, records) in &log.settings {
        for (index, setting) in records.iter().enumerate() {
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
            let open = array && (!query.is_empty() || expanded.contains(&id));
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
    let open = !query.is_empty() || expanded.contains("@settings");
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
