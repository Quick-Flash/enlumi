use crate::{
    browser::BrowserProps, filter_workspace::FilterWorkspaceProps, fps::FpsProps, state::Workbench,
    workspace::PlotWorkspaceProps,
};
use std::path::PathBuf;
use zgui::prelude::*;
use zgui_ui::prelude::*;
use zgui_ui_tokens::prelude::*;

#[component]
pub fn WorkbenchApp(initial_path: Option<PathBuf>) -> impl IntoView {
    let workbench = Workbench::new();
    if let Some(path) = initial_path {
        workbench.open(path);
    }
    view! {
        ThemeProvider(scheme = ColorScheme::Dark, dark = Theme::dark().with_css(include_str!("theme.css"))) {
            column(class = "page workbench", class:workbench-dragging = move || workbench.drag.state.get().is_some_and(|drag| drag.3) || workbench.line_drag.state.get().is_some_and(|drag| drag.moving),
                on:key_down:capture = move |event| {
                    if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape) && (workbench.drag.state.get_untracked().is_some() || workbench.line_drag.state.get_untracked().is_some()) {
                        workbench.line_drag.cancel();
                        workbench.drag.state.set(None); workbench.drop_target.set(None); event.prevent_default(); event.stop_propagation();
                        workbench.filter_drop_active.set(false);
                    }
                }) {
                row(class = "toolbar") {
                    Button(size = ButtonSize::Xs, class = "sidebar-toggle", class:setting-active = move || workbench.sidebar_visible.get(),
                        a11y:label = "Toggle fields sidebar", on:click = move |_| workbench.toggle_sidebar()) {
                        vector(class = "sidebar-toggle-icon", prop:viewBox = "0 0 20 20", prop:d = move || PropValue::from(if workbench.sidebar_visible.get() { "M2 3 H18 V17 H2 Z M7 3 V17 M14 7 L11 10 L14 13" } else { "M2 3 H18 V17 H2 Z M7 3 V17 M10 7 L13 10 L10 13" }), a11y:hidden = true)
                    }
                    Button(size = ButtonSize::Sm, on:click = move |_| {
                        spawn_local(async move {
                            let path = blocking(|| rfd::FileDialog::new().add_filter("PX4 ULog", &["ulg"]).pick_file()).await;
                            if let Some(path) = path { workbench.open(path); }
                        });
                    }) { "Open ULog" }
                    Button(size = ButtonSize::Sm, class = "equalize-panels", a11y:label = "Equalize dock panel sizes",
                        disabled = Signal::derive_local(move || !matches!(workbench.layout.get(), plot::workspace::layout::Layout::Split { .. })),
                        on:click = move |_| workbench.equalize_panels()) { "Equalize panels" }
                    text(class = "file-name") {{move || workbench.log.with(|log| log.as_ref().map_or_else(|| "Enlumi".into(), |log| log.name.clone()))}}
                    text(class = "status", a11y:role = Role::Status) {{move || workbench.status.get()}}
                    Fps()
                }
                row(class = "body") {
                    Browser(workbench = workbench)
                    column(class = "plot-page", class:hidden = move || workbench.filters_open.get()) { PlotWorkspace(workbench = workbench) }
                    if move || workbench.filters_open.get() { FilterWorkspace(workbench = workbench) }
                }
            }
        }
    }
}
