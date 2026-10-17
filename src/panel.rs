use crate::{
    plot_actions::PlotActionsProps,
    plot_cursor::CursorPlotProps,
    state::{Pane, Workbench},
    tab_name::TabNameProps,
};
use plot::{
    workspace::plot_visible,
    zgui_link::{LinkGroup, PlotLinks},
    zgui_workspace::*,
};
use zgui::{prelude::*, reactive::UnsyncCallback};

#[component]
pub fn Panel(
    workbench: Workbench,
    pane: Pane,
    dock: DockState,
    geometry: WorkspaceGeometry,
    group: LinkGroup,
    cursor: RwSignal<Option<(u64, plot::PlotPoint)>, LocalStorage>,
) -> impl IntoView {
    provide_local_context(PlotLinks::with_options(pane.id, group, pane.links));
    let visible = Signal::derive_local(move || {
        plot_visible(pane.id, workbench.maximized.get(), workbench.layout.get())
    });
    view! {
        WorkspacePane(pane = pane.dock, dock = dock, geometry = geometry, active = workbench.active, maximized = workbench.maximized,
            on_select = UnsyncCallback::new(move |id| workbench.activate(id))) {
            row(class = "plot-strip") {
                PlotTabs(pane = pane.dock, dock = dock,
                    title = UnsyncCallback::new(move |id| workbench.profiles.titles.with(|titles| titles.get(&id).cloned().unwrap_or_else(|| format!("Plot {id}")))),
                    on_select = UnsyncCallback::new(move |id| workbench.activate(id)),
                    on_close = UnsyncCallback::new(move |id| workbench.close_plot(id)),
                    on_add = UnsyncCallback::new(move |id| { workbench.add_plot(id, false, true); }))
                TabName(workbench = workbench, pane = pane)
                PlotLinksMenu(options = pane.links)
                PlotActions(workbench = workbench, pane = pane)
            }
            column(class = "plot-content", class:legend-drop-active = move || workbench.line_drag.target.get().is_some_and(|target| target.pane == pane.id)) {
                CursorPlot(workbench = workbench, pane = pane, visible = visible, linked = cursor)
                if move || pane.lines.with(|lines| lines.is_empty()) {
                    column(class = "plot-empty", a11y:label = "Empty plot") {
                        text(class = "empty-title") { "Drag fields or views here" }
                        text(class = "muted") { "Choose Fields or Profiles in sidebar, then drag an item onto this plot." }
                    }
                }

            }
            if move || workbench.drag.state.get().is_some_and(|drag| drag.3) {
                column(class = "signal-drop-overlay", class:signal-drop-active = move || workbench.drop_target.get() == Some(pane.id)) {
                    text(class = "drop-title") {{move || if workbench.drag.state.get().is_some_and(|drag| drag.0.starts_with("@profile-")) { if workbench.drop_target.get() == Some(pane.id) { "Release to add view" } else { "Drop view here" } } else if workbench.drop_target.get() == Some(pane.id) { "Release to add line" } else { "Drop field here" }}}
                    text(class = "drop-name") {{move || workbench.drag.state.get().map(|drag| workbench.field_name(&drag.0)).unwrap_or_default()}}
                }
            }
        }
    }
}
