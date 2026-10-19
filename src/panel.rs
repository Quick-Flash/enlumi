use crate::{
    analysis_plot::AnalysisPlotProps,
    plot_actions::PlotActionsProps,
    plot_cursor::CursorPlotProps,
    state::{Pane, Workbench},
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
    frequency_group: LinkGroup,
    cursor: RwSignal<Option<(u64, plot::PlotPoint)>, LocalStorage>,
) -> impl IntoView {
    let group = if pane.analysis.with_untracked(|result| {
        result
            .as_ref()
            .is_some_and(|result| result.mode != crate::spectral::Mode::Spectrogram)
    }) {
        frequency_group
    } else {
        group
    };
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
                    on_rename = UnsyncCallback::new(move |(id, name): (u64, String)| {
                        workbench.profiles.titles.update(|titles| { if name.is_empty() { titles.remove(&id); } else { titles.insert(id, name); } });
                    }),
                    on_close = UnsyncCallback::new(move |id| workbench.close_plot(id)),
                    on_add = UnsyncCallback::new(move |id| { workbench.add_plot(id, false, true); }))
                PlotLinksMenu(options = pane.links)
                PlotActions(workbench = workbench, pane = pane)
            }
            column(class = "plot-content", class:legend-drop-active = move || workbench.line_drag.target.get().is_some_and(|target| target.pane == pane.id)) {
                if move || pane.analysis.with(|result| result.as_ref().is_some_and(|result| result.heatmap.is_some())) {
                    {move || pane.analysis.get().map(|result| AnyView::new(view! {AnalysisPlot(result=result,state=pane.plot,visible=visible)}))}
                } else {
                    CursorPlot(workbench = workbench, pane = pane, visible = visible, linked = cursor)
                }
                if move || pane.analysis.get().is_none() && pane.lines.with(|lines| lines.is_empty()) {
                    column(class = "plot-empty", a11y:label = "Empty plot") {
                        text(class = "empty-title") { "Drag fields or views here" }
                        text(class = "muted") { "Choose Fields or Profiles in sidebar, then drag an item onto this plot." }
                    }
                }

            }
            if move || pane.analysis.get().is_none() && workbench.drag.state.get().is_some_and(|drag| drag.3) {
                column(class = "signal-drop-overlay", class:signal-drop-active = move || workbench.drop_target.get() == Some(pane.id)) {
                    text(class = "drop-title") {{move || if workbench.drag.state.get().is_some_and(|drag| drag.0.starts_with("@profile/")) { if workbench.drop_target.get() == Some(pane.id) { "Release to add view" } else { "Drop view here" } } else if workbench.drop_target.get() == Some(pane.id) { "Release to add line" } else { "Drop field here" }}}
                    text(class = "drop-name") {{move || workbench.drag.state.get().map(|drag| workbench.field_name(&drag.0)).unwrap_or_default()}}
                }
            }
        }
    }
}

impl Workbench {
    pub fn plot_units(self, pane: Pane) -> std::collections::BTreeSet<String> {
        pane.lines.with(|lines| {
            lines
                .iter()
                .filter_map(|line| {
                    self.profiles
                        .metadata
                        .with(|all| all.get(&line.source).map(|m| m.unit.clone()))
                })
                .filter(|unit| !unit.is_empty())
                .collect()
        })
    }
    pub fn separate_plot_units(self, pane: Pane) {
        let lines = pane.lines.get_untracked();
        let mut groups = std::collections::BTreeMap::<String, Vec<crate::state::Line>>::new();
        for line in lines {
            let unit = self
                .profiles
                .metadata
                .with_untracked(|all| all.get(&line.source).map(|m| m.unit.clone()))
                .filter(|u| !u.is_empty())
                .unwrap_or("Unknown unit".into());
            groups.entry(unit).or_default().push(line);
        }
        if groups.len() < 2 {
            return;
        }
        let styles = pane.plot.styles.get_untracked();
        let hidden = pane.plot.hidden.get_untracked();
        let drawing = pane.drawing.get_untracked();
        for (index, (unit, lines)) in groups.into_iter().enumerate() {
            let target = if index == 0 {
                pane
            } else {
                self.add_plot(pane.id, true, false)
            };
            target.plot.styles.set(
                styles
                    .iter()
                    .filter(|(id, _)| lines.iter().any(|line| line.id.to_string() == **id))
                    .map(|(id, style)| (id.clone(), *style))
                    .collect(),
            );
            target.plot.hidden.set(
                hidden
                    .iter()
                    .filter(|id| lines.iter().any(|line| line.id.to_string() == **id))
                    .cloned()
                    .collect(),
            );
            target.drawing.set(
                drawing
                    .iter()
                    .filter(|(id, _)| lines.iter().any(|line| line.id == **id))
                    .map(|(id, options)| (*id, *options))
                    .collect(),
            );
            target.lines.set(lines);
            target.geometry.set(pane.geometry.get_untracked());
            target.fit_axes(plot::interaction::Axes::BOTH);
            self.profiles.titles.update(|titles| {
                titles.insert(target.id, unit);
            });
        }
    }
}
