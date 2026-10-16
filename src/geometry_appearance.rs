use crate::action_button::ActionButtonProps;
use crate::{geometry_inspector::change, state::Workbench};
use plot::zgui_series::ColorPickerProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn GeometryAppearance(workbench: Workbench, id: u64) -> impl IntoView {
    let item = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id)
    };
    view! {row(class="geometry-appearance") {
        if move || item().is_some_and(|item|item.states.is_none()) {
        ColorPicker(color=Signal::derive_local(move || item().map_or([235,174,82,255],|item|[item.color[0],item.color[1],item.color[2],255])),on_change=UnsyncCallback::new(move |color:[u8;4]|change(workbench,id,|item|item.color=[color[0],color[1],color[2]])))
        }
        column(class="geometry-style-controls") {
            text(class="heading") {{move || format!("Fill {:.0}%",item().map_or(0.18,|item|item.opacity)*100.)}}
            Slider(class="geometry-opacity",min=0.,max=1.,step=0.01,label="Geometry fill opacity",value=Binding::controlled(Signal::derive_local(move ||item().map_or(0.18,|item|item.opacity)),move |value|change(workbench,id,|item|item.opacity=value)))
            if move ||item().is_some_and(|item|item.kind!=crate::geometry::GeometryKind::Band) {
            text(class="heading") {{move ||format!("Stroke {:.1} px",item().map_or(2.,|item|item.width))}}
            Slider(class="geometry-width",min=0.5,max=10.,step=0.5,label="Geometry stroke width",value=Binding::controlled(Signal::derive_local(move ||item().map_or(2.,|item|item.width)),move |value|change(workbench,id,|item|item.width=value)))
            }
            if move ||item().is_some_and(|item|item.kind==crate::geometry::GeometryKind::Band) {ActionButton(class:setting-active=move ||item().is_some_and(|item|item.labels),on:click=move |_|change(workbench,id,|item|item.labels = !item.labels)) {{move ||if item().is_some_and(|item|item.labels){"Labels: on"}else{"Labels: off"}}}}
            text(class="geometry-help muted") {"Color and style update every copy. Legend toggles affect one plot."}
        }
    }}
}
