use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterSourceRow(workbench: Workbench, source: String) -> impl IntoView {
    let source = StoredValue::new_local(source);
    let name = Binding::controlled(
        Signal::derive_local(move || {
            source.with_value(|source| {
                workbench.filter_inputs.with(|inputs| {
                    inputs
                        .iter()
                        .find(|input| &input.source == source)
                        .map_or_else(String::new, |input| input.name.clone())
                })
            })
        }),
        move |name: String| {
            source.with_value(|source| {
                workbench.filter_inputs.update(|inputs| {
                    if let Some(input) = inputs.iter_mut().find(|input| &input.source == source) {
                        input.name = name;
                    }
                })
            })
        },
    );
    view! {
        column(class = "filter-source-row") {
            row(class = "filter-source-row-heading") {
                text(class = "filter-source-label") {{move || source.with_value(|source| workbench.field_name(source))}}
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "remove-filter-source", a11y:label = "Remove filter source", on:click = move |_| {
                    source.with_value(|source| workbench.filter_inputs.update(|inputs| inputs.retain(|input| &input.source != source)));
                    let sources: Vec<_> = workbench.filter_inputs.with_untracked(|inputs| inputs.iter().map(|input| input.source.clone()).collect());
                    let fields = workbench.derived.get_untracked();
                    workbench.filter_outputs.update(|outputs| outputs.retain(|id| fields.iter().any(|field| field.id == *id && sources.contains(&field.source))));
                }) { "×" }
            }
            row(class = "filter-output-row") {
                text(class = "muted") { "Output" }
                Input(value = name, class = "filter-output-name", a11y:label = move || zgui::vocab::SharedString::from(source.with_value(|source| format!("Output name for {source}"))))
            }
        }
    }
}
