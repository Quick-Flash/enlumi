use crate::state::Workbench;
use zgui::prelude::*;

#[component]
pub fn FilterModeBanner(workbench: Workbench) -> impl IntoView {
    view! {
        row(class = "filter-mode-banner", class:filter-mode-edit = move || !workbench.filter_mode.get().is_new()) {
            text(class = "filter-mode-title", a11y:role = Role::Status) {{move || {
                let mode = workbench.filter_mode.get();
                if mode.is_new() { "Create derived fields".to_owned() }
                else if mode.ids().len() == 1 { workbench.derived.with(|fields| fields.iter().find(|field| field.id == mode.ids()[0]).map_or_else(|| "Editing saved field".to_owned(), |field| format!("Editing {}",field.name))) }
                else { format!("Editing {} fields",mode.ids().len()) }
            }}}
            if move || !workbench.filter_mode.get().is_new() {
                text(class = "filter-edit-state", class:filter-edit-unchanged = move || unchanged(workbench)) {{move || {
                    if unchanged(workbench) { "Unchanged" } else { "Changed" }
                }}}
            }
        }
    }
}

fn unchanged(workbench: Workbench) -> bool {
    use crate::derived::FilterSpec;

    let Some(id) = workbench.filter_mode.get().selected() else {
        return true;
    };
    let Some(saved) = workbench.derived.with(|fields| {
        fields
            .iter()
            .find(|field| field.id == id)
            .map(|field| field.filter.clone())
    }) else {
        return false;
    };
    let parse = |value: String| value.trim().parse::<f64>().ok();
    match workbench.filter_kind.get().as_str() {
        "low" => parse(workbench.filter_cutoff.get()).is_some_and(|value| match saved {
            FilterSpec::LowPass(old) => value == old && !workbench.filter_zero_phase.get(),
            FilterSpec::ZeroPhaseLowPass(old) => value == old && workbench.filter_zero_phase.get(),
            _ => false,
        }),
        "high" => parse(workbench.filter_cutoff.get()).is_some_and(|value| match saved {
            FilterSpec::HighPass(old) => value == old && !workbench.filter_zero_phase.get(),
            FilterSpec::ZeroPhaseHighPass(old) => value == old && workbench.filter_zero_phase.get(),
            _ => false,
        }),
        "average" => workbench
            .filter_window
            .get()
            .trim()
            .parse::<usize>()
            .ok()
            .is_some_and(|value| saved == FilterSpec::MovingAverage(value)),
        "derivative" => saved == FilterSpec::Derivative,
        "integral" => parse(workbench.filter_initial.get())
            .is_some_and(|value| saved == FilterSpec::Integral(value)),
        "affine" => {
            let gain = parse(workbench.filter_gain.get());
            let offset = parse(workbench.filter_offset.get());
            matches!((saved, gain, offset), (FilterSpec::Affine { gain: old_gain, offset: old_offset }, Some(gain), Some(offset)) if gain == old_gain && offset == old_offset)
        }
        "magnitude" => saved == FilterSpec::Magnitude,
        "rpy" => matches!(saved, FilterSpec::Quaternion(..)),
        "rust" => {
            matches!(saved, FilterSpec::Rust(ref script, _) if script.source == workbench.script_source.get())
        }
        _ => false,
    }
}
