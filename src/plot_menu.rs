use crate::{
    preset_capture::CaptureKind,
    state::{Pane, Workbench},
};
use plot::zgui_series::SeriesEditorProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotPresetMenu(workbench: Workbench, pane: Pane) -> impl IntoView {
    let settings = RwSignal::new_local(false);
    view! {
    PlotMenuEditor(workbench=workbench,pane=pane,open=settings)
    DropdownMenu {
        DropdownMenuTrigger(class="app-action plot-preset-menu",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Plot actions") {
            vector(class="plot-more-icon",prop:viewBox="0 0 16 16",prop:d="M3 7 H4 V8 H3 Z M7.5 7 H8.5 V8 H7.5 Z M12 7 H13 V8 H12 Z",a11y:hidden=true)
        }
        DropdownMenuContent {PlotMenuEntries(workbench=workbench,pane=pane,settings=settings)}
    }}
}
#[component]
pub fn PlotContext(workbench: Workbench, pane: Pane, children: Children) -> impl IntoView {
    let settings = RwSignal::new_local(false);
    view! {
    PlotMenuEditor(workbench=workbench,pane=pane,open=settings)
    ContextMenu {
        ContextMenuTrigger(class="plot-menu-area") {
            PlotMenuGate(pane=pane) {{children.into_view_once()}}
        }
        ContextMenuContent(class="plot-fit-menu plot-area-context") {PlotAreaMenuEntries(workbench=workbench,pane=pane,settings=settings)}
    }}
}
#[component]
pub fn PlotTabContext(
    workbench: Workbench,
    dock: plot::zgui_workspace::DockState,
    children: Children,
) -> impl IntoView {
    let target = RwSignal::new_local(None::<u64>);
    view! {ContextMenu {
        ContextMenuTrigger(class="plot-tab-context") {TabContextGate(dock=dock,target=target) {{children.into_view_once()}}}
        ContextMenuContent(class="plot-fit-menu plot-tab-context-menu") {PlotTabMenuEntries(workbench=workbench,target=target)}
    }}
}
#[component]
fn TabContextGate(
    dock: plot::zgui_workspace::DockState,
    target: RwSignal<Option<u64>, LocalStorage>,
    children: Children,
) -> impl IntoView {
    let place = ContextMenuPlace::current();
    let hit = move |x: f32, y: f32| {
        dock.nodes.with_value(|nodes| {
            nodes.iter().find_map(|((_, id), node)| {
                let rect = plot::zgui_svg::node_rect(*node)?;
                (f64::from(x) >= rect.x
                    && f64::from(x) <= rect.x + rect.width
                    && f64::from(y) >= rect.y
                    && f64::from(y) <= rect.y + rect.height)
                    .then_some(*id)
            })
        })
    };
    view! {column(class="plot-tab-context-content",
    on:pointer_down:capture=move |event|if event.button==Some(zgui::vocab::PointerButton::Secondary){if let Some(id)=hit(event.position.x.0,event.position.y.0){target.set(Some(id));if let Some(place)=place{place.open_at(event.position);}}event.prevent_default();event.stop_propagation();},
    on:context_menu:capture=move |event|{if let Some(id)=hit(event.position.x.0,event.position.y.0){target.set(Some(id));if let Some(place)=place{place.open_at(event.position);}}event.prevent_default();event.stop_propagation();}) {{children.into_view_once()}}}
}
#[component]
fn PlotTabMenuEntries(
    workbench: Workbench,
    target: RwSignal<Option<u64>, LocalStorage>,
) -> impl IntoView {
    let resolve = move || {
        target.get_untracked().and_then(|id| {
            workbench
                .panes()
                .with_untracked(|panes| panes.iter().find(|pane| pane.id == id).copied())
        })
    };
    view! {
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.activate(target.id);workbench.open_preset_browser(crate::preset_browser::BrowseTarget::Plot,None);})) {"Add plot preset…"}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.begin_save(CaptureKind::Plot(target.id));})) {"Save plot preset…"}
        MenuItem(disabled=Signal::derive_local(move ||resolve().is_none_or(|pane|pane.preset.get_untracked().is_none())),on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve() && let Some(source)=target.preset.get_untracked(){workbench.update_preset(CaptureKind::Plot(target.id),source);})) {"Update source preset"}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.duplicate_plot(target);})) {"Duplicate plot"}
        MenuSeparator {}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.save_screenshot(Some(target),plot::zgui_svg::ScreenshotFormat::Png);})) {"Save PNG…"}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.save_screenshot(Some(target),plot::zgui_svg::ScreenshotFormat::Svg);})) {"Save SVG…"}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.copy_screenshot(Some(target));})) {"Copy image"}
        MenuItem(on_select=UnsyncCallback::new(move |()|if let Some(target)=resolve(){workbench.close_plot(target.id);})) {"Close plot"}
    }
}
#[component]
fn PlotMenuGate(pane: Pane, children: Children) -> impl IntoView {
    let place = ContextMenuPlace::current();
    let inside = move |position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>| {
        pane.menu_area
            .get_untracked()
            .is_some_and(|area| Workbench::inside(area, position.x.0, position.y.0))
            || pane.analysis.with_untracked(|result| {
                result
                    .as_ref()
                    .is_some_and(|result| result.heatmap.is_some())
            })
    };
    view! {column(class="plot-menu-gate",on:pointer_down:capture=move |event|{
        if event.button==Some(zgui::vocab::PointerButton::Secondary)&&inside(event.position){if let Some(place)=place{place.open_at(event.position);}event.prevent_default();event.stop_propagation();}
    },on:pointer_down=move |event|if event.button==Some(zgui::vocab::PointerButton::Secondary){event.stop_propagation();},on:context_menu:capture=move |event|{if inside(event.position){if let Some(place)=place{place.open_at(event.position);}event.prevent_default();event.stop_propagation();}},on:context_menu=move |event|event.stop_propagation()) {{children.into_view_once()}}}
}
#[component]
fn PlotMenuEntries(
    workbench: Workbench,
    pane: Pane,
    settings: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    view! {
        MenuItem(class="plot-add-fields",on_select=UnsyncCallback::new(move |()|{workbench.activate(pane.id);workbench.sidebar_visible.set(true);})) {"Choose fields to add…"}
        MenuItem(class="plot-add-preset",on_select=UnsyncCallback::new(move |()|workbench.open_preset_browser(crate::preset_browser::BrowseTarget::Plot,None))) {"Add plot preset…"}
        MenuItem(class="plot-save-preset",on_select=UnsyncCallback::new(move |()|workbench.begin_save(CaptureKind::Plot(pane.id)))) {"Save plot preset…"}
        MenuItem(class="plot-update-preset",disabled=Signal::derive_local(move ||pane.preset.with(Option::is_none)),on_select=UnsyncCallback::new(move |()|if let Some(source)=pane.preset.get_untracked(){workbench.update_preset(CaptureKind::Plot(pane.id),source);})) {{move ||pane.preset.get().map_or_else(||"Update source preset".to_owned(),|source|if workbench.preset_modified(&CaptureKind::Plot(pane.id),&source){"Update source preset · Modified".into()}else{"Update source preset".into()})}}
        MenuSeparator {}
        PlotAreaMenuEntries(workbench=workbench,pane=pane,settings=settings)
    }
}
#[component]
fn PlotAreaMenuEntries(
    workbench: Workbench,
    pane: Pane,
    settings: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    view! {
        MenuItem(class="plot-fit-x",on_select=UnsyncCallback::new(move |()|fit(pane,plot::interaction::Axes::X_ONLY))) {"Fit X axis"}
        MenuItem(class="plot-fit-y",on_select=UnsyncCallback::new(move |()|fit(pane,plot::interaction::Axes{x:false,y:true}))) {"Fit Y axis"}
        MenuItem(class="plot-fit-both",on_select=UnsyncCallback::new(move |()|fit(pane,plot::interaction::Axes::BOTH))) {"Fit both axes"}
        MenuItem(class="plot-line-settings",disabled=Signal::derive_local(move ||pane.lines.with(Vec::is_empty)),on_select=UnsyncCallback::new(move |()|settings.set(true))) {"Line settings"}
        MenuSeparator {}
        MenuItem(class="preset-duplicate-plot",on_select=UnsyncCallback::new(move |()|workbench.duplicate_plot(pane))) {"Duplicate plot"}
        MenuItem(on_select=UnsyncCallback::new(move |()|workbench.save_screenshot(Some(pane),plot::zgui_svg::ScreenshotFormat::Png))) {"Save PNG…"}
        MenuItem(on_select=UnsyncCallback::new(move |()|workbench.save_screenshot(Some(pane),plot::zgui_svg::ScreenshotFormat::Svg))) {"Save SVG…"}
        MenuItem(on_select=UnsyncCallback::new(move |()|workbench.copy_screenshot(Some(pane)))) {"Copy image"}
        MenuItem(class="preset-close-plot",on_select=UnsyncCallback::new(move |()|workbench.close_plot(pane.id))) {"Close plot"}
    }
}

