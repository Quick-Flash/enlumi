use super::{SeriesActions, SeriesPane as Pane, editor::SeriesEditorProps};
use zgui::{prelude::*, reactive::UnsyncCallback};
use zgui_ui::prelude::*;

#[component]
pub fn PlotEditor(
    actions: SeriesActions,
    #[prop(optional)] fixed_connection: Option<crate::line_draw::Connection>,
    pane: Pane,
    #[prop(default = RwSignal::new_local(false))] open: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let gear = zgui::elements::canvas().class("gear-icon").draw(|cx| {
        let mut path = kurbo::BezPath::new();
        for index in 0..32 {
            let angle = f64::from(index) * std::f64::consts::TAU / 32.0;
            let radius = if index % 4 < 2 { 8.0 } else { 6.0 };
            let point = (9.0 + radius * angle.cos(), 9.0 + radius * angle.sin());
            if index == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        path.close_path();
        crate::append_circle(&mut path, (9.0, 9.0), 2.5);
        crate::zgui_renderer::push_stroke(
            cx.scene,
            path,
            zgui::canvas::zgui_color::Color::srgb_u8(215, 225, 217, 255),
            1.25,
        );
    });
    view! {
        Dialog(open = open, on_open_change = UnsyncCallback::new(move |open| {
            if open && !pane.lines.with_untracked(|lines| lines.iter().any(|line| Some(line.id) == pane.selected.get_untracked()))
                && let Some(id) = pane.lines.with_untracked(|lines| lines.first().map(|line| line.id)) { pane.selected.set(Some(id)); actions.select.run((pane.id, id)); }
        })) {
            DialogTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "plot-settings-trigger plot-gear", a11y:label = "Plot line settings") { {gear.into_view()} }
            DialogContent(class = "plot-settings-surface plot-editor-surface") {
                column(class = "plot-settings-content") {
                    DialogTitle(class = "settings-title") {{format!("Plot {} line settings", pane.id)}}
                    SeriesEditor(fixed_connection = fixed_connection, actions = actions, pane = pane, on_extra = UnsyncCallback::new(move |()| open.set(false)))
                    DialogClose(size = ButtonSize::Sm, variant = ButtonVariant::Outline, class = "settings-done") { "Done" }
                }
            }
        }
    }
}
