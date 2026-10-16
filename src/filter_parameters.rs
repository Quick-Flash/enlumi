use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn FilterParameters(workbench: Workbench) -> impl IntoView {
    let phase_control = NodeRef::new();
    view! { column(class = "filter-parameter-controls") {
                if move || workbench.filter_kind.get() == "affine" {
                    row(class = "filter-angle-presets") {
                        text(class = "muted") { "Presets" }
                        ActionButton(class = "preset-rad-to-deg", on:click = move |_| {
                            workbench.filter_gain.set(1.0_f64.to_degrees().to_string()); workbench.filter_offset.set("0".into());
                        }) { "rad → deg" }
                        ActionButton(class = "preset-deg-to-rad", on:click = move |_| {
                            workbench.filter_gain.set(1.0_f64.to_radians().to_string()); workbench.filter_offset.set("0".into());
                        }) { "deg → rad" }
                    }
                    row(class = "filter-parameters") {
                        column { text(class = "heading") { "Gain" } Input(value = workbench.filter_gain, a11y:label = "Filter gain") }
                        column { text(class = "heading") { "Offset" } Input(value = workbench.filter_offset, a11y:label = "Filter offset") }
                    }
                    text(class = "muted") { "Output = input × gain + offset" }
                } else {
                if move || workbench.filter_kind.get() == "average" {
                    text(class = "heading") { "Window (samples)" }
                    Input(value = workbench.filter_window, class = "filter-window", a11y:label = "Moving average window in samples")
                    text(class = "muted") { "Trailing sample mean; gaps reset window." }
                } else {
                if move || workbench.filter_kind.get() == "integral" {
                    text(class = "heading") { "Initial value" }
                    Input(value = workbench.filter_initial, class = "filter-integral-initial", a11y:label = "Initial integral value")
                    text(class = "muted") { "Trapezoidal integration using actual time. Resets after gaps." }
                } else {
                if move || workbench.filter_kind.get() == "derivative" {
                    text(class = "muted") { "Centered derivative using actual time; one-sided at edges. Gaps stay separate." }
                } else {
                    text(class = "heading") { "Cutoff (Hz)" }
                    Input(value = workbench.filter_cutoff, a11y:label = "Filter cutoff in Hz")
                    row(class = "filter-option") {
                        Checkbox(node_ref = phase_control, class = "filter-zero-phase", checked = Binding::controlled(Signal::derive_local(move || if workbench.filter_zero_phase.get() { Checked::Yes } else { Checked::No }), move |value| workbench.filter_zero_phase.set(value == Checked::Yes)))
                        Label(control = phase_control, class = "filter-option-label") { "Zero phase (forward/backward)" }
                    }
                    text(class = "muted") {{move || if workbench.filter_zero_phase.get() { "Two passes; stronger attenuation. Edge transients remain." } else { "Causal PT1 using actual time. Gaps reset state." }}}
                }

                }
                }
                }

    } }
}
