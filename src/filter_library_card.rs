use crate::action_button::ActionButtonProps;
use crate::{derived::FilterSpec, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterLibraryCard(workbench: Workbench, id: u64) -> impl IntoView {
    let current = Signal::derive_local(move || {
        workbench
            .derived
            .with(|fields| fields.iter().find(|field| field.id == id).cloned())
    });
    view! {
        row(class = "derived-library-row", class:chosen = move || workbench.filter_mode.with(|mode| mode.ids().contains(&id))) {
            Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "derived-library-item",
                a11y:label = move || zgui::vocab::SharedString::from(current.get().map_or_else(String::new, |field| format!("{}, {}. Click to {}.", field.name, field.filter.label(), if workbench.filter_mode.with(|mode| mode.ids().contains(&id)) { "finish editing" } else { "edit" }))),
                on:click = move |_| workbench.toggle_filter_editing(id)) {
                row(class = "derived-card-main") {
                    text(class = "derived-library-name") {{move || current.get().map_or_else(String::new, |field| field.name)}}
                    text(class = "derived-filter-badge") {{move || current.get().map_or("", |field| match field.filter { FilterSpec::LowPass(_) => "LP", FilterSpec::HighPass(_) => "HP", FilterSpec::ZeroPhaseLowPass(_) => "LP0", FilterSpec::ZeroPhaseHighPass(_) => "HP0", FilterSpec::MovingAverage(_) => "AVG", FilterSpec::Derivative => "d/dt", FilterSpec::Integral(_) => "INT", FilterSpec::Affine {..} => "G/O", FilterSpec::Magnitude => "MAG", FilterSpec::Quaternion(..) => "RPY", FilterSpec::Rust(..) => "RUST" })}}
                }
                text(class = "derived-library-detail") {{move || current.get().map_or_else(String::new, |field| format!("{} \u{00b7} {} inputs", field.filter.label(), field.sources.len()))}}
            }
            ActionButton(icon = true, class = "remove-saved-field", a11y:label = move || zgui::vocab::SharedString::from(current.get().map_or_else(String::new, |field| format!("Delete {}",field.name))), on:click = move |_| workbench.remove_saved_filter(id)) { "\u{00d7}" }
        }
    }
}
