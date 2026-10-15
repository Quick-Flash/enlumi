use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterOutputNames(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-output-names") {
            for index in move || if workbench.filter_kind.get() == "rpy" { vec![1, 2, 3] } else { vec![0] }, key = |index| *index {
                row(class = "filter-output-name-row") {
                    text(class = "heading") {{move || if index == 0 { "Output".to_owned() } else { format!("{} ({})", ["", "Roll", "Pitch", "Yaw"][index], workbench.filter_angle_unit.get()) }}}
                    Input(value = Binding::controlled(Signal::derive_local(move || workbench.filter_names.with(|names| names[index].clone())), move |name: String| workbench.filter_names.update(|names| names[index] = name)), a11y:label = ["Magnitude output name", "Roll output name", "Pitch output name", "Yaw output name"][index])
                }
            }
        }
    }
}
