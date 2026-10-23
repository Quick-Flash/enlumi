use crate::{
    field_tree::{FieldTree, TreeRow},
    state::Workbench,
};
use zgui::prelude::*;

pub fn rows(workbench: Workbench) -> Memo<Vec<TreeRow>> {
    let tree = Memo::new(move |_| {
        workbench.log.with(|log| {
            log.as_ref().map_or_else(FieldTree::default, |log| {
                let mut labels = workbench.profiles.metadata.with(|metadata| {
                    metadata
                        .iter()
                        .map(|(source, meta)| (source.clone(), meta.tree_label()))
                        .collect::<std::collections::BTreeMap<_, _>>()
                });
                labels.extend(workbench.profiles.groups.get());
                let names = workbench.profiles.metadata.with(|metadata| {
                    metadata
                        .iter()
                        .map(|(source, meta)| (source.clone(), meta.label()))
                        .collect()
                });
                let tree = FieldTree::new(&log.catalogue)
                    .with_labels(&labels)
                    .with_search_labels(&names);
                if log.blackbox_headers.len() == 1 {
                    tree.without_single_root()
                } else {
                    tree
                }
            })
        })
    });
    let rows = Memo::new(move |_| {
        let query = workbench.field_query.get().trim().to_lowercase();
        let expanded = workbench.expanded.get();
        let raw_query = if "raw".contains(&query) { "" } else { &query };
        let mut parameter_expanded = expanded.clone();
        if expanded.contains("@parameters") {
            parameter_expanded.insert("@settings".into());
        }
        let parameters = workbench.log.with(|log| {
            log.as_ref().map_or_else(Vec::new, |log| {
                crate::parameter_rows::rows(log, &query, &parameter_expanded)
            })
        });
        let mut raw = tree.with(|tree| tree.rows(raw_query, &expanded));
        for row in &mut raw {
            row.depth += 1;
        }
        let parameter_count = parameters.first().map_or(0, |row| row.count);
        let count: usize = tree.with(|tree| {
            tree.rows(raw_query, &Default::default())
                .iter()
                .map(|row| if row.field.is_some() { 1 } else { row.count })
                .sum()
        });
        let parameter_open = expanded.contains("@parameters");
        let mut rows = Vec::new();
        if parameter_count > 0 {
            rows.push(section(
                "@parameters",
                "Parameters",
                parameter_count,
                parameter_open,
            ));
            if parameter_open {
                rows.extend(parameters.into_iter().skip(1));
            }
        }
        let raw_open = expanded.contains("@raw");
        rows.push(section("@raw", "Raw", count, raw_open));
        if raw_open {
            rows.extend(raw);
        }
        let fields = workbench.derived.get();
        let matching = fields
            .iter()
            .filter(|field| {
                "derived".contains(&query)
                    || field.key().to_lowercase().contains(&query)
                    || workbench
                        .field_name(&field.key())
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        let open = expanded.contains("@derived");
        rows.push(section("@derived", "Derived", matching.len(), open));
        if open {
            for (index, field) in matching.iter().enumerate() {
                rows.push(TreeRow {
                    id: field.key(),
                    label: workbench.field_name(&field.key()),
                    depth: 1,
                    count: field.data.as_ref().map_or(0, |data| data.samples().len()),
                    field: Some(field.key()),
                    open: false,
                    last: index + 1 == matching.len(),
                    guides: Vec::new(),
                });
            }
        }
        crate::field_tree::connect(&mut rows);
        rows
    });
    let sync = zgui::reactive::RenderEffect::new(move |_| {
        workbench.browser_fields.set(rows.with(|rows| {
            rows.iter()
                .filter(|row| row.count > 0)
                .filter_map(|row| row.field.clone())
                .collect()
        }));
    });
    on_cleanup_local(move || drop(sync));
    rows
}

fn section(id: &str, label: &str, count: usize, open: bool) -> TreeRow {
    TreeRow {
        id: id.into(),
        label: label.into(),
        depth: 0,
        count,
        field: None,
        open,
        last: true,
        guides: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_preserves_independent_sections_and_matches_derived_labels() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let rows = window.scope.with(|| rows(workbench));
        workbench.derived.set(vec![crate::derived::DerivedField {
            id: 10,
            name: "Smoothed RPM".into(),
            sources: vec![],
            group: 10,
            filter: crate::derived::FilterSpec::MovingAverage(3),
            data: None,
        }]);
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@raw" && !row.open)
        );
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@derived" && !row.open && row.count == 1)
        );
        workbench.field_query.set("smoothed".into());
        assert!(
            rows.get_untracked().iter().all(|row| row.field.is_none()),
            "Search must not open sections"
        );
        workbench.toggle_branch("@derived".into());
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@derived/10")
        );
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@raw" && !row.open)
        );
        workbench.field_query.set("absent".into());
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@derived" && row.open && row.count == 0)
        );
        workbench.field_query.set(String::new());
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@derived/10")
        );
        assert!(
            rows.get_untracked()
                .iter()
                .any(|row| row.id == "@raw" && !row.open)
        );
    }
}
