use crate::tree_lines::{TreeBranchProps, TreeLinesProps};
use crate::{
    profile::identity::{Drag, Item, ProfileKey},
    profile_library::ProfileFile,
    profile_output_row::ProfileOutputRowProps,
    state::Workbench,
    workspace_collection::WorkspaceCollectionProps,
    workspace_preset::WorkspacePresetProps,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct ProfileViewport(pub NodeRef);
#[derive(Clone, Copy)]
pub struct VirtualProfileFields;
use zgui::prelude::*;

#[derive(PartialEq, Eq)]
struct PresetEntry {
    file: Arc<ProfileFile>,
    item: Item,
    name: String,
    count: usize,
    search: String,
}

#[component]
pub fn ProfilePresetTree(
    workbench: Workbench,
    files: Signal<Vec<ProfileFile>>,
    field_query: Signal<String, LocalStorage>,
) -> impl IntoView {
    provide_local_context(crate::profile_outputs::ProfileFieldData::new(workbench));
    // Share each catalogue across its rows instead of copying all fields per row.
    let files = Memo::new(move |_| {
        files.with(|files| files.iter().cloned().map(Arc::new).collect::<Vec<_>>())
    });
    view! {
        column(class="profile-preset-tree") {
            for group in move ||["Workspaces", "Views", "Plots", "Fields", "Geometry"],key=|group|*group {
                {{let open=RwSignal::new_local(matches!(group,"Workspaces"|"Views"|"Plots"));
                let indexed=Memo::new(move |_| {
                    let mut entries=Vec::new();
                    let flight=workbench.profiles.flight.get();
                    for file in files.get() {
                        let key=ProfileKey::new(file.path.clone(),flight);
                        let prefix=format!("{} {} {}",group,file.name,file.path.display()).to_lowercase();
                        let items:Vec<_>=match group {
                            "Workspaces"=>file.collections.iter().map(|(name,views)|(Item::WorkspacePreset(name.clone()),name.clone(),views.len())).collect(),
                            "Views"=>file.workspaces.iter().map(|name|(Item::Workspace(name.clone()),name.clone(),0)).collect(),
                            "Plots"=>file.views.iter().map(|(name,count)|(Item::View(name.clone()),name.clone(),*count)).chain(file.workspace_plots.iter().flat_map(|(view,plots)|plots.iter().enumerate().map(move |(index,(name,count))|(Item::View(format!("@workspace/{view}/{index}")),format!("{view} / {name}"),*count)))).collect(),
                            "Fields"=>file.fields.iter().map(|(id,name)|(Item::Field(format!("${id}")),name.clone(),0)).collect(),
                            _=>file.geometry.iter().map(|(id,name)|(Item::Geometry(id.clone()),name.clone(),0)).collect(),
                        };
                        for (item,name,count) in items {
                            let resolved=if let Item::Field(token)=&item {workbench.profile_field_search_text(&key,token)} else {String::new()};
                            let search=format!("{prefix} {} {} {resolved}",name.to_lowercase(),item.name().to_lowercase());
                            entries.push(Arc::new(PresetEntry{file:file.clone(),item,name,count,search}));
                        }
                    }
                    entries.sort_by_cached_key(|entry|(entry.name.to_lowercase(),entry.file.path.clone()));
                    entries
                });
                let entries=Memo::new(move |_| {
                    // Only Fields subscribes to search; saved editors keep their state.
                    let query=if group=="Fields" {field_query.get().trim().to_lowercase()} else {String::new()};
                    indexed.with(|entries|entries.iter().filter(|entry|query.is_empty()||entry.search.contains(&query)).cloned().collect::<Vec<_>>())
                });view! {
                    if move ||!entries.with(Vec::is_empty) {
                        column(class="profile-section") {
                            TreeBranch(open=open,name=group.to_string(),count=Signal::derive_local(move ||entries.with(Vec::len)))
                            if move ||open.get() {
                                {if group=="Fields" {AnyView::new(view! {ProfileFieldRows(workbench=workbench,entries=entries)})} else {AnyView::new(view! {
                                    for entry in move ||entries.get().into_iter().enumerate().collect::<Vec<_>>(),key=|(_,entry)|(Arc::as_ptr(&entry.file) as usize,entry.item.clone()) {
                                        {{let last=is_last(entries,&entry.1);view! {ProfileEntry(workbench=workbench,entry=entry.1,last=last)}}}
                                    }
                                })}}
                            }
                        }
                    }
                }}}
            }
        }
    }
}

fn is_last(
    entries: Memo<Vec<Arc<PresetEntry>>>,
    entry: &Arc<PresetEntry>,
) -> Signal<bool, LocalStorage> {
    let file = entry.file.clone();
    let item = entry.item.clone();
    Signal::derive_local(move || {
        entries.with(|entries| {
            entries
                .last()
                .is_some_and(|entry| Arc::ptr_eq(&entry.file, &file) && entry.item == item)
        })
    })
}

#[component]
fn ProfileEntry(
    workbench: Workbench,
    entry: Arc<PresetEntry>,
    last: Signal<bool, LocalStorage>,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    let file = StoredValue::new_local(entry.file.clone());
    let item = StoredValue::new_local(entry.item.clone());
    let name = entry.name.clone();
    let count = entry.count;
    let content = match &entry.item {
        Item::WorkspacePreset(_) => {
            AnyView::new(view! {WorkspaceCollection(workbench=workbench,file=file,name=name)})
        }
        Item::Workspace(_) => {
            AnyView::new(view! {WorkspacePreset(workbench=workbench,file=file,name=name)})
        }
        _ => AnyView::new(
            view! {ProfileOutputRow(workbench=workbench,file=file,drag=Signal::derive_local(move ||Drag{profile:ProfileKey::new(file.with_value(|file|file.path.clone()),workbench.profiles.flight.get()),item:item.get_value()}),name=name,count=count,compact=compact)},
        ),
    };
    view! {column(class="profile-tree-entry",class:profile-field-entry=compact) {
        {content}
        TreeLines(depth=2,last=last,guides=vec![0])
    }}
}

#[component]
fn ProfileFieldRows(workbench: Workbench, entries: Memo<Vec<Arc<PresetEntry>>>) -> impl IntoView {
    provide_local_context(VirtualProfileFields);
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let viewport = use_local_context::<ProfileViewport>().map(|port| port.0);
    let scroll = viewport.map(|port| port.observe_scroll());
    let viewport_bounds = viewport.map(|port| port.observe_border_box());
    let window = Memo::new(move |_| {
        let count = entries.with(Vec::len);
        bounds.track();
        if let Some(scroll) = scroll {
            scroll.track();
        }
        if let Some(bounds) = viewport_bounds {
            bounds.track();
        }
        let (height, offset) = viewport
            .and_then(|port| {
                let area = port.window_bounds()?;
                let list = host.window_bounds()?;
                let scale = port.scale();
                Some((
                    area.size.height.0 / scale,
                    (area.origin.y.0 - list.origin.y.0).max(0.) / scale,
                ))
            })
            .unwrap_or((
                if viewport.is_some() {
                    0.
                } else {
                    count as f32 * 28.
                },
                0.,
            ));
        zgui_ui::virtualize::window(count, 28., height, offset, 4)
    });
    // Range selection follows all expanded results, including rows not mounted.
    let field_data = use_local_context::<crate::profile_outputs::ProfileFieldData>()
        .expect("Profile field data");
    let sources = Memo::new(move |_| {
        if !workbench.profiles.open.get()
            || workbench.log.with(Option::is_none)
            || workbench.loading.get()
            || workbench.profiles.busy.get()
            || workbench.derived_busy.get()
        {
            return Vec::new();
        }
        let flight = workbench.profiles.flight.get();
        workbench.profiles.instances.with(|instances| {
            field_data.counts.with(|counts| {
                entries.with(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            let key = ProfileKey::new(entry.file.path.clone(), flight);
                            if let Some(instance) = instances.get(&key) {
                                let Item::Field(token) = &entry.item else {
                                    return None;
                                };
                                let source = instance.resolve(token)?;
                                if crate::derived::id(&source).is_some()
                                    && !counts.contains_key(&source)
                                {
                                    return None;
                                }
                            } else if entry.file.compatible.is_err() {
                                return None;
                            }
                            Some(
                                Drag {
                                    profile: key,
                                    item: entry.item.clone(),
                                }
                                .source(),
                            )
                        })
                        .collect()
                })
            })
        })
    });
    let sources = Signal::derive_local(move || sources.get());
    let row_id = workbench.id();
    workbench.profiles.rows.update_value(|rows| {
        rows.insert(
            row_id,
            crate::profile_selection::ProfileSourceRow {
                source: Signal::stored_local(None),
                node: host,
                range: Some(sources),
            },
        );
    });
    on_cleanup_local(move || {
        workbench.profiles.rows.update_value(|rows| {
            rows.remove(&row_id);
        })
    });
    view! {column(class="profile-field-rows",node_ref=host,
        style:padding-top=move ||Some(format!("{}px",window.get().lead)),style:padding-bottom=move ||Some(format!("{}px",window.get().trail))) {
        for entry in move ||window.get().indices().into_iter().filter_map(|index|entries.with(|entries|entries.get(index).cloned()).map(|entry|(index,entry))).collect::<Vec<_>>(),key=|(_,entry)|(Arc::as_ptr(&entry.file) as usize,entry.item.clone()) {
            {{let last=is_last(entries,&entry.1);view! {ProfileEntry(workbench=workbench,entry=entry.1,last=last,compact=true)}}}
        }
    }}
}