fn fit(pane: Pane, axes: plot::interaction::Axes) {
    if !axes.y
        || axes.x
        || pane.analysis.with_untracked(|result| {
            result
                .as_ref()
                .is_some_and(|result| result.heatmap.is_some())
        })
    {
        pane.fit_axes(axes);
        return;
    }
    let data = plot::plot_data::PlotData::Time(pane.series.get_untracked());
    let drawing = pane.drawing.with_untracked(|drawing| {
        drawing
            .iter()
            .map(|(id, draw)| (id.to_string(), *draw))
            .collect()
    });
    let current = pane.plot.ranges.get_untracked();
    if let Some(y) = data.fitted_y_range(
        &pane.plot.hidden.get_untracked(),
        current.x,
        pane.plot.histogram.get_untracked().bins,
        &drawing,
    ) {
        let _ = pane.plot.set_view(plot::Ranges { x: current.x, y });
    }
}

#[component]
fn PlotMenuEditor(
    workbench: Workbench,
    pane: Pane,
    open: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let mut actions = workbench.legend_host().actions;
    if pane.analysis.with_untracked(Option::is_some) {
        actions.extra = Some(UnsyncCallback::new(move |_| workbench.edit_analysis(pane)));
        actions.extra_label = "FFT / PSD settings";
    }
    view! {Dialog(open=open) {DialogContent(class="plot-settings-surface plot-editor-surface") {
        column(class="plot-settings-content") {
            DialogTitle(class="settings-title") {{format!("Plot {} line settings",pane.id)}}
            SeriesEditor(actions=actions,pane=pane.display(),on_extra=UnsyncCallback::new(move |()|open.set(false)))
            DialogClose(class="settings-done",size=ButtonSize::Sm,variant=ButtonVariant::Outline) {"Done"}
        }
    }}}
}
