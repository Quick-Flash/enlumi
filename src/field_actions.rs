use crate::{profile::Metadata, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

impl Workbench {
    pub fn editable_source(self, source: &str) -> Option<String> {
        let source = if let Some(drag) = crate::profile::identity::Drag::parse(source) {
            let crate::profile::identity::Item::Field(name) = drag.item else {
                return None;
            };
            self.profiles.instances.with_untracked(|instances| {
                instances
                    .get(&drag.profile)
                    .and_then(|instance| instance.resolve(&name))
            })?
        } else {
            source.into()
        };
        (source.starts_with("@derived/") || !source.starts_with('@')).then_some(source)
    }
    pub fn set_field_metadata(self, source: String, metadata: Metadata) -> bool {
        if metadata.name.trim().is_empty()
            || metadata.name.len() > 256
            || metadata.axis.len() > 64
            || metadata.unit.len() > 64
        {
            self.status
                .set("Use name up to 256 bytes; component and unit up to 64 bytes".into());
            return false;
        }
        self.profiles.overrides.update(|overrides| {
            overrides.insert(source.clone(), Some(metadata));
        });
        self.rebuild_profile_metadata();
        for pane in self.all_panes() {
            pane.lines.update(|lines| {
                for line in lines {
                    if line.source == source {
                        line.name = self.field_name_untracked(&source);
                    }
                }
            });
        }
        true
    }
    pub fn save_field_to_profile(
        self,
        path: std::path::PathBuf,
        source: String,
        metadata: Metadata,
    ) {
        let token = self.profiles.instances.with_untracked(|instances| {
            instances
                .values()
                .find(|instance| instance.key.path == path)
                .and_then(|instance| {
                    instance
                        .symbols
                        .iter()
                        .find(|(_, value)| **value == source)
                        .map(|(id, _)| id.clone())
                })
        });
        self.task(async move {let target=path.clone();let field=source.clone();let result=blocking(move || {
            let previous=std::fs::read(&target).map_err(|e|e.to_string())?;let mut document:serde_json::Value=serde_json::from_slice(&previous).map_err(|e|e.to_string())?;
            let mut changed=false;for collection in ["signals","filters"]{if let Some(entries)=document[collection].as_array_mut(){for entry in entries {if token.as_ref().is_some_and(|id|entry["id"]==*id)||collection=="signals"&&entry["source"]==source{entry["metadata"]=serde_json::to_value(&metadata).map_err(|e|e.to_string())?;changed=true;}}}}
            if !changed{return Err("Field is not owned by this profile. Use Add to profile to include its dependencies.".into());}
            crate::preset_store::write_document(document,&target,Some(previous),Default::default(),Some(&target))
        }).await;
        match result{Ok(())=>{self.profiles.overrides.update(|overrides|{overrides.remove(&field);});self.authoring.field_edit.set(None);self.authoring.revision.update(|revision|*revision+=1);let keys=self.profiles.instances.with_untracked(|instances|instances.keys().filter(|key|key.path==path).cloned().collect::<Vec<_>>());if let Some(key)=keys.first(){self.reload_profile(key.clone());}self.status.set("Profile labels saved".into());},Err(error)=>self.status.set(error)}
        });
    }
}
#[component]
pub fn FieldActions(
    workbench: Workbench,
    source: Signal<Option<String>, LocalStorage>,
    children: ChildrenFn,
    #[prop(optional)] on_double_click: Option<UnsyncCallback<()>>,
) -> impl IntoView {
    let clicks = StoredValue::new_local(Clicks::default());
    let open = RwSignal::new_local(false);
    let rename = RwSignal::new_local(false);
    let name = RwSignal::new_local(String::new());
    let editing = StoredValue::new_local(String::new());
    let input = NodeRef::new();
    let resolved = Signal::derive_local(move || {
        source
            .get()
            .and_then(|source| workbench.editable_source(&source))
    });
    let row_sync = zgui::reactive::RenderEffect::new(move |_| {
        let source = resolved.get();
        if source.as_deref() != Some(editing.get_value().as_str()) {
            rename.set(false);
        }
    });
    on_cleanup_local(move || drop(row_sync));
    let begin = move || {
        if let Some(source) = resolved.get_untracked() {
            let metadata = workbench
                .profiles
                .metadata
                .with_untracked(|metadata| metadata.get(&source).cloned());
            name.set(metadata.map_or_else(
                || workbench.field_name_untracked(&source),
                |metadata| metadata.name,
            ));
            editing.set_value(source);
            rename.set(true);
        }
    };
    let save = move || {
        let source = editing.get_value();
        let mut metadata = workbench
            .profiles
            .metadata
            .with_untracked(|metadata| metadata.get(&source).cloned())
            .unwrap_or_default();
        metadata.name = name.get_untracked().trim().into();
        if workbench.set_field_metadata(source, metadata) {
            rename.set(false);
        }
    };
    let bounds = input.observe_border_box();
    let focus = zgui::reactive::RenderEffect::new(move |_| {
        if rename.get() && bounds.get().is_some() {
            input.focus();
            input.select_all();
        }
    });
    on_cleanup_local(move || drop(focus));
    view! {
        ContextMenu(open=open) {
            ContextMenuTrigger(class="field-actions",on:pointer_up:capture=move |event| {
                if rename.get_untracked() || event.button!=Some(zgui::vocab::PointerButton::Primary) || !event.modifiers.is_empty() {return;}
                if workbench.drag.state.get_untracked().is_some_and(|drag|drag.3) {clicks.set_value(Clicks::default());return;}
                let Some(source)=source.get_untracked()else{return;};
                let mut double=false;clicks.update_value(|clicks|double=clicks.note(&source,event.position.x.0,event.position.y.0));
                if double {if resolved.get_untracked().is_some(){begin();}else if let Some(action)=on_double_click {action.run(());}}
            }) {
                if move ||rename.get() {
                    Input(node_ref=input,value=name,class="field-inline-rename",a11y:label="Field display name",on:focus_out=move |_|if rename.get_untracked(){save();},on:key_down=move |event|{
                        if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){save();event.prevent_default();event.stop_propagation();}
                        if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape){rename.set(false);event.prevent_default();event.stop_propagation();}
                    })
                } else {{children.view()}}
            }
            ContextMenuContent(class="field-context-menu") {
                MenuItem(class="field-rename",disabled=Signal::derive_local(move ||resolved.get().is_none()),on_select=UnsyncCallback::new(move |()|begin())) {"Rename"}
                MenuItem(class="field-edit-labels",disabled=Signal::derive_local(move ||resolved.get().is_none()),on_select=UnsyncCallback::new(move |()|if let Some(source)=resolved.get_untracked(){workbench.selected_field.set(Some(source.clone()));workbench.authoring.field_edit.set(Some(source));})) {"Edit labels…"}
                MenuItem(class="field-edit-filter",disabled=Signal::derive_local(move ||resolved.get().and_then(|source|crate::derived::id(&source)).is_none()),on_select=UnsyncCallback::new(move |()|if let Some(id)=resolved.get_untracked().and_then(|source|crate::derived::id(&source)){workbench.edit_filter(Some(id),None);})) {"Edit filter"}
                MenuItem(class="field-add-profile",disabled=Signal::derive_local(move ||resolved.get().is_none()),on_select=UnsyncCallback::new(move |()|if let Some(source)=resolved.get_untracked(){workbench.begin_save(crate::preset_capture::CaptureKind::Selection(vec![source]));})) {"Add to profile…"}
                MenuItem(class="field-copy-source",disabled=Signal::derive_local(move ||resolved.get().is_none()),on_select=UnsyncCallback::new(move |()|if let Some(source)=resolved.get_untracked() && let Some(clipboard)=try_use_clipboard(){clipboard.set_text(ClipboardKind::Standard,source);})) {"Copy source path"}
            }
        }
    }
}

#[derive(Default)]
pub struct Clicks {
    last: Option<(String, std::time::Instant, (f32, f32))>,
}
impl Clicks {
    pub fn note(&mut self, source: &str, x: f32, y: f32) -> bool {
        let now = std::time::Instant::now();
        let double = self.last.as_ref().is_some_and(|(key, time, point)| {
            key == source
                && now.duration_since(*time) < std::time::Duration::from_millis(500)
                && (point.0 - x).hypot(point.1 - y) < 5.
        });
        self.last = if double {
            None
        } else {
            Some((source.into(), now, (x, y)))
        };
        double
    }
}
