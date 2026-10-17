use crate::action_button::ActionButtonProps;
use crate::field_label::FieldLabelProps;
use crate::geometry_library::GeometryLibraryProps;
use crate::parameter_details::ParameterDetailsProps;
use crate::profile_library::ProfileLibraryProps;
use crate::{field_row::FieldRowProps, field_tree::FieldTree, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn Browser(workbench: Workbench) -> impl IntoView {
    let tree = Memo::new(move |_| {
        workbench.log.with(|log| {
            log.as_ref().map_or_else(FieldTree::default, |log| {
                let mut labels: std::collections::BTreeMap<_, _> =
                    workbench.profiles.metadata.with(|metadata| {
                        metadata
                            .iter()
                            .map(|(source, meta)| (source.clone(), meta.tree_label()))
                            .collect()
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
        let query = workbench.query.get();
        let expanded = workbench.expanded.get();
        let mut rows = Vec::new();
        let fields = workbench.derived.get();
        let matching: Vec<_> = fields
            .iter()
            .filter(|field| {
                "derived fields".contains(&query.to_lowercase())
                    || workbench
                        .field_name(&field.key())
                        .to_lowercase()
                        .contains(&query.to_lowercase())
            })
            .collect();
        if !matching.is_empty() {
            let open = expanded.contains("@derived");
            rows.push(crate::field_tree::TreeRow {
                id: "@derived".into(),
                label: "Derived fields".into(),
                depth: 0,
                count: matching.len(),
                field: None,
                open,
                last: false,
                guides: Vec::new(),
            });
            if open {
                for (index, field) in matching.iter().enumerate() {
                    rows.push(crate::field_tree::TreeRow {
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
        }
        workbench.log.with(|log| {
            if let Some(log) = log {
                rows.extend(crate::parameter_rows::rows(log, &query, &expanded));
            }
        });
        rows.extend(tree.with(|tree| tree.rows(&query, &expanded)));
        rows
    });
    let visible_fields = Memo::new(move |_| {
        rows.with(|rows| {
            rows.iter()
                .filter(|row| row.count > 0)
                .filter_map(|row| row.field.clone())
                .collect::<Vec<_>>()
        })
    });
    let resize = RwSignal::new_local(None::<(f32, f32)>);
    view! {
        row(class = "browser", class:hidden = move || !workbench.sidebar_visible.get(), style:width = move || Some(format!("{}px", workbench.sidebar_width.get() + 6.0))) {
            column(class = "signal-sidebar") {
                row(class = "browser-heading") {
                    ActionButton(class = "fields-menu", class:setting-active = move || !workbench.profiles.browsing.get(), on:click = move |_| workbench.profiles.browsing.set(false)) { "Fields" }
                    ActionButton(class = "profiles-trigger", class:setting-active = move || workbench.profiles.browsing.get(), on:click = move |_| workbench.profiles.browsing.set(true)) { "Profiles" }
                    ActionButton(class = "open-filters", node_ref = workbench.filter_button, class:setting-active = move || workbench.filters_open.get(), disabled = Signal::derive_local(move || workbench.log.get().is_none() || workbench.loading.get()), on:click = move |_| workbench.filters_open.update(|open| *open = !*open)) { "Filters" }
                    if move || !workbench.profiles.browsing.get() {
                        ActionButton(class = "collapse-fields", on:click = move |_| workbench.expanded.set(Default::default())) { "Collapse all" }
                    }
                }
                if move || workbench.profiles.browsing.get() {
                    ProfileLibrary(workbench = workbench)
                } else {
                InputGroup(class = "field-search") {
                    InputGroupInput(value = workbench.query, placeholder = "Search topics and fields", a11y:label = "Search log fields")
                    if move || !workbench.query.with(String::is_empty) {
                        InputGroupAddon(align = InputGroupAddonAlign::InlineEnd, on:click = move |_| workbench.query.set(String::new())) {
                            InputGroupButton(size = InputGroupButtonSize::IconXs, a11y:label = "Clear search") { "×" }
                        }
                    }
                }
                text(class = "muted") {{move || workbench.log.with(|log| log.as_ref().map_or_else(|| "Open a log to browse fields".into(), |log| format!("{} fields | drag onto a plot", log.catalogue.len())))}}
                GeometryLibrary(workbench = workbench)
                VirtualList(count = Signal::derive_local(move || rows.with(Vec::len)), row_size = 28.0, label = "Log field tree", class = "signal-list",
                    row = move |index: usize| view! { FieldRow(workbench = workbench, row = Memo::new(move |_| rows.with(|rows| rows.get(index).cloned())), visible_fields = visible_fields) })
                ParameterDetails(workbench = workbench)
                row(class = "selected-field-row", class:hidden = move || workbench.selected_field.get().is_some_and(|id| id.starts_with("@setting/"))) {
                text(class = "selected-field") {{move || workbench.selected_field.get().map_or_else(|| "Select a field or drag it onto a plot".into(), |field| workbench.field_name(&field))}}
                    FieldLabel(workbench = workbench)
                }
                text(class = "field-selection-count muted") {{move || format!("{} selected · drag selected fields together", workbench.selected_fields.with(|fields| fields.len()))}}
                text(class = "muted") { "Ctrl-click toggles fields. Shift-click selects range. Drag selection onto plot or Filters. Enter plots focused field." }
                if move || workbench.log.get().is_some_and(|log| !log.warnings.is_empty()) {
                    text(class = "warnings") {{move || workbench.log.with(|log| log.as_ref().map_or_else(String::new, |log| format!("{} load warnings", log.warnings.len())))}}
                }
                }
            }
            control(class = "sidebar-grip", tabindex = Focus::Sequential, a11y:label = "Resize field browser",
                on:pointer_down = move |event| { resize.set(Some((event.position.x.0, workbench.sidebar_width.get_untracked()))); event.capture_pointer(); },
                on:pointer_move = move |event| { if let Some((x, width)) = resize.get_untracked() { workbench.sidebar_width.set((width + event.position.x.0 - x).clamp(220.0, 480.0)); } },
                on:pointer_up = move |event| { resize.set(None); event.release_pointer(); },
                on:pointer_cancel = move |event| { resize.set(None); event.release_pointer(); },
                on:key_down = move |event| {
                    let change = match event.key { zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowLeft) => -16.0, zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight) => 16.0, _ => return };
                    workbench.sidebar_width.update(|width| *width = (*width + change).clamp(220.0, 480.0)); event.prevent_default(); event.stop_propagation();
                }) {}
        }
    }
}
