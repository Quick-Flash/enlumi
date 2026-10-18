use crate::action_button::ActionButtonProps;
use crate::analysis_workspace::AnalysisWorkspaceProps;
use crate::github_link::GithubLinkProps;
use crate::preferences::ThemePreference;
use crate::{
    browser::BrowserProps, filter_workspace::FilterWorkspaceProps, fps::FpsProps, state::Workbench,
    workspace::PlotWorkspaceProps,
};
use crate::{theme::AppThemeProps, theme_picker::ThemePickerProps};
use crate::{
    update_controls::UpdateControlsProps, update_notice::UpdateNoticeProps,
    update_state::UpdateState,
};
use std::path::PathBuf;
use zgui::prelude::*;
use zgui_ui::prelude::*;
use zgui_ui_primitives::Orientation;

#[component]
pub fn WorkbenchApp(
    initial_path: Option<PathBuf>,
    #[prop(default = ThemePreference::user())] preferences: ThemePreference,
) -> impl IntoView {
    let workbench = Workbench::new();
    if let Some(path) = initial_path {
        workbench.open(path);
    }
    let palette = RwSignal::new_local(preferences.load());
    let updates = UpdateState::new();
    view! {
        AppTheme(palette = palette) {
            column(class = "page workbench", class:workbench-dragging = move || workbench.drag.state.get().is_some_and(|drag| drag.3) || workbench.line_drag.state.get().is_some_and(|drag| drag.moving) || workbench.filter_input_ui.drag.get().is_some_and(|drag| drag.moving),
                on:key_down:capture = move |event| {
                    if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape) && (workbench.geometry.editing.get_untracked() || workbench.geometry.library_drag.get_untracked().is_some() || workbench.bands.open.get_untracked()) {
                        workbench.cancel_band_source(); workbench.finish_geometry_editing(); event.prevent_default(); event.stop_propagation(); return;
                    }
                    if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape) && workbench.filter_input_ui.drag.get_untracked().is_some() {
                        workbench.finish_filter_input_drag(true); event.prevent_default(); event.stop_propagation(); return;
                    }
                    if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape) && (workbench.drag.state.get_untracked().is_some() || workbench.line_drag.state.get_untracked().is_some()) {
                        workbench.line_drag.cancel();
                        workbench.drag.state.set(None); workbench.drop_target.set(None); event.prevent_default(); event.stop_propagation();
                        workbench.filter_drop_active.set(false);
                    }
                }) {
                row(class = "toolbar") {
                    ActionButton(icon = true, class = "sidebar-toggle", class:setting-active = move || workbench.sidebar_visible.get(),
                        a11y:label = "Toggle fields sidebar", on:click = move |_| workbench.toggle_sidebar()) {
                        vector(class = "sidebar-toggle-icon", prop:viewBox = "0 0 20 20", prop:d = move || PropValue::from(if workbench.sidebar_visible.get() { "M2 3 H18 V17 H2 Z M7 3 V17 M14 7 L11 10 L14 13" } else { "M2 3 H18 V17 H2 Z M7 3 V17 M10 7 L13 10 L10 13" }), a11y:hidden = true)
                    }
                    ActionButton(class = "open-log", on:click = move |_| {
                        spawn_local(async move {
                            let path = blocking(|| {
                                let supported: Vec<_> = ["ulg", "ULG"].into_iter()
                                    .chain(crate::log::BLACKBOX_EXTENSIONS.iter().copied()).collect();
                                rfd::FileDialog::new()
                                    .add_filter("Supported logs", &supported)
                                    .add_filter("ULog", &["ulg", "ULG"])
                                    .add_filter("Blackbox", crate::log::BLACKBOX_EXTENSIONS)
                                    .add_filter("All files", &["*"])
                                    .pick_file()
                            }).await;
                            if let Some(path) = path { workbench.open(path); }
                        });
                    }) { "Open log" }
                    ActionButton(class = "equalize-panels", a11y:label = "Equalize dock panel sizes",
                        disabled = Signal::derive_local(move || !matches!(workbench.layout.get(), plot::workspace::layout::Layout::Split { .. })),
                        on:click = move |_| workbench.equalize_panels()) { "Equalize panels" }
                    text(class = "file-name") {{move || workbench.log.with(|log| log.as_ref().map_or_else(|| "Enlumi".into(), |log| log.name.clone()))}}
                    text(class = "status", a11y:role = Role::Status) {{move || workbench.status.get()}}
                    UpdateControls(state = updates)
                    ActionButton(class:setting-active=move ||workbench.analysis.open.get(),on:click=move |_|{workbench.analysis.open.update(|open|*open = !*open);workbench.filters_open.set(false);}) {"Analyze"}
                    GithubLink(workbench = workbench)
                    ThemePicker(palette = palette, preferences = preferences.clone())
                    Fps()
                }
                UpdateNotice(state = updates)
                row(class = "body") {
                    Browser(workbench = workbench)
                    if move || !workbench.filters_open.get() {
                        ResizablePanelGroup(direction = Orientation::Horizontal, label = "Plots and frequency analysis", class = "workbench-analysis-split") {
                            ResizablePanel(default_size = 65.0, min_size = 35.0, min_pixels = 350.0, label = "Plots", class = "analysis-plots-panel") {
                                column(class = "plot-page") { PlotWorkspace(workbench = workbench) }
                            }
                            if move || workbench.analysis.open.get() {
                                ResizableHandle(label = "Resize plot and analysis panels", with_handle = true)
                                ResizablePanel(default_size = 35.0, min_size = 25.0, min_pixels = 300.0, label = "Frequency analysis", class = "analysis-side-panel") {
                                    AnalysisWorkspace(workbench = workbench)
                                }
                            }
                        }
                    }
                    if move || workbench.filters_open.get() { FilterWorkspace(workbench = workbench) }
                }
            }
        }
    }
}
