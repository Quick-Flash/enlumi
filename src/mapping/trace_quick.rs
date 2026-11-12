//! Per-trace color mode and framing, opened from the map legend row.
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trace_color::{self, Mode},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

const SWATCH: &str = "M8 2 A6 6 0 1 0 8 14 A6 6 0 1 0 8 2 Z M8 2 V14";

#[component]
pub fn TraceQuickSettings(workbench: Workbench, pane: Pane, id: u64) -> impl IntoView {
    let mode = Memo::new(move |_| trace_color::current(pane, Some(id)).mode);
    let field = Memo::new(move |_| {
        pane.trajectory.with(|s| {
            s.as_ref()
                .map(|s| s.color_source.clone())
                .unwrap_or_default()
        })
    });
    let set = move |next: Mode| {
        if let Err(error) = trace_color::change(pane, Some(id), |color| color.mode = next) {
            workbench.status.set(error);
        }
    };
    let option = move |next: Mode,
                       class: &'static str,
                       text: Signal<String, LocalStorage>,
                       disabled: Signal<bool, LocalStorage>| {
        view! {ActionButton(class=class,disabled=disabled,class:setting-active=move ||mode.get()==next,on:click=move |_|set(next)){{move ||text.get()}}}
    };
    let fixed = |text: &'static str| Signal::stored_local(text.to_owned());
    let never = Signal::stored_local(false);
    view! {Popover {
        PopoverTrigger(class="app-action trace-quick icon-action",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Trace color and framing") {
            vector(class="icon-action-glyph",prop:viewBox="0 0 16 16",prop:d=SWATCH,a11y:hidden=true)
        }
        PopoverContent(class="trace-quick-menu") {
            text(class="map-layers-heading") {"Color by"}
            row(class="trace-quick-modes") {
                {option(Mode::Solid,"trace-color-solid",fixed("Solid"),never)}
                {option(Mode::Speed,"trace-color-speed",fixed("Speed"),never)}
                {option(Mode::Height,"trace-color-height",fixed("Height"),never)}
                {option(Mode::Field,"trace-color-field",Signal::derive_local(move ||field.with(|f|f.rsplit('/').next().filter(|s|!s.is_empty()).unwrap_or("Field").to_owned())),Signal::derive_local(move ||field.with(String::is_empty)))}
            }
            if move ||field.with(String::is_empty) {
                text(class="muted") {"Drop a field on Color in Edit coordinates to color by it."}
            }
            row(class="trace-quick-actions") {
                ActionButton(class="trace-fit",on:click=move |_|{pane.fit_trace.set(Some(id));super::settings::update(workbench,pane,|s|if let Some(map)=s.map.as_mut(){map.fit();});}){"Fit to trace"}
                ActionButton(class="trace-edit",on:click=move |_|workbench.edit_trajectory(pane)){"Edit trace…"}
            }
        }
    }}
}
