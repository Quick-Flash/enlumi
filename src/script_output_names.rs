use crate::action_button::ActionButtonProps;
use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

impl Workbench {
    pub fn script_output_names(self) -> Vec<String> {
        self.script_names
            .with(|names| names.split('\n').map(str::to_owned).collect())
    }
    pub fn rename_script_output(self, index: usize, name: String) {
        self.script_names.update(|names| {
            let mut rows: Vec<_> = names.split('\n').map(str::to_owned).collect();
            if let Some(row) = rows.get_mut(index) {
                *row = name;
            }
            *names = rows.join("\n");
        });
    }
    pub fn add_script_output(self) {
        if self.script_busy.get_untracked() {
            return;
        }
        self.script_names.update(|names| {
            if names.split('\n').count() < 16 {
                let count = names.split('\n').count();
                self.script_output_ids.update(|ids| {
                    ids.resize(count, None);
                    ids.push(None);
                });
                names.push('\n');
            }
        });
    }
    pub fn remove_script_output(self, index: usize) {
        if self.script_busy.get_untracked() {
            return;
        }
        self.script_names.update(|names| {
            let mut rows: Vec<_> = names.split('\n').collect();
            if rows.len() > 1 && index < rows.len() {
                self.script_output_ids.update(|ids| {
                    ids.resize(rows.len(), None);
                    ids.remove(index);
                });
                rows.remove(index);
            }
            *names = rows.join("\n");
        });
    }
}

#[component]
pub fn ScriptOutputNames(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "script-output-names") {
            text(class = "heading") { "Outputs" }
            column(class = "script-output-list") {
                for index in move || 0..workbench.script_output_names().len(), key = |index| *index {
                    row(class = "filter-output-name-row script-output-row") {
                        text(class = "heading script-output-index") { {format!("outputs[{index}]")} }
                        Input(class = "script-output-name", value = Binding::controlled(Signal::derive_local(move || workbench.script_output_names().get(index).cloned().unwrap_or_default()), move |name| workbench.rename_script_output(index, name)),
                            disabled = Signal::derive_local(move || workbench.script_busy.get()), placeholder = "Output name", a11y:label = format!("Output {index} name"))
                        ActionButton(icon = true, class = "remove-script-output", a11y:label = format!("Remove output {index}"),
                            disabled = Signal::derive_local(move || workbench.script_busy.get() || workbench.script_output_names().len() == 1),
                            on:click = move |_| workbench.remove_script_output(index)) { "\u{00d7}" }
                    }
                }
            }
            ActionButton(class = "add-script-output", disabled = Signal::derive_local(move || workbench.script_busy.get() || workbench.script_output_names().len() >= 16), on:click = move |_| workbench.add_script_output()) { "Add output" }
        }
    }
}
