use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use kurbo::Shape;
use plot::zgui_series::ColorPickerProps;
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};
use zgui_ui::prelude::*;

fn effective_background(
    background: RwSignal<Option<[u8; 3]>, LocalStorage>,
) -> Signal<[u8; 3], LocalStorage> {
    let palette = use_local_context::<Signal<crate::theme::Palette, LocalStorage>>()
        .unwrap_or_else(|| Signal::stored_local(crate::theme::Palette::default()));
    Signal::derive_local(move || {
        background
            .get()
            .unwrap_or_else(|| palette.get().plot_background())
    })
}

pub fn css([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

impl Workbench {
    pub fn apply_background_to_all(self, source: Pane) {
        let color = source.background.get_untracked();
        for pane in self.all_panes() {
            pane.background.set(color);
        }
    }
    pub fn use_theme_backgrounds(self) {
        for pane in self.all_panes() {
            pane.background.set(None);
        }
    }
}

#[component]
pub fn PlotAppearance(workbench: Workbench, pane: Pane) -> impl IntoView {
    // Install the picker rules with the plot, before opening a popup can trigger
    // stylesheet parsing and a restyle of the entire populated workspace.
    install_stylesheet("plot-series-controls", plot::zgui_series::stylesheet());
    install_stylesheet("zui-slider", zgui_ui::slider::SliderStyle::CSS);
    install_stylesheet("zui-input", zgui_ui::input::InputStyle::CSS);
    let color = effective_background(pane.background);
    let bounds = pane.dock.node.observe_border_box();
    let compact = Signal::derive_local(move || {
        bounds
            .get()
            .is_none_or(|bounds| bounds.size.width.0 / pane.dock.node.scale() < 700.)
    });
    view! {Popover {
        PopoverTrigger(class="plot-background-trigger",class:plot-background-compact=move ||compact.get(),size=ButtonSize::Xs,variant=ButtonVariant::Ghost,a11y:label="Plot background") {
            box(class="plot-background-swatch",style:background=move ||Some(css(color.get())))
            if move ||!compact.get() {text {"Background"}}
        }
        PopoverContent(class="plot-background-menu") {
            text(class="heading") {"Plot background"}
            text(class="muted") {{move ||if pane.background.get().is_none(){"Following app theme"}else{"Custom color for this plot"}}}
            ColorPicker(color=Signal::derive_local(move || {let [r,g,b]=color.get();[r,g,b,255]}),on_change=UnsyncCallback::new(move |[r,g,b,_]:[u8;4]|pane.background.set(Some([r,g,b]))))
            row(class="plot-background-actions") {
                ActionButton(class="plot-background-reset",on:click=move |_|pane.background.set(None)) {"Use theme"}
                ActionButton(class="plot-background-all",a11y:label="Apply background to all plots",on:click=move |_|workbench.apply_background_to_all(pane)) {"Apply to all"}
            }
            ActionButton(class="plot-background-theme-all",on:click=move |_|workbench.use_theme_backgrounds()) {"Use theme for all plots"}
        }
    }}
}

/// Consumer-owned background is below grid/data, and participates in retained exports.
#[component]
pub fn PlotBackground(
    background: RwSignal<Option<[u8; 3]>, LocalStorage>,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let color = effective_background(background);
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    plot::zgui_svg::register_description(
        plot::zgui_svg::SnapshotLayer::Background,
        move |description| {
            description.background = css(color.get_untracked());
        },
    );
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        let color = color.get();
        let shown = visible.get();
        let bounds = bounds.get();
        drawing.draw(|scene| {
            scene.clear();
            if !shown {
                return;
            }
            let ([r, g, b], Some(bounds)) = (color, bounds) else {
                return;
            };
            let scale = f64::from(host.scale());
            let width = f64::from(bounds.size.width.0) / scale;
            let height = f64::from(bounds.size.height.0) / scale;
            let color = Color::srgb_u8(r, g, b, 255);
            scene.push(
                ShapeBuilder::new(kurbo::Rect::new(0., 0., width, height).to_path(0.1))
                    .fill(Brush::Linear {
                        start: (0., 0.).into(),
                        end: (1., 0.).into(),
                        stops: vec![(0., color), (1., color)],
                        repeating: false,
                    })
                    .build(),
            );
        });
    });
    let canvas = zgui::elements::canvas()
        .class("plot-background-canvas")
        .scene(&handle);
    view! {column(class="plot-background-layer",node_ref=host) {{canvas.into_view()}}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_is_local_until_applied_to_all_open_plots() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let origin = workbench.bind_view();
        let first = origin.pane();
        let second = window
            .scope
            .with(|| origin.add_plot(first.id, false, false));
        first.background.set(Some([230, 235, 240]));
        assert_eq!(second.background.get_untracked(), None);
        window.scope.with(|| workbench.new_view());
        let other_view = workbench.pane();
        window
            .scope
            .with(|| workbench.new_workspace("Other workspace".into()));
        let other_workspace = workbench.pane();
        workbench.apply_background_to_all(first);
        for pane in [first, second, other_view, other_workspace] {
            assert_eq!(pane.background.get_untracked(), Some([230, 235, 240]));
        }
        let copy = window
            .scope
            .with(|| origin.add_plot(first.id, false, false));
        origin.copy_plot(first, copy);
        assert_eq!(
            copy.background.get_untracked(),
            first.background.get_untracked()
        );
        first.background.set(None);
        workbench.apply_background_to_all(first);
        assert!(
            workbench
                .all_panes()
                .iter()
                .all(|pane| pane.background.get_untracked().is_none())
        );
    }
}
