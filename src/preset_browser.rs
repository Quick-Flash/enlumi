use crate::{action_button::ActionButtonProps, preset_ui::ApplyAction, state::Workbench};
use std::path::PathBuf;
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[derive(Clone, Copy)]
pub enum BrowseTarget {
    Plot,
    Fields(u64),
}
#[derive(Clone)]
pub struct BrowseRequest {
    pub target: BrowseTarget,
    pub view: u64,
    pub filter: Option<(PathBuf, String)>,
}
impl Workbench {
    pub fn open_preset_browser(self, target: BrowseTarget, filter: Option<(PathBuf, String)>) {
        if let Some((path, _)) = &filter
            && !self
                .profiles
                .instances
                .with_untracked(|instances| instances.keys().any(|key| &key.path == path))
        {
            self.apply_profile(path.clone());
        }
        self.authoring.browse.set(Some(BrowseRequest {
            target,
            view: self.bind_view().view().id,
            filter,
        }));
    }
}
#[component]
pub fn PresetBrowser(workbench: Workbench) -> impl IntoView {
    let ui = workbench.authoring;
    view! {
        Dialog(open=Binding::controlled(Signal::derive_local(move ||ui.browse.with(Option::is_some)),move |open:bool|if !open{ui.browse.set(None)})) {
            DialogContent(class="profile-builder") {
                DialogHeader {DialogTitle {"Choose preset"} DialogDescription {"Open saved plots and layouts, or select fields to add."}}
                ActionButton(class="preset-load-file",on:click=move |_|workbench.task(async move {let path=blocking(||rfd::FileDialog::new().add_filter("Enlumi profile",&["json"]).pick_file()).await;if let Some(path)=path{workbench.apply_profile(path);}})) {"Load profile file…"}
                ScrollArea(class="profile-builder-body",label="Available presets") {
                    column(class="profile-builder-form") {
                    for instance in move ||workbench.profiles.instances.with(|instances|instances.values().cloned().collect::<Vec<_>>()),key=|instance|(instance.key.clone(),crate::preset_capture::hash(&serde_json::to_vec(&instance.definition).unwrap_or_default())) {
                        {{let instance=StoredValue::new_local(instance);view!{column {
                            text(class="heading") {{instance.with_value(|instance|instance.name.clone())}}
                            for recipe in move ||instance.with_value(crate::preset_apply::available_plots),key=|recipe|recipe.name.clone() {
                                PresetChoice(workbench=workbench,instance=instance,recipe=recipe)
                            }
                        }}}}
                    }
                    }
                }
                text(class="muted") {{move ||if workbench.profiles.busy.get(){"Preparing profile…"}else{"Choose individual fields below a plot preset to add only selected inputs."}}}
                DialogFooter {ActionButton(on:click=move |_|ui.browse.set(None)) {"Close"}}
            }
        }
    }
}
#[component]
fn PresetChoice(
    workbench: Workbench,
    instance: StoredValue<crate::profile_instance::AppliedProfile, LocalStorage>,
    recipe: crate::preset::PlotPreset,
) -> impl IntoView {
    let ui = workbench.authoring;
    let recipe = StoredValue::new_local(recipe);
    let selected = RwSignal::new_local(recipe.with_value(|recipe| {
        recipe
            .lines
            .iter()
            .map(|line| line.source.clone())
            .collect::<std::collections::BTreeSet<_>>()
    }));
    let visible = Signal::derive_local(move || {
        ui.browse.with(|request| {
            request.as_ref().is_some_and(|request| {
                request.filter.as_ref().is_none_or(|(path, name)| {
                    instance.with_value(|instance| &instance.key.path == path)
                        && recipe.with_value(|recipe| &recipe.name == name)
                })
            })
        })
    });
    view! {
        if move ||visible.get() {
            column(class="profile-builder-entry") {
                text(class="heading") {{recipe.with_value(|recipe|recipe.name.clone())}}
                if move ||ui.browse.with(|request|request.as_ref().is_some_and(|request|matches!(request.target,BrowseTarget::Fields(_)))) {
                    for token in move ||recipe.with_value(|recipe|recipe.lines.iter().map(|line|line.source.clone()).collect::<Vec<_>>()),key=|token|token.clone() {
                        {{let checked=token.clone();let value=token.clone();let label=instance.with_value(|instance|instance.resolve(&token)).map(|source|workbench.field_name_untracked(&source)).unwrap_or(token);let node=NodeRef::new();view!{row {
                            Checkbox(node_ref=node,checked=Binding::controlled(Signal::derive_local(move ||if selected.with(|selected|selected.contains(&checked)){Checked::Yes}else{Checked::No}),move |checked|selected.update(|selected|if checked==Checked::Yes{selected.insert(value.clone());}else{selected.remove(&value);})))
                            Label(control=node) {{label}}
                        }}}}
                    }
                }
                ActionButton(class="preset-use-choice",disabled=Signal::derive_local(move ||ui.browse.with(|request|request.as_ref().is_some_and(|request|matches!(request.target,BrowseTarget::Fields(_))))&&selected.with(|selected|selected.is_empty())),on:click=move |_|{
                    let Some(request)=ui.browse.get_untracked()else{return;};let Some(target)=workbench.for_view(request.view)else{ui.browse.set(None);return;};
                    let key=instance.with_value(|instance|instance.key.clone());let mut recipe=recipe.get_value();
                    if let BrowseTarget::Fields(id)=request.target{recipe.lines.retain(|line|selected.with_untracked(|selected|selected.contains(&line.source)));if let Err(error)=target.apply_plot_preset(&key,&recipe,Some(id)){workbench.status.set(error);return;}}
                    else {target.use_saved_preset(key,recipe.name,ApplyAction::Plot);}
                    ui.browse.set(None);
                }) {{move ||if ui.browse.with(|request|request.as_ref().is_some_and(|request|matches!(request.target,BrowseTarget::Fields(_)))){"Add selected fields"}else{"Add as new plot"}}}
            }
        }
    }
}
