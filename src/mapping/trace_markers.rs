use super::{layer_option::LayerOptionProps, settings::target};
use crate::{
    state::{Pane, Workbench},
    trace_color,
};
use plot::zgui_series::ColorPickerProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn TraceMarkers(
    workbench: Workbench,
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
) -> impl IntoView {
    let pane = target(workbench, pane);
    let style = Memo::new(move |_| trace_color::current(pane, selected.get()));
    let color = Signal::derive_local(move || {
        style.get().arrow_color.unwrap_or_else(|| {
            pane.trajectory.with(|s| {
                s.as_ref()
                    .map_or_else(crate::trajectory::arrow_color, |s| s.arrow_color)
            })
        })
    });
    view! {column(class="map-trace-markers") {
        Popover {
            PopoverTrigger(class="map-arrow-color",size=ButtonSize::Xs,variant=ButtonVariant::Outline,a11y:label="Selected line direction arrow color") {
                box(class="state-color-key-swatch",style:background=move ||{let [r,g,b,_]=color.get();Some(crate::plot_appearance::css([r,g,b]))}) {}
                text {"Arrow color"}
            }
            PopoverContent(class="trace-gradient-color-popup map-arrow-color-popup") {
                ColorPicker(color=color,on_change=UnsyncCallback::new(move |color:[u8;4]|{let _=trace_color::change(pane,selected.get_untracked(),|style|style.arrow_color=Some(color));}))
            }
        }
        LayerOption(class="map-trace-states map-layer-option",label="State markers",checked=Signal::derive_local(move ||style.get().state_markers),on_change=UnsyncCallback::new(move |value|{let _=trace_color::change(pane,selected.get_untracked(),|style|style.state_markers=value);}))
    }}
}
