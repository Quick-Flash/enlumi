use crate::band_mapping::BandMappingProps;
use crate::geometry_appearance::GeometryAppearanceProps;
use crate::geometry_coordinates::GeometryCoordinatesProps;
use crate::{
    action_button::ActionButtonProps,
    geometry::{Geometry, GeometryKind, LineOrientation, Shade},
    state::Workbench,
};
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui::prelude::*;
pub fn change(workbench: Workbench, id: u64, edit: impl FnOnce(&mut Geometry)) {
    if let Some(mut item) = workbench.geometry_item(id) {
        edit(&mut item);
        workbench.update_geometry(item, false);
    }
}
#[component]
pub fn GeometryInspector(workbench: Workbench, id: u64, boundary: NodeRef) -> impl IntoView {
    let item = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id)
    };
    let appearance = RwSignal::new_local(false);
    let collapsed = RwSignal::new_local(false);
    let editor = NodeRef::new();
    let offset = RwSignal::new_local((0.0_f64, 0.0_f64));
    let area_size = boundary.observe_border_box();
    let editor_size = editor.observe_border_box();
    let keep_visible = RenderEffect::new(move |_| {
        area_size.track();
        editor_size.track();
        if let (Some(area), Some(bounds)) = (boundary.window_bounds(), editor.window_bounds()) {
            let scale = f64::from(boundary.scale());
            let old = offset.get_untracked();
            let x = bounds.origin.x.0;
            let y = bounds.origin.y.0;
            let right =
                (area.origin.x.0 + area.size.width.0 - bounds.size.width.0).max(area.origin.x.0);
            let bottom =
                (area.origin.y.0 + area.size.height.0 - bounds.size.height.0).max(area.origin.y.0);
            let next = (
                old.0 + f64::from(x.clamp(area.origin.x.0, right) - x) / scale,
                old.1 + f64::from(y.clamp(area.origin.y.0, bottom) - y) / scale,
            );
            if (next.0 - old.0).abs() > 0.5 || (next.1 - old.1).abs() > 0.5 {
                offset.set(next);
            }
        }
    });
    on_cleanup_local(move || drop(keep_visible));
    on_cleanup_local(move || {
        if workbench
            .geometry
            .editor_drag
            .get_untracked()
            .is_some_and(|drag| drag.editor == editor)
        {
            workbench.finish_geometry_editor(false);
        }
    });
    let vertical = move || item().is_some_and(|item| item.coordinates[0] == item.coordinates[2]);
    view! {column(class="geometry-inspector",node_ref=editor,style:transform=move || {let (x,y)=offset.get();Some(format!("translate({x}px, {y}px)"))},a11y:label="Live geometry editor",
        on:pointer_down=move |event| {if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.editor==editor){event.capture_pointer();event.prevent_default();}event.stop_propagation();},
        on:pointer_move=move |event| {if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.editor==editor){workbench.move_geometry_editor(workbench.active().get_untracked(),event.position);event.stop_propagation();}},
        on:pointer_up=move |event| {if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.editor==editor){workbench.finish_geometry_editor(false);event.release_pointer();event.stop_propagation();}},
        on:pointer_cancel=move |event| {if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.editor==editor){workbench.finish_geometry_editor(true);event.release_pointer();event.stop_propagation();}}) {
        row(class="geometry-editor-heading",a11y:label="Drag geometry editor title to move window",
            on:pointer_down=move |event| {if event.button==Some(zgui::vocab::PointerButton::Primary){workbench.begin_geometry_editor(editor,boundary,offset,event.position);event.prevent_default();}}) {
            text(class="geometry-title-mark",style:background=move || item().map(|v|format!("#{:02x}{:02x}{:02x}",v.color[0],v.color[1],v.color[2]))) {""}
            column {text(class="heading") {{move || item().map_or("Geometry",|v|if v.states.is_some(){"State bands"}else{v.kind.name()})}} text(class="muted") {"Editing shared geometry"}}
            ActionButton(icon=true,class="geometry-editor-collapse",a11y:label=move || zgui::vocab::SharedString::from(if collapsed.get(){"Expand geometry editor"}else{"Collapse geometry editor"}),on:pointer_down=move |event|event.stop_propagation(),on:click=move |_|collapsed.update(|value|*value = !*value)) {
                vector(class="geometry-collapse-icon",prop:viewBox="0 0 16 16",prop:d=move || PropValue::from(if collapsed.get(){"M3 6 L8 11 L13 6"}else{"M3 10 L8 5 L13 10"}),a11y:hidden=true)
            }
            ActionButton(class="geometry-edit-toggle",on:pointer_down=move |event|event.stop_propagation(),on:click=move |_|workbench.finish_geometry_editing()) {"Done"}
        }
        if move || !collapsed.get() {
        column(class="geometry-name-field") {
            text(class="geometry-section-label muted") {"Name"}
            Input(class="geometry-name-input",value=Binding::controlled(Signal::derive_local(move || item().map_or_else(String::new,|item|item.name)),move |name:String|change(workbench,id,|item|item.name=name)),a11y:label="Geometry name")
        }
        row(class="geometry-editor-tabs") {
            ActionButton(class="geometry-coordinates-tab",class:setting-active=move || !appearance.get(),on:click=move |_|appearance.set(false)) {{move ||if item().is_some_and(|item|item.states.is_some()){"Mapping"}else{"Coordinates"}}}
            ActionButton(class="geometry-appearance-tab",class:setting-active=move || appearance.get(),on:click=move |_|appearance.set(true)) {"Appearance"}
        }
        column(class="geometry-editor-body") {
        if move || appearance.get() {
            GeometryAppearance(workbench=workbench,id=id)
        } else {
            if move || item().is_some_and(|v|v.kind==GeometryKind::Line) {
                row(class="geometry-line-mode") {text(class="geometry-option-name muted") {"Direction"}
                    for choice in move || [("Free",LineOrientation::Free),("Horizontal",LineOrientation::Horizontal),("Vertical",LineOrientation::Vertical)],key=|choice|choice.0 {
                        ActionButton(class="geometry-orientation",class:geometry-horizontal=move || choice.1==LineOrientation::Horizontal,class:geometry-vertical=move || choice.1==LineOrientation::Vertical,class:setting-active=move || item().is_some_and(|item|item.orientation==choice.1),on:click=move |_|workbench.orient_geometry(id,choice.1)) {{choice.0}}
                    }
                }
            }
            if move ||item().is_some_and(|item|item.states.is_some()) {BandMapping(workbench=workbench,id=id)} else {GeometryCoordinates(workbench=workbench,id=id)}
            if move || item().is_some_and(|v|v.kind==GeometryKind::Line) {
                row(class="geometry-line-extent") {
                    text(class="geometry-option-name muted") {"Extent"}
                    ActionButton(class:setting-active=move || item().is_some_and(|v|!v.extend),on:click=move |_|change(workbench,id,|v|v.extend=false)) {"Segment"}
                    ActionButton(class:setting-active=move || item().is_some_and(|v|v.extend),on:click=move |_|change(workbench,id,|v|v.extend=true)) {"Across plot"}
                }
                row(class="geometry-shade") {
                    text(class="geometry-option-name muted") {"Shade"}
                    for shade in move || [Shade::None,Shade::Above,Shade::Below],key=|shade|*shade as u8 {
                        ActionButton(class="geometry-shade-option",class:setting-active=move || item().is_some_and(|v|v.shade==shade),disabled=Signal::derive_local(move || shade!=Shade::None && vertical()),on:click=move |_|change(workbench,id,|v|v.shade=shade)) {{match shade {Shade::None=>"None",Shade::Above=>"Above",Shade::Below=>"Below"}}}
                    }
                }
                text(class="geometry-help muted") {{move || if vertical(){"Vertical line: above/below shading unavailable."}else{"Above/below uses data Y, independent of endpoint order."}}}
            }
            if move ||item().is_some_and(|item|item.states.is_none()) {text(class="geometry-help muted") {"Drag arrow handle to move; squares resize. Drag background to pan, scroll to zoom."}}
        }
        }
        if move || !workbench.geometry.error.with(String::is_empty) {text(class="warnings") {{move || workbench.geometry.error.get()}}}
        row(class="geometry-editor-footer") {
            ActionButton(class="duplicate-geometry",on:click=move |_|workbench.duplicate_geometry(id)) {"Duplicate"}
            text(class="geometry-placement-hint muted") {{move || {let count=workbench.panes().with(|panes|panes.iter().filter(|pane|pane.geometry.with(|ids|ids.contains(&id))).count());if count==0 {"Drag onto a plot to place".into()}else{format!("Shared on {count} plots")}}}}
            ActionButton(class="delete-geometry",a11y:label="Delete saved geometry and all plot copies",on:click=move |_|workbench.delete_geometry(id)) {"Delete saved"}
        }
        }
    }}
}
