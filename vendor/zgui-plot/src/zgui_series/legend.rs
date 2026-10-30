use super::{
    LegendHost, SeriesPane as Pane, legend_entry::LegendEntryProps, plot_editor::PlotEditorProps,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

/// Related PlotLegend settings.
#[derive(Clone)]
pub struct LegendOptions {
    pub fixed_connection: Option<crate::line_draw::Connection>,
    pub extra_visible: Signal<bool, LocalStorage>,
    pub custom_entries: bool,
    pub settings_open: RwSignal<bool, LocalStorage>,
}
impl Default for LegendOptions {
    fn default() -> Self {
        Self {
            fixed_connection: None,
            extra_visible: Signal::derive_local(|| false),
            custom_entries: false,
            settings_open: RwSignal::new_local(false),
        }
    }
}

#[component]

pub fn PlotLegend(
    host: LegendHost,
    pane: Pane,
    node_ref: NodeRef,
    #[prop(optional)] children: Option<ChildrenFn>,
    #[prop(default = LegendOptions::default())] options: LegendOptions,
) -> impl IntoView {
    let LegendOptions {
        fixed_connection,
        extra_visible,
        custom_entries,
        settings_open,
    } = options;

    let children = StoredValue::new_local(children);
    let collapsed = RwSignal::new_local(false);
    view! {
        if move || !pane.lines.get().is_empty() || extra_visible.get() {
        column(class = "workbench-legend", node_ref = node_ref) {
            row(class = "legend-heading") {
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                    class = "legend-collapse",
                    {..Attrs::new().a11y_from(A11yBinding::new(Role::Button)
                        .expanded(move || !collapsed.get())
                        .label(move || if collapsed.get() { "Expand legend" } else { "Collapse legend" }))},
                    on:click = move |event| { collapsed.update(|value| *value = !*value); event.stop_propagation(); }) {
                    vector(class = "legend-chevron", prop:viewBox = "0 0 12 12", a11y:hidden = true,
                        prop:d = move || PropValue::from(if collapsed.get() { "M4 2 L8 6 L4 10" } else { "M2 4 L6 8 L10 4" }))
                    text {"Legend"}
                }
                PlotEditor(fixed_connection = fixed_connection, actions = host.actions, pane = pane, open = settings_open)
            }
            if move || !collapsed.get() {
            if move || !custom_entries {
            for line in move || pane.lines.get(), key = |line| (line.id, line.label()) {
                LegendEntry(host = host, pane = pane, line = line)
            }
            }
            {children.with_value(|children| children.as_ref().map(|children| children.view()))}
            }
        }
        }
    }
}
