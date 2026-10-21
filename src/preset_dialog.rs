use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PresetDialog(workbench: Workbench) -> impl IntoView {
    let ui = workbench.authoring;
    view! {
        Dialog(open=ui.open) {
            DialogContent(class="profile-builder") {
                DialogHeader {DialogTitle {{move ||if ui.session.with(|session|session.as_ref().is_some_and(|session|session.kind.is_none())){"Edit profile"}else{"Save to profile"}}} DialogDescription {"Reuse fields, filters, plots, and layouts. Zoom and analysis time selections stay in current session."}}
                ScrollArea(class="profile-builder-body",label="Profile settings and contents") {
                column(class="profile-builder-form") {
                text(class="preset-modified muted") {{move ||ui.session.with(|session|session.as_ref().map_or("",|session|ui.baseline.with_value(|baseline|if let Some(document)=baseline {if document==&session.bundle.document && document["name"].as_str()==Some(ui.name.get().as_str()) {"Saved"}else{"Modified"}}else{"New content"})))}}
                text(class="muted") {"Name"} Input(value=ui.name,class="preset-name",a11y:label="Preset or profile name")
                text(class="muted") {"Destination profile"}
                Select(value=Binding::controlled(ui.path,move |path|ui.path.set(path))) {
                    SelectTrigger(label="Loaded destination profile") {text {{move || if ui.path.get().is_empty(){"Choose profile or file".into()}else{std::path::Path::new(&ui.path.get()).file_name().unwrap_or_default().to_string_lossy().into_owned()}}}}
                    SelectContent {for path in move ||workbench.profiles.instances.with(|instances|instances.keys().map(|key|key.path.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>()),key=|path|path.clone() {
                        SelectItem(value=path.display().to_string()) {{path.file_name().unwrap_or_default().to_string_lossy().into_owned()}}
                    }}
                }
                row(class="profile-file-controls") {
                    Input(value=ui.path,class="preset-path",a11y:label="Profile file path")
                    ActionButton(class="preset-choose-file",disabled=Signal::derive_local(move ||ui.busy.get()),on:click=move |_|{
                        let current=std::path::PathBuf::from(ui.path.get_untracked());workbench.task(async move {let path=blocking(move ||{let dialog=rfd::FileDialog::new().add_filter("Enlumi profile",&["json"]);let dialog=if let Some(parent)=current.parent(){dialog.set_directory(parent)}else{dialog};dialog.save_file()}).await;if let Some(path)=path{ui.path.set(path.display().to_string());}});
                    }) {"Choose file…"}
                }
                {{let replace=NodeRef::new();view!{row(class="profile-replace-option") {
                    Checkbox(node_ref=replace,checked=Binding::controlled(Signal::derive_local(move ||if ui.replace.get(){Checked::Yes}else{Checked::No}),move |value|ui.replace.set(value==Checked::Yes)))
                    Label(control=replace) {"Replace existing entries with matching names"}
                }}}}
                text(class="muted") {"Add fields from sidebar Save or field context menu. Arrange plots in workspace, then save view preset."}
                text(class="muted") {"Name labels plots and legends. Component is an optional short name for field tree leaves."}
                column(class="profile-builder-preview",a11y:label="Profile contents and dependency preview") {
                    for collection in move ||["workspace_presets","view_presets","plot_presets","views","signals","filters","geometry"],key=|collection|*collection {
                        if move ||collection=="view_presets" && ui.session.with(|session|session.as_ref().is_some_and(|session|matches!(session.kind,Some(crate::preset_capture::CaptureKind::View)))) {
                            column(class="captured-view-plots") {
                                text(class="heading") {"Plots"}
                                RecipePreview(workbench=workbench,collection=collection,index=0,always_open=true)
                            }
                        } else { ProfileCollection(workbench=workbench,collection=collection) }
                    }
                }
                text(class="warnings",a11y:role=Role::Alert) {{move ||ui.error.get()}}
                }
                }
                DialogFooter {
                    ActionButton(class="preset-save",disabled=Signal::derive_local(move ||ui.busy.get()||ui.name.with(|name|name.trim().is_empty())||ui.path.with(String::is_empty)),on:click=move |_|workbench.save_authoring()) {{move ||if ui.busy.get(){"Saving…"}else{"Save"}}}
                    ActionButton(class="preset-cancel",disabled=Signal::derive_local(move ||ui.busy.get()),on:click=move |_|ui.open.set(false)) {"Cancel"}
                }
            }
        }
    }
}
#[component]
fn ProfileCollection(workbench: Workbench, collection: &'static str) -> impl IntoView {
    let ui = workbench.authoring;
    let expanded = RwSignal::new_local(false);
    let count = Memo::new(move |_| {
        ui.session.with(|session| {
            session
                .as_ref()
                .and_then(|session| session.bundle.document[collection].as_array())
                .map_or(0, Vec::len)
        })
    });
    view! {
        column(class="profile-builder-section") {
            if move ||count.get()>0 {
            ActionButton(class="profile-builder-toggle",on:click=move |_|expanded.update(|open|*open = !*open)) {{move ||format!("{} {} ({})",if expanded.get(){"v"}else{">"},match collection{"signals"=>"Fields","filters"=>"Required filters","geometry"=>"Geometry","plot_presets"|"views"=>"Plots","workspace_presets"=>"Workspaces",_=>"Views"},count.get())}}
            for index in move ||if expanded.get(){(0..count.get()).collect::<Vec<_>>()}else{Vec::new()},key=|index|*index {
                column(class="profile-builder-entry") {
                    row {
                        Input(a11y:label=format!("{} name",collection),value=Binding::controlled(Signal::derive_local(move ||entry_name(ui,collection,index)),move |name:String|set_entry_name(ui,collection,index,name)))
                        ActionButton(class="profile-entry-remove",disabled=Signal::derive_local(move ||ui.session.with(|session|session.as_ref().is_none_or(|session| referenced(&session.bundle.document,collection,index)))),on:click=move |_|ui.session.update(|session|if let Some(session)=session {session.bundle.document[collection].as_array_mut().unwrap().remove(index);})) {"Remove"}
                    }
                    if move ||matches!(collection,"signals"|"filters") {
                        row {for attribute in move ||["axis","unit"],key=|attribute|*attribute {
                            Input(placeholder=if attribute=="axis"{"Component (optional)"}else{"Unit (optional)"},a11y:label=if attribute=="axis"{"Optional field tree component label"}else{"Field unit label"},value=Binding::controlled(Signal::derive_local(move ||ui.session.with(|session|session.as_ref().and_then(|session|session.bundle.document[collection][index]["metadata"][attribute].as_str()).unwrap_or_default().to_owned())),move |value:String|ui.session.update(|session|if let Some(session)=session {session.bundle.document[collection][index]["metadata"][attribute]=serde_json::json!(value);})))
                        }}
                    }
                    text(class="muted") {{move ||ui.session.with(|session|session.as_ref().map(|session|{let entry=&session.bundle.document[collection][index];match collection{"signals"=>format!("Source: {}",entry["source"]),"filters"=>format!("{} · inputs {}",entry["operation"]["kind"],entry["inputs"]),"geometry"=>format!("{}",entry["shape"]["kind"]),"views"=>format!("{} fields",entry["fields"].as_array().map_or(0,Vec::len)),"plot_presets"=>format!("{} fields",entry["lines"].as_array().map_or(0,Vec::len)),_=>format!("{} plots",entry["plots"].as_array().map_or(0,Vec::len))}}).unwrap_or_default())}}
                    RecipePreview(workbench=workbench,collection=collection,index=index)
                    if move ||collection=="workspace_presets" {
                        for view_index in move ||ui.session.with(|session|session.as_ref().and_then(|session|session.bundle.document[collection][index]["views"].as_array()).map(|views|(0..views.len()).collect::<Vec<_>>()).unwrap_or_default()),key=|index|*index {
                            column(class="profile-recipe-preview") {
                                Input(a11y:label="View name in workspace",value=Binding::controlled(Signal::derive_local(move ||ui.session.with(|session|session.as_ref().and_then(|session|session.bundle.document[collection][index]["views"][view_index]["name"].as_str()).unwrap_or_default().to_owned())),move |name:String|ui.session.update(|session|if let Some(session)=session {session.bundle.document[collection][index]["views"][view_index]["name"]=serde_json::json!(name);})))
                                text(class="muted") {{move ||ui.session.with(|session|session.as_ref().map(|session|format!("{} plots",session.bundle.document[collection][index]["views"][view_index]["plots"].as_array().map_or(0,Vec::len))).unwrap_or_default())}}
                            }
                        }
                    }
                    if move ||collection=="filters" {
                        for parameter in move ||ui.session.with(|session|session.as_ref().and_then(|session|session.bundle.document["filters"][index]["operation"].as_object()).map(|operation|operation.iter().filter(|(key,value)|key.as_str()!="kind"&&(value.is_number()||value.is_string())).map(|(key,_)|key.clone()).collect::<Vec<_>>()).unwrap_or_default()),key=|parameter|parameter.clone() {
                            {{let label=parameter.clone();let read=parameter.clone();let write=parameter.clone();view!{row {text(class="muted") {{parameter}} Input(a11y:label=label,value=Binding::controlled(Signal::derive_local(move ||ui.session.with(|session|session.as_ref().map(|session|{let value=&session.bundle.document["filters"][index]["operation"][&read];value.as_str().map(str::to_owned).unwrap_or_else(||value.to_string())}).unwrap_or_default())),move |text:String|ui.session.update(|session|if let Some(session)=session {let current=&mut session.bundle.document["filters"][index]["operation"][&write];if current.is_number(){if let Ok(value)=serde_json::from_str::<serde_json::Value>(&text)&&value.is_number(){*current=value;}}else{*current=serde_json::json!(text);}})))}}}}
                        }
                    }
                }
            }
            }
        }
    }
}
fn referenced(document: &serde_json::Value, collection: &str, index: usize) -> bool {
    if !matches!(collection, "signals" | "filters" | "geometry") {
        return false;
    }
    let Some(id) = document[collection][index]["id"].as_str() else {
        return true;
    };
    let token = format!("${id}");
    fn contains(value: &serde_json::Value, id: &str, token: &str) -> bool {
        match value {
            serde_json::Value::String(value) => value == token || value == id,
            serde_json::Value::Array(values) => {
                values.iter().any(|value| contains(value, id, token))
            }
            serde_json::Value::Object(values) => values
                .iter()
                .any(|(key, value)| key != "id" && contains(value, id, token)),
            _ => false,
        }
    }
    for key in [
        "signals",
        "filters",
        "geometry",
        "views",
        "plot_presets",
        "view_presets",
        "workspace_presets",
    ] {
        if let Some(entries) = document[key].as_array() {
            for (entry_index, entry) in entries.iter().enumerate() {
                if key == collection && entry_index == index {
                    continue;
                }
                if contains(entry, id, &token) {
                    return true;
                }
            }
        }
    }
    false
}

fn primary(session: &crate::preset_ui::SaveSession, collection: &str, index: usize) -> bool {
    index == 0
        && matches!(
            (&session.kind, collection),
            (
                Some(crate::preset_capture::CaptureKind::Plot(_)),
                "plot_presets"
            ) | (
                Some(crate::preset_capture::CaptureKind::View),
                "view_presets"
            ) | (
                Some(crate::preset_capture::CaptureKind::Workspace),
                "workspace_presets"
            )
        )
}
fn entry_name(ui: crate::preset_ui::Authoring, collection: &str, index: usize) -> String {
    ui.session.with(|session| {
        session.as_ref().map_or_else(String::new, |session| {
            if primary(session, collection, index) {
                return ui.name.get();
            }
            let entry = &session.bundle.document[collection][index];
            let name = if matches!(collection, "signals" | "filters") {
                &entry["metadata"]["name"]
            } else {
                &entry["name"]
            };
            name.as_str().unwrap_or_default().into()
        })
    })
}
fn set_entry_name(ui: crate::preset_ui::Authoring, collection: &str, index: usize, name: String) {
    if ui.session.with_untracked(|session| {
        session
            .as_ref()
            .is_some_and(|session| primary(session, collection, index))
    }) {
        ui.name.set(name.clone());
    }
    ui.session.update(|session| {
        if let Some(session) = session {
            let entry = &mut session.bundle.document[collection][index];
            if matches!(collection, "signals" | "filters") {
                entry["metadata"]["name"] = serde_json::json!(name);
            } else {
                entry["name"] = serde_json::json!(name);
            }
        }
    });
}
#[component]
fn RecipePreview(
    workbench: Workbench,
    collection: &'static str,
    index: usize,
    #[prop(default = false)] always_open: bool,
) -> impl IntoView {
    let details = RwSignal::new_local(false);
    let has_plots = Memo::new(move |_| {
        workbench.authoring.session.with(|session| {
            session.as_ref().is_some_and(|session| {
                let entry = &session.bundle.document[collection][index];
                match collection {
                    "views" | "plot_presets" => entry.is_object(),
                    "view_presets" => entry["plots"]
                        .as_array()
                        .is_some_and(|plots| !plots.is_empty()),
                    _ => false,
                }
            })
        })
    });
    let plots = Memo::new(move |_| {
        workbench.authoring.session.with(|session| {
            session
                .as_ref()
                .map(|session| {
                    let entry = &session.bundle.document[collection][index];
                    if collection == "views" {
                        let mut plot = entry.clone();
                        plot["lines"] = entry["fields"]
                            .as_array()
                            .map(|fields| {
                                fields
                                    .iter()
                                    .map(|source| serde_json::json!({"source":source}))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                            .into();
                        vec![plot]
                    } else if collection == "plot_presets" {
                        vec![entry.clone()]
                    } else if collection == "view_presets" {
                        entry["plots"].as_array().cloned().unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                })
                .unwrap_or_default()
        })
    });
    view! {if move ||has_plots.get() {
        if move || !always_open { ActionButton(on:click=move |_|details.update(|open|*open = !*open)) {{move ||if details.get(){"Hide recipe"}else{"Show fields and layout"}}} }
        if move ||always_open || details.get() {
            for plot in move ||plots.get().into_iter().enumerate().collect::<Vec<_>>(),key=|plot|plot.0 {
                {{let plot_index=plot.0;let plot=Signal::derive_local(move ||plots.with(|plots|plots.get(plot_index).cloned().unwrap_or_default()));view!{column(class="profile-recipe-preview") {
                    if move ||collection=="view_presets" {
                        Input(a11y:label="Plot name in view preset",value=Binding::controlled(Signal::derive_local(move ||plot.with(|plot|plot["name"].as_str().unwrap_or_default().to_owned())),move |name:String|workbench.authoring.session.update(|session|if let Some(session)=session {session.bundle.document[collection][index]["plots"][plot_index]["name"]=serde_json::json!(name);})))
                    } else {
                        text(class="heading") {{move ||plot.with(|plot|plot["name"].as_str().unwrap_or_default().to_owned())}}
                    }
                    text(class="muted") {{move ||plot.with(|plot|if plot["analysis"].is_object(){format!("{} / {} / remove DC: {}",plot["analysis"]["mode"],plot["analysis"]["window"],plot["analysis"]["remove_dc"])}else{"Time series".into()})}}
                    for source in move ||plot.with(|plot|plot["lines"].as_array().map(|lines|lines.iter().filter_map(|line|line["source"].as_str().map(str::to_owned)).collect::<Vec<_>>()).unwrap_or_default()),key=|source|source.clone() {
                        text(class="muted") {{move ||workbench.authoring.session.with(|session|session.as_ref().map(|session| {
                            let id=source.trim_start_matches('$');
                            ["signals","filters"].iter().flat_map(|collection|session.bundle.document[*collection].as_array().into_iter().flatten()).find(|entry|entry["id"]==id).and_then(|entry|entry["metadata"]["name"].as_str()).unwrap_or(&source).to_owned()
                        }).unwrap_or_default())}}
                    }
                }}}}
            }
            if move ||collection=="view_presets" {text(class="muted") {{move ||workbench.authoring.session.with(|session|session.as_ref().map(|session|layout_description(&session.bundle.document[collection][index]["layout"])).unwrap_or_default())}}}
        }
    }}
}
fn layout_description(layout: &serde_json::Value) -> String {
    match layout["kind"].as_str() {
        Some("plot") => format!("Plot {}", layout["plot"].as_u64().unwrap_or(0) + 1),
        Some("tabs") => format!(
            "Tabs: {} plots",
            layout["plots"].as_array().map_or(0, Vec::len)
        ),
        Some("split") => format!(
            "{} [{} | {}]",
            if layout["stacked"] == true {
                "Rows"
            } else {
                "Columns"
            },
            layout_description(&layout["first"]),
            layout_description(&layout["second"])
        ),
        _ => String::new(),
    }
}
