use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterKindPicker(workbench: Workbench) -> impl IntoView {
    let builtin = RwSignal::new_local("low".to_owned());
    let mode = Binding::controlled(
        Signal::derive_local(move || {
            vec![if workbench.filter_kind.get() == "rust" {
                "rust".into()
            } else {
                "builtin".into()
            }]
        }),
        move |values: Vec<String>| {
            let Some(value) = values.first() else {
                return;
            };
            if value == "rust" {
                let kind = workbench.filter_kind.get_untracked();
                if kind != "rust" {
                    builtin.set(kind);
                }
                workbench.set_filter_kind("rust".into());
            } else {
                workbench.set_filter_kind(builtin.get_untracked());
            }
        },
    );
    view! {
        column(class = "filter-kind-picker") {
            ToggleGroup(value = mode, selection = ToggleSelection::Single, variant = ToggleVariant::Outline, size = ToggleSize::Sm, spacing = 1.5, label = "Filter mode", class = "filter-mode-picker", disabled = Signal::derive_local(move || workbench.script_busy.get())) {
                ToggleGroupItem(value = "builtin", label = "Built-in filters", class = "builtin-filter-mode") { "Built-in" }
                ToggleGroupItem(value = "rust", label = "Custom Rust function", class = "custom-rust-filter") { "Rust function" }
            }
                Select(value = Binding::controlled(Signal::derive_local(move || if workbench.filter_kind.get()=="rust" {String::new()} else {workbench.filter_kind.get()}), move |kind: String| {builtin.set(kind.clone());workbench.set_filter_kind(kind)})) {
                    SelectTrigger(label = "Filter type", class = "filter-type",disabled=Signal::derive_local(move ||workbench.script_busy.get())) { SelectValue(placeholder = "Choose filter") }
                    SelectContent(class = "filter-type-surface") {
                        SelectItem(value = "low") { "Low-pass (PT1)" }
                        SelectItem(value = "high") { "High-pass (PT1)" }
                        SelectItem(value = "average", class = "filter-type-average") { "Moving average" }
                        SelectItem(value = "derivative", class = "filter-type-derivative") { "Derivative" }
                        SelectItem(value = "integral", class = "filter-type-integral") { "Integral" }
                        SelectItem(value = "affine") { "Gain / offset" }
                        SelectItem(value = "magnitude") { "Magnitude" }
                        SelectItem(value = "rpy") { "Quaternion \u{2192} RPY" }
                    }
                }
            if move ||workbench.filter_kind.get()=="rust" {text(class="muted") {"Choose a built-in filter above to switch from Rust."}}
        }
    }
}
