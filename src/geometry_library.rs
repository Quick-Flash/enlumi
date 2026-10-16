use crate::{action_button::ActionButtonProps, geometry::GeometryKind, state::Workbench};
use zgui::prelude::*;
#[component]
pub fn GeometryLibrary(workbench: Workbench) -> impl IntoView {
    view! { column(class="geometry-library",class:band-source-open=move ||workbench.bands.open.get()) {
        row(class="geometry-heading") {text(class="heading") {"Geometry"}
            ActionButton(class="open-state-bands",disabled=Signal::derive_local(move ||workbench.loading.get()),on:click=move |_|{workbench.finish_geometry_editing();workbench.filters_open.set(false);workbench.bands.open.set(true);workbench.bands.error.set(String::new());}) {"State bands"}
        }
        row(class="geometry-create-tools") {
            ActionButton(class="draw-rectangle",class:setting-active=move || workbench.geometry.tool.get()==Some(GeometryKind::Rectangle),on:click=move |_| workbench.arm_geometry(GeometryKind::Rectangle)) {"Rectangle"}
            ActionButton(class="draw-line",class:setting-active=move || workbench.geometry.tool.get()==Some(GeometryKind::Line),on:click=move |_| workbench.arm_geometry(GeometryKind::Line)) {"Line"}
            ActionButton(class="draw-time-band",class:setting-active=move ||workbench.geometry.tool.get()==Some(GeometryKind::Band),on:click=move |_|workbench.arm_geometry(GeometryKind::Band)) {"Time band"}
        }
        if move || workbench.bands.open.get() {
            column(class="band-source-drop",node_ref=workbench.bands.drop,class:band-source-hover=move ||workbench.bands.hover.get()) {
                text(class="heading") {"State bands from field"}
                text(class="muted") {{move ||if workbench.bands.busy.get(){"Reading source..."}else{"Drop one discrete field here (e.g. nav_state)."}}}
                ActionButton(class="blank-state-mapping",on:click=move |_|workbench.new_state_mapping()) {"Blank mapping"}
                ActionButton(class="cancel-state-bands",on:click=move |_|workbench.cancel_band_source()) {"Cancel"}
                if move ||!workbench.bands.error.with(String::is_empty) {text(class="warnings") {{move ||workbench.bands.error.get()}}}
            }
        }
        if move || !workbench.bands.open.get() && workbench.geometry.items.with(Vec::is_empty) {text(class="muted") {"Draw bounds; drag saved shapes onto plots."}}
        column(class="geometry-rows") {
            for item in move || workbench.geometry.items.get(), key=|item| item.id {
                control(class="metric-option geometry-row",class:metric-option-selected=move || workbench.geometry.selected.get()==Some(item.id),tabindex=Focus::Sequential,
                    a11y:label=move || zgui::vocab::SharedString::from(workbench.geometry_item(item.id).map_or_else(String::new,|item|format!("{}: edit or drag onto plot",item.name))),
                    on:pointer_down=move |event| {if event.button==Some(zgui::vocab::PointerButton::Primary) {workbench.filters_open.set(false);workbench.geometry.library_drag.set(Some((item.id,kurbo::Point::new(f64::from(event.position.x.0),f64::from(event.position.y.0)),false)));event.capture_pointer();event.prevent_default();}},
                    on:pointer_move=move |event| workbench.geometry_library_move(event.position),
                    on:pointer_up=move |event| {if let Some((id,_,moving))=workbench.geometry.library_drag.get_untracked(){if moving {if let Some(pane)=workbench.geometry.target.get_untracked().and_then(|id| workbench.find(id)) {workbench.attach_geometry(id,pane,true);}} else {workbench.edit_geometry_library(id);}}workbench.geometry.library_drag.set(None);workbench.geometry.target.set(None);event.release_pointer();},
                    on:pointer_cancel=move |event| {workbench.cancel_geometry_drag();event.release_pointer();},
                    on:key_down=move |event| {if matches!(event.key,zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter | zgui::vocab::NamedKey::Space)){workbench.edit_geometry_library(item.id);event.prevent_default();}}) {
                    text(class="signal-name geometry-row-name") {{move || {workbench.geometry.items.track();workbench.geometry_item(item.id).map_or_else(String::new,|item|item.name)}}}
                    text(class="metric-option-count") {{if item.states.is_some(){"States"}else{item.kind.name()}}}

                }
            }
        }
    } }
}
