use super::{gradient_controls::GradientControlsProps, settings::target};
use crate::{
    state::{Pane, Workbench},
    trace_color::{self, Mode},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn ColorSettings(workbench: Workbench, pane: Pane) -> impl IntoView {
    let actual = target(workbench, pane);
    let selected = Signal::derive_local(move || trace_color::selected(actual));
    let style = Memo::new(move |_| trace_color::current(actual, selected.get()));
    let mode = Memo::new(move |_| style.get().mode);
    view! {if move ||selected.get().is_some() {
        column(class="map-color-settings") {
            text(class="heading"){{move ||actual.view_lines.with(|lines|lines.iter().find(|line|Some(line.id)==selected.get()).map_or_else(||"Trace color".into(),|line|format!("Color: {}",line.name)))}}
            Select(value=Binding::controlled(Signal::derive_local(move ||match mode.get(){Mode::Solid=>"solid",Mode::Speed=>"speed",Mode::Height=>"height",Mode::Field=>"field"}.to_owned()),move |value:String|{let mode=match value.as_str(){"speed"=>Mode::Speed,"height"=>Mode::Height,"field"=>Mode::Field,_=>Mode::Solid};let _=trace_color::change(actual,selected.get_untracked(),|color|color.mode=mode);})) {
                SelectTrigger(class="map-color-mode",label="Selected trace color mode"){{move ||match mode.get(){Mode::Solid=>"Solid color",Mode::Speed=>"Speed gradient",Mode::Height=>"Height gradient",Mode::Field=>"Field gradient"}}}
                SelectContent {SelectItem(value="solid"){"Solid color"}SelectItem(value="speed"){"Speed gradient"}SelectItem(value="height"){"Height gradient"}SelectItem(value="field"){"Field gradient"}}
            }
            if move ||mode.get()!=Mode::Solid {GradientControls(workbench=workbench,pane=actual,selected=selected,appearance=style)}
            if move ||mode.get()==Mode::Solid {text(class="muted"){"Choose solid color in line controls below."}}
        }
    }}
}
