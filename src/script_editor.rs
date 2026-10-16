use crate::action_button::ActionButtonProps;
use crate::{script_code_editor::ScriptCodeEditorProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;
use zgui_ui_primitives::Orientation;

#[component]
pub fn ScriptEditor(workbench: Workbench) -> impl IntoView {
    let editor = RwSignal::new_local(None::<zgui_editor::EditorHandle>);
    view! {
        column(class = "script-editor") {
            row(class = "script-editor-heading") {
                text(class = "filter-section-title") { "Rust function" }
                text(class = "script-cursor") {{move || editor.with(|editor| editor.as_ref().map_or_else(String::new, |editor| { let pos = editor.cursor_position().get(); format!("Ln {}, Col {}", pos.line + 1, pos.col + 1) }))}}
                ActionButton(class = "check-script", disabled = Signal::derive_local(move || workbench.script_busy.get() || workbench.derived_busy.get()), on:click = move |_| workbench.run_script(true)) { "Check" }
                if move || workbench.script_busy.get() {
                    ActionButton(class = "cancel-script", on:click = move |_| workbench.cancel_script()) { "Cancel" }
                }
            }
            text(class = "muted") { "Declare OUTPUTS to match named rows. Inputs use list order. State resets each run." }
            ResizablePanelGroup(direction = Orientation::Vertical, label = "Code and compilation diagnostics", class = "script-edit-panels") {
                ResizablePanel(default_size = 70.0, min_pixels = 100.0, class = "script-code-panel") {
                    ScriptCodeEditor(source = workbench.script_source, read_only = Signal::derive_local(move || workbench.script_busy.get()), on_ready = UnsyncCallback::new(move |handle| editor.set(Some(handle))))
                }
                ResizableHandle(label = "Resize compilation diagnostics", with_handle = true, class = "script-diagnostics-resizer")
                ResizablePanel(default_size = 30.0, min_pixels = 70.0, class = "script-diagnostics-panel") {
                    text(class = "filter-section-title") { "Compilation / filter diagnostics" }
                    if move || !workbench.filter_error.get().is_empty() {
                        text(class = "filter-error script-diagnostics", a11y:role = Role::Alert) {{move || workbench.filter_error.get()}}
                    } else {
                        text(class = "muted script-diagnostics-empty") {{move || if workbench.script_busy.get() { "Compiling / running filter..." } else if workbench.status.get() == "Rust filter compiled successfully" { "Rust filter compiled successfully" } else { "No compilation errors." }}}
                    }
                }
            }
            text(class = "muted") { "Local trusted code. Compiler can read local files through Rust macros. WASM execution has no filesystem/network imports." }
        }
    }
}
